# Downloads the latest Microsoft Visual C++ x64 redistributable for the NSIS installer.
$ErrorActionPreference = "Stop"
$dest = Join-Path $PSScriptRoot "..\packaging\windows\vcredist"
New-Item -ItemType Directory -Force $dest | Out-Null
Invoke-WebRequest "https://aka.ms/vs/17/release/vc_redist.x64.exe" -OutFile (Join-Path $dest "vc_redist.x64.exe")
Write-Host "Downloaded vc_redist.x64.exe to $dest"
