use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};

/// Shared mic PCM buffer: interleaved little-endian i16 stereo @ 48 kHz.
#[derive(Clone)]
pub struct MicBuffer {
    inner: Arc<Mutex<VecDeque<u8>>>,
}

impl MicBuffer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::with_capacity(48000 * 4))),
        }
    }

    pub fn clear(&self) {
        if let Ok(mut q) = self.inner.lock() {
            q.clear();
        }
    }

    pub fn push_i16(&self, samples: &[i16]) {
        let Ok(mut q) = self.inner.lock() else {
            return;
        };
        // Cap ~3 seconds to avoid unbounded growth if encoder stalls.
        let max = 48000 * 3 * 2 * 2;
        for s in samples {
            if q.len() >= max {
                q.pop_front();
                q.pop_front();
            }
            let b = s.to_le_bytes();
            q.push_back(b[0]);
            q.push_back(b[1]);
        }
    }

    pub fn drain(&self, max_bytes: usize) -> Vec<u8> {
        let Ok(mut q) = self.inner.lock() else {
            return Vec::new();
        };
        let n = max_bytes.min(q.len());
        let n = n - (n % 4); // keep stereo i16 frames aligned
        q.drain(..n).collect()
    }

    pub fn len_bytes(&self) -> usize {
        self.inner.lock().map(|q| q.len()).unwrap_or(0)
    }
}

#[derive(Clone, Debug)]
pub struct MicDeviceInfo {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Copy, Debug)]
pub struct NoiseReduceConfig {
    pub enabled: bool,
    /// 0..=100 — higher = more aggressive gate / stronger high-pass.
    pub strength: u8,
}

impl Default for NoiseReduceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strength: 55,
        }
    }
}

pub fn list_input_devices() -> Vec<MicDeviceInfo> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();

    let Ok(devices) = host.input_devices() else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for (i, device) in devices.enumerate() {
        let Ok(name) = device.name() else {
            continue;
        };
        // Skip devices that cannot open an input config.
        if device.default_input_config().is_err() {
            continue;
        }
        let label = if !default_name.is_empty() && name == default_name {
            format!("{name}  ·  بنەڕەتی")
        } else {
            name.clone()
        };
        out.push(MicDeviceInfo {
            id: format!("idx:{i}:{name}"),
            label,
        });
    }
    out
}

fn find_device(device_id: Option<&str>) -> Result<Device> {
    let host = cpal::default_host();
    let id = device_id.unwrap_or("").trim();

    if id.is_empty() {
        return host
            .default_input_device()
            .context("هیچ مایکێکی بنەڕەتی نییە");
    }

    // Prefer exact name match encoded after idx:N:NAME
    let want_name = id.splitn(3, ':').nth(2).unwrap_or(id);

    if let Ok(devices) = host.input_devices() {
        for device in devices {
            if let Ok(name) = device.name() {
                if name == want_name || id.ends_with(&name) {
                    return Ok(device);
                }
            }
        }
    }

    // Fallback: default
    host.default_input_device()
        .context("مایکەکە نەدۆزرایەوە")
}

/// Gentle denoise: mild high-pass + soft gate that never kills speech.
struct NoiseProcessor {
    hp_l: OnePoleHp,
    hp_r: OnePoleHp,
    envelope: f32,
    noise_floor: f32,
    gate: f32,
    strength: f32,
}

impl NoiseProcessor {
    fn new(strength: u8) -> Self {
        let s = (strength as f32 / 100.0).clamp(0.0, 1.0);
        // Keep cutoff under voice fundamentals (~85Hz+).
        let cutoff = 55.0 + s * 35.0; // 55–90 Hz
        Self {
            hp_l: OnePoleHp::new(cutoff, 48_000.0),
            hp_r: OnePoleHp::new(cutoff, 48_000.0),
            envelope: 0.0,
            noise_floor: 0.002,
            gate: 1.0,
            strength: s,
        }
    }

