param(
    [string]$EnvFile = ".env",
    [switch]$StartDependencies,
    [switch]$WaitForHealth,
    [int]$HealthTimeoutSeconds = 30,
    [string]$HealthBaseUrl = "http://127.0.0.1:8081"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. "$PSScriptRoot/load_env.ps1"

if ($StartDependencies) {
    Write-Host "[run_relay] Starting dependencies with docker compose"
    docker compose up -d
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose up failed"
    }
}

Write-Host "[run_relay] Loading environment from $EnvFile"
$loaded = Import-SimyEnvFile -Path $EnvFile -RequireFile -ValidateRequired
Write-Host "[run_relay] Loaded $($loaded.Count) environment variables"

if ($WaitForHealth) {
    Write-Host "[run_relay] Waiting for dependencies to become reachable"
    $deadline = (Get-Date).AddSeconds($HealthTimeoutSeconds)
    $postgresReady = $false
    $redisReady = $false

    while ((Get-Date) -lt $deadline) {
        try {
            $pgTcp = New-Object Net.Sockets.TcpClient
            $pgTcp.Connect("127.0.0.1", 5432)
            $pgTcp.Dispose()
            $postgresReady = $true
        } catch {
            $postgresReady = $false
        }

        try {
            $redisTcp = New-Object Net.Sockets.TcpClient
            $redisTcp.Connect("127.0.0.1", 6379)
            $redisTcp.Dispose()
            $redisReady = $true
        } catch {
            $redisReady = $false
        }

        if ($postgresReady -and $redisReady) {
            break
        }

        Start-Sleep -Milliseconds 400
    }

    if (-not ($postgresReady -and $redisReady)) {
        throw "Dependencies are not reachable on 127.0.0.1:5432 and 127.0.0.1:6379"
    }
}

Write-Host "[run_relay] Starting relay"
cargo run -p relay
if ($LASTEXITCODE -ne 0) {
    throw "relay exited with code $LASTEXITCODE"
}
