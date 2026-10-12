# ڕیل — سکرین ڕیکۆردەر

ئەپێکی دێسکتۆپی کوردی بۆ تۆمارکردنی شاشە لە ویندۆز (MP4).

## دامەزراندن (Installer)

1. بیلدی ئەپ و سێتئەپ:
```powershell
.\scripts\build-installer.ps1
```
2. فایلەکە بکەوە: `dist\REEL-Setup-0.3.0.exe`
3. لە کاتی دامەزراندندا **ئایکۆنی Desktop** هەڵبژێردراوە — لەسەر شاشە دەردەکەوێت

یان خێرا بەبێ ویزارد:
```powershell
.\scripts\install-desktop.ps1
```

## کردنەوە (گەشەپێدان)

```powershell
.\target\release\screen-recorder.exe
```

یان:

```powershell
cargo run --release
```

## بەکارهێنان

1. مۆنیتەر هەڵبژێرە
2. دوگمەی تۆمار داگرە
3. دووبارە داگرە بۆ وەستان
4. فایل لە فۆڵدەری `recordings` پاشەکەوت دەبێت