    fn process(&mut self, samples: &mut [i16]) {
        if samples.len() < 2 {
            return;
        }
        let frames = samples.len() / 2;

        // Apply high-pass first, then gate on filtered signal.
        for i in 0..frames {
            let l = samples[i * 2] as f32 / 32768.0;
            let r = samples[i * 2 + 1] as f32 / 32768.0;
            samples[i * 2] = float_to_i16(self.hp_l.process(l));
            samples[i * 2 + 1] = float_to_i16(self.hp_r.process(r));
        }

        let mut sum_sq = 0.0f32;
        for i in 0..frames {
            let l = samples[i * 2] as f32 / 32768.0;
            let r = samples[i * 2 + 1] as f32 / 32768.0;
            let m = 0.5 * (l + r);
            sum_sq += m * m;
        }
        let rms = (sum_sq / frames.max(1) as f32).sqrt().max(1e-7);
        self.envelope = self.envelope * 0.85 + rms * 0.15;

        // Only learn floor during clearly quiet moments (never while speaking).
        if self.envelope < 0.008 {
            let adapt = 0.015;
            self.noise_floor =
                self.noise_floor * (1.0 - adapt) + self.envelope * adapt;
            self.noise_floor = self.noise_floor.clamp(0.0003, 0.012);
        }

        // Absolute thresholds so quiet laptop speech always passes.
        // Strength only changes how much hush is applied between words.
        let open_th = (self.noise_floor * 5.0).max(0.010);
        let close_th = (self.noise_floor * 2.2).max(0.004);
        // Never mute: even at 100% strength keep ~35% so voice isn't cut mid-word.
        let min_gate = 0.55 - self.strength * 0.20; // 0.55 → 0.35

        let target = if self.envelope >= open_th {
            1.0
        } else if self.envelope <= close_th {
            min_gate
        } else {
            let t = ((self.envelope - close_th) / (open_th - close_th).max(1e-6)).clamp(0.0, 1.0);
            // Smooth ease-in so consonants aren't chopped.
            let t = t * t * (3.0 - 2.0 * t);
            min_gate + (1.0 - min_gate) * t
        };

        let attack = 0.55; // open quickly on speech
        let release = 0.08; // close slowly so words aren't clipped
        let coeff = if target > self.gate { attack } else { release };
        self.gate += (target - self.gate) * coeff;

        // At low strength, barely gate — mostly just the high-pass.
        let g = if self.strength < 0.15 {
            1.0
        } else {
            // Blend toward unity so mid settings stay natural.
            let wet = 0.35 + self.strength * 0.65;
            1.0 - wet + wet * self.gate
        };

        if (g - 1.0).abs() < 0.01 {
            return;
        }
        for i in 0..frames {
            let l = samples[i * 2] as f32 / 32768.0 * g;
            let r = samples[i * 2 + 1] as f32 / 32768.0 * g;
            samples[i * 2] = float_to_i16(l);
            samples[i * 2 + 1] = float_to_i16(r);
        }
    }
}

/// One-pole high-pass filter (removes rumble / fan thump / AC hum lows).
struct OnePoleHp {
    a: f32,
    x1: f32,
    y1: f32,
}

impl OnePoleHp {
    fn new(cutoff_hz: f32, sample_rate: f32) -> Self {
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff_hz.max(20.0));
        let dt = 1.0 / sample_rate.max(1.0);
        let a = rc / (rc + dt);
        Self {
            a,
            x1: 0.0,
            y1: 0.0,
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.a * (self.y1 + x - self.x1);
        self.x1 = x;
        self.y1 = y;
        y
    }
}

pub struct MicCapture {
    _stream: Stream,
    pub buffer: MicBuffer,
    pub device_label: String,
}

