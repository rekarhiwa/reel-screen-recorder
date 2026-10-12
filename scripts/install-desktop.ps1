# Quick install: copy app + create Desktop / Start Menu icons with app icon.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$exeSrc = Join-Path $root "target\release\screen-recorder.exe"
$icoSrc = Join-Path $root "assets\icon.ico"

if (-not (Test-Path $exeSrc)) {
  Write-Host "Building release..." -ForegroundColor Cyan
  Set-Location $root
  cargo build --release
  if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}

$dest = Join-Path $env:LOCALAPPDATA "REEL"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item $exeSrc (Join-Path $dest "screen-recorder.exe") -Force
Copy-Item $icoSrc (Join-Path $dest "icon.ico") -Force

$exe = Join-Path $dest "screen-recorder.exe"
$ico = Join-Path $dest "icon.ico"

function New-Shortcut([string]$Path, [string]$Target, [string]$Icon) {
  $w = New-Object -ComObject WScript.Shell
  $s = $w.CreateShortcut($Path)
  $s.TargetPath = $Target
  $s.WorkingDirectory = Split-Path $Target
  $s.IconLocation = "$Icon,0"
  $s.Description = "REEL Screen Recorder"
  $s.Save()
}

$desktop = [Environment]::GetFolderPath("Desktop")
$start = Join-Path ([Environment]::GetFolderPath("StartMenu")) "Programs\REEL"
New-Item -ItemType Directory -Force -Path $start | Out-Null

New-Shortcut (Join-Path $desktop "REEL.lnk") $exe $ico
New-Shortcut (Join-Path $start "REEL.lnk") $exe $ico

Write-Host "Installed to $dest" -ForegroundColor Green
Write-Host "Desktop icon: $(Join-Path $desktop 'REEL.lnk')"
Write-Host "Start Menu: $(Join-Path $start 'REEL.lnk')"
