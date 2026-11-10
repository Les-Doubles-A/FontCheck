$ErrorActionPreference = 'Stop'

Write-Host "Compilation de FontCheck (Rust — Release ultra-optimisée)..." -ForegroundColor Cyan
cargo build --release

if (-not (Test-Path "dist")) {
    New-Item -ItemType Directory -Path "dist" | Out-Null
}

Copy-Item -Path "target\release\FontCheck.exe" -Destination "dist\FontCheck.exe" -Force
$sizeMB = [Math]::Round((Get-Item "dist\FontCheck.exe").Length / 1MB, 2)
Write-Host "Application créée : $PWD\dist\FontCheck.exe ($sizeMB Mo)" -ForegroundColor Green