impl MicCapture {
    pub fn start(device_id: Option<&str>, noise: NoiseReduceConfig) -> Result<Self> {
        let device = find_device(device_id)?;
        let device_label = device.name().unwrap_or_else(|_| "مایک".into());
        let supported = device
            .default_input_config()
            .context("نەتوانرا ڕێکخستنی مایک بخوێنرێتەوە")?;

        let buffer = MicBuffer::new();
        let buf_cb = buffer.clone();
        let processor = if noise.enabled {
            Some(Arc::new(Mutex::new(NoiseProcessor::new(noise.strength))))
        } else {
            None
        };
        let err_fn = |e| eprintln!("mic stream error: {e}");

        let stream = match supported.sample_format() {
            SampleFormat::F32 => {
                let cfg: StreamConfig = supported.clone().into();
                let channels = cfg.channels as usize;
                let sr = cfg.sample_rate.0;
                let proc = processor.clone();
                device.build_input_stream(
                    &cfg,
                    move |data: &[f32], _| {
                        let mut converted = to_stereo_i16_f32(data, channels, sr);
                        if let Some(p) = &proc {
                            if let Ok(mut g) = p.lock() {
                                g.process(&mut converted);
                            }
                        }
                        buf_cb.push_i16(&converted);
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::I16 => {
                let cfg: StreamConfig = supported.clone().into();
                let channels = cfg.channels as usize;
                let sr = cfg.sample_rate.0;
                let proc = processor.clone();
                device.build_input_stream(
                    &cfg,
                    move |data: &[i16], _| {
                        let mut converted = to_stereo_i16_i16(data, channels, sr);
                        if let Some(p) = &proc {
                            if let Ok(mut g) = p.lock() {
                                g.process(&mut converted);
                            }
                        }
                        buf_cb.push_i16(&converted);
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::U16 => {
                let cfg: StreamConfig = supported.clone().into();
                let channels = cfg.channels as usize;
                let sr = cfg.sample_rate.0;
                let proc = processor.clone();
                device.build_input_stream(
                    &cfg,
                    move |data: &[u16], _| {
                        let i16s: Vec<i16> = data
                            .iter()
                            .map(|s| ((*s as i32) - 32768) as i16)
                            .collect();
                        let mut converted = to_stereo_i16_i16(&i16s, channels, sr);
                        if let Some(p) = &proc {
                            if let Ok(mut g) = p.lock() {
                                g.process(&mut converted);
                            }
                        }
                        buf_cb.push_i16(&converted);
                    },
                    err_fn,
                    None,
                )?
            }
            other => bail!("فۆرماتی مایک پشتگیری ناکرێت: {other:?}"),
        };

        stream.play().context("نەتوانرا مایک دەست پێبکرێت")?;
        Ok(Self {
            _stream: stream,
            buffer,
            device_label,
        })
    }
}

fn to_stereo_i16_f32(data: &[f32], channels: usize, sample_rate: u32) -> Vec<i16> {
    let mut stereo = Vec::with_capacity(data.len().saturating_mul(2));
    if channels == 0 {
        return stereo;
    }
    for frame in data.chunks(channels) {
        let l = frame.first().copied().unwrap_or(0.0);
        let r = if channels > 1 {
            frame.get(1).copied().unwrap_or(l)
        } else {
            l
        };
        // Gentle gain so quiet laptop mics are audible.
        stereo.push(float_to_i16(l * 1.35));
        stereo.push(float_to_i16(r * 1.35));
    }
    resample_stereo_i16(&stereo, sample_rate, 48_000)
}

fn to_stereo_i16_i16(data: &[i16], channels: usize, sample_rate: u32) -> Vec<i16> {
    let mut stereo = Vec::with_capacity(data.len().saturating_mul(2));
    if channels == 0 {
        return stereo;
    }
    for frame in data.chunks(channels) {
        let l = frame.first().copied().unwrap_or(0);
        let r = if channels > 1 {
            frame.get(1).copied().unwrap_or(l)
        } else {
            l
        };
        stereo.push(gain_i16(l, 1.35));
        stereo.push(gain_i16(r, 1.35));
    }
    resample_stereo_i16(&stereo, sample_rate, 48_000)
}

fn float_to_i16(s: f32) -> i16 {
    (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

fn gain_i16(s: i16, gain: f32) -> i16 {
    let v = (s as f32 * gain).round();
    v.clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

fn resample_stereo_i16(input: &[i16], from: u32, to: u32) -> Vec<i16> {
    if from == 0 || to == 0 || input.len() < 2 {
        return input.to_vec();
    }
    if from == to {
        return input.to_vec();
    }
    let frames_in = input.len() / 2;
    let frames_out = (frames_in as u64 * to as u64 / from as u64) as usize;
    let mut out = Vec::with_capacity(frames_out.saturating_mul(2));
    for i in 0..frames_out {
        let src = i as f64 * from as f64 / to as f64;
        let idx = src.floor() as usize;
        let frac = src - idx as f64;
        let i0 = idx.min(frames_in.saturating_sub(1));
        let i1 = (idx + 1).min(frames_in.saturating_sub(1));
        for ch in 0..2 {
            let a = input[i0 * 2 + ch] as f64;
            let b = input[i1 * 2 + ch] as f64;
            out.push((a + (b - a) * frac) as i16);
        }
    }
    out
}
