# Build release + Inno Setup installer (desktop icon included).
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $root

Write-Host "==> cargo build --release" -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$exe = Join-Path $root "target\release\screen-recorder.exe"
if (-not (Test-Path $exe)) { throw "Missing $exe" }

$iscc = @(
  "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
  "${env:ProgramFiles}\Inno Setup 6\ISCC.exe",
  "${env:LOCALAPPDATA}\Programs\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $iscc) {
  throw "Inno Setup not found. Install with: winget install JRSoftware.InnoSetup"
}

New-Item -ItemType Directory -Force -Path (Join-Path $root "dist") | Out-Null

# Rebuild Kurdish RTF pages (Unicode escapes) so Inno wizard renders Arabic script
Write-Host "==> Building Kurdish RTF wizard pages" -ForegroundColor Cyan
$py = Join-Path $root "scripts\build-installer-pages.py"
python $py
if ($LASTEXITCODE -ne 0) { throw "Failed to build installer RTF pages" }

$issPath = Join-Path $root "installer\reel-setup.iss"
$issText = [System.IO.File]::ReadAllText($issPath)
[System.IO.File]::WriteAllText($issPath, $issText, (New-Object System.Text.UTF8Encoding $true))

Write-Host "==> Compiling installer with Inno Setup" -ForegroundColor Cyan
& $iscc $issPath
if ($LASTEXITCODE -ne 0) { throw "ISCC failed" }

$setup = Get-ChildItem (Join-Path $root "dist\REEL-Setup-*.exe") | Sort-Object LastWriteTime -Descending | Select-Object -First 1
Write-Host ""
Write-Host "Installer ready:" -ForegroundColor Green
Write-Host "  $($setup.FullName)"
Write-Host "Desktop icon is created during setup (checked by default)."
