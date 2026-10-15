# Downloads the newest Microsoft Visual C++ x64 runtime (14.50+, needed by the
# VS18 builds of Apache and PHP) for the NSIS installer, and checks that it is
# signed by Microsoft.
$ErrorActionPreference = "Stop"
$dest = Join-Path $PSScriptRoot "..\packaging\windows\vcredist"
New-Item -ItemType Directory -Force $dest | Out-Null
$file = Join-Path $dest "vc_redist.x64.exe"
Invoke-WebRequest "https://aka.ms/vc14/vc_redist.x64.exe" -OutFile $file

$signature = Get-AuthenticodeSignature $file
if ($signature.Status -ne "Valid" -or $signature.SignerCertificate.Subject -notmatch "O=Microsoft Corporation") {
    Remove-Item $file
    throw "vc_redist.x64.exe is not validly signed by Microsoft ($($signature.Status))"
}
$version = (Get-Item $file).VersionInfo.ProductVersion
if ([version]$version -lt [version]"14.50") {
    throw "vc_redist.x64.exe is $version; the bundled servers need 14.50 or newer"
}
Write-Host "Downloaded vc_redist.x64.exe $version to $dest"
