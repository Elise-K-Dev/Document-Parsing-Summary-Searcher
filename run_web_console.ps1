$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$backendDir = Join-Path $repoRoot "code\evidence_backend"
$cargoExe = "C:\Users\Elise\.cargo\bin\cargo.exe"
$builtExe = Join-Path $backendDir "target\debug\evidence_backend.exe"
$bind = "127.0.0.1:8080"

if (Get-NetTCPConnection -LocalPort 8080 -State Listen -ErrorAction SilentlyContinue) {
    Write-Output "Port 8080 is already in use. Open http://$bind/ if that is the web console you want."
    exit 0
}

Set-Location $backendDir

if (Test-Path -LiteralPath $builtExe) {
    Write-Output "Starting existing backend binary at http://$bind/"
    & $builtExe serve --bind $bind
}
else {
    if (-not (Test-Path -LiteralPath $cargoExe)) {
        throw "Cargo not found: $cargoExe"
    }

    Write-Output "Building and starting backend at http://$bind/"
    & $cargoExe run --offline -- serve --bind $bind
}
