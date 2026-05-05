param(
    [switch]$RunRelaySmoke,
    [string]$RelayBaseUrl = "http://127.0.0.1:8081",
    [int]$RelayHealthTimeoutSeconds = 15,
    [switch]$SkipPreflight,
    [string]$EnvFile = ".env"
)

$ErrorActionPreference = "Stop"

if (-not $SkipPreflight) {
    Write-Host "[preflight] Running environment and toolchain checks"
    powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/preflight.ps1 -EnvFile $EnvFile
    if ($LASTEXITCODE -ne 0) {
        throw "preflight failed"
    }
}

function Wait-RelayHealth {
    param(
        [string]$BaseUrl,
        [int]$TimeoutSeconds
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        try {
            $health = Invoke-RestMethod -Uri "$BaseUrl/healthz" -Method Get
            if ($health.status -eq "ok") {
                return
            }
        } catch {
            # keep waiting while relay is booting
        }

        Start-Sleep -Milliseconds 400
    }

    throw "Relay health endpoint did not return ok within $TimeoutSeconds seconds at $BaseUrl/healthz"
}

Write-Host "[1/3] Running workspace tests"
cargo test
if ($LASTEXITCODE -ne 0) {
    throw "cargo test failed"
}

Write-Host "[2/3] Running ratchet file-store feature tests"
cargo test -p comm-core --features ratchet-store-fs ratchet_store_fs
if ($LASTEXITCODE -ne 0) {
    throw "feature tests failed"
}

if ($RunRelaySmoke) {
    Write-Host "[3/3] Running relay smoke test"
    powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/preflight.ps1 -SkipEnvValidation -CheckDependencyPorts
    if ($LASTEXITCODE -ne 0) {
        throw "dependency preflight failed"
    }

    Wait-RelayHealth -BaseUrl $RelayBaseUrl -TimeoutSeconds $RelayHealthTimeoutSeconds
    $pwshCommand = Get-Command pwsh -ErrorAction SilentlyContinue
    if ($pwshCommand) {
        & $pwshCommand.Source -NoProfile -File ./scripts/relay_smoke.ps1 -BaseUrl $RelayBaseUrl
    } else {
        powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/relay_smoke.ps1 -BaseUrl $RelayBaseUrl
    }
    if ($LASTEXITCODE -ne 0) {
        throw "relay smoke test failed"
    }
} else {
    Write-Host "[3/3] Relay smoke skipped (pass -RunRelaySmoke to enable)"
}

Write-Host "All selected tests passed."
