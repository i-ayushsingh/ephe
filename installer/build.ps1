# builds a full release: cargo release build, then NSIS installer.
# produces:
#   release/ephe.exe          (the portable binary, with embedded icon)
#   release/ephe-portable.zip (the exe + Logo.png zipped)
#   release/ephe-setup.exe    (NSIS installer)
#   release/SHA256SUMS
# ponytail: no error handling, no progress noise. if something fails, cargo will yell.

$ErrorActionPreference = "Stop"

Set-Location $PSScriptRoot\..

Write-Host "==> cargo build --release"
cargo build --release

if (-not (Test-Path "installer\ephe.exe")) {
    Copy-Item "target\release\ephe.exe" "installer\ephe.exe" -Force
}
if (-not (Test-Path "installer\Logo.png")) {
    Copy-Item "Logo.png" "installer\Logo.png" -Force
}

Write-Host "==> makensis"
Set-Location installer
& "C:\Program Files (x86)\NSIS\makensis.exe" /INPUTCHARSET UTF8 /V2 ephe.nsi | Out-Null
Set-Location ..

Write-Host "==> packing portable"
New-Item -ItemType Directory -Path "release" -Force | Out-Null
Copy-Item "Logo.png" "release\Logo.png" -Force
Compress-Archive -Path "target\release\ephe.exe", "release\Logo.png" `
                 -DestinationPath "release\ephe-portable.zip" -Force

Write-Host "==> SHA256SUMS"
$sums = foreach ($f in @("ephe-setup.exe", "ephe-portable.zip")) {
    (Get-FileHash "release\$f" -Algorithm SHA256).Hash.ToLower() + "  " + $f
}
Set-Content -Path "release\SHA256SUMS" -Value ($sums -join "`r`n" + "`r`n") -Encoding utf8

Write-Host ""
Write-Host "==> release artifacts"
Get-ChildItem "release" | Format-Table Name, Length -AutoSize