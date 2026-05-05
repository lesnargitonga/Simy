param(
    [string]$EnvFile = ".env",
    [string]$RelayBaseUrl = "http://127.0.0.1:8081",
    [int]$RelayHealthTimeoutSeconds = 45,
    [switch]$RunSmoke,
    [switch]$OpenUi,
    [switch]$SkipPreflight,
    [switch]$StrictSecrets
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. "$PSScriptRoot/load_env.ps1"

$relayRootUrl = $RelayBaseUrl.TrimEnd("/")
$relayHealthUrl = "$relayRootUrl/healthz"

function Test-RelayHealthy {
    param(
        [string]$BaseUrl
    )

    try {
        $health = Invoke-RestMethod -Uri ($BaseUrl.TrimEnd("/") + "/healthz") -Method Get -TimeoutSec 3
        return $health.status -eq "ok"
    } catch {
        return $false
    }
}

function Wait-RelayHealthy {
    param(
        [string]$BaseUrl,
        [int]$TimeoutSeconds,
        [System.Diagnostics.Process]$Process,
        [string]$StdOutLog,
        [string]$StdErrLog
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if (Test-RelayHealthy -BaseUrl $BaseUrl) {
            return
        }

        if ($Process -and $Process.HasExited) {
            $stdoutTail = @()
            $stderrTail = @()

            if (Test-Path -LiteralPath $StdOutLog) {
                $stdoutTail = Get-Content -LiteralPath $StdOutLog -Tail 20
            }

            if (Test-Path -LiteralPath $StdErrLog) {
                $stderrTail = Get-Content -LiteralPath $StdErrLog -Tail 20
            }

            throw (
                "Relay process exited before health check passed. ExitCode=$($Process.ExitCode)`n" +
                "stdout:`n$($stdoutTail -join [Environment]::NewLine)`n" +
                "stderr:`n$($stderrTail -join [Environment]::NewLine)"
            )
        }

        Start-Sleep -Milliseconds 500
    }

    throw "Relay health endpoint did not return ok within $TimeoutSeconds seconds at $($BaseUrl.TrimEnd('/'))/healthz"
}

Write-Host "[start_all] Loading environment from $EnvFile"
$loaded = Import-SimyEnvFile -Path $EnvFile -RequireFile -ValidateRequired
Write-Host "[start_all] Loaded $($loaded.Count) environment variables"

if (-not $SkipPreflight) {
    Write-Host "[start_all] Running preflight checks"
    $preflightArgs = @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", (Join-Path $PSScriptRoot "preflight.ps1"),
        "-EnvFile", $EnvFile
    )

    if ($StrictSecrets) {
        $preflightArgs += "-EnforceNonDefaultAdminToken"
    }

    & powershell @preflightArgs
    if ($LASTEXITCODE -ne 0) {
        throw "preflight failed"
    }
}

Write-Host "[start_all] Starting Docker dependencies"
docker compose up -d
if ($LASTEXITCODE -ne 0) {
    throw "docker compose up failed"
}

if (Test-RelayHealthy -BaseUrl $relayRootUrl) {
    Write-Host "[start_all] Relay already healthy at $relayRootUrl"
} else {
    $logDir = Join-Path $PSScriptRoot "..\artifacts\start_all"
    $null = New-Item -ItemType Directory -Force -Path $logDir

    $stdoutLog = Join-Path $logDir "relay.stdout.log"
    $stderrLog = Join-Path $logDir "relay.stderr.log"

    if (Test-Path -LiteralPath $stdoutLog) {
        Remove-Item -LiteralPath $stdoutLog -Force
    }

    if (Test-Path -LiteralPath $stderrLog) {
        Remove-Item -LiteralPath $stderrLog -Force
    }

    $runRelayScript = Join-Path $PSScriptRoot "run_relay.ps1"
    $argumentList = @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", ('"' + $runRelayScript + '"'),
        "-EnvFile", ('"' + $EnvFile + '"')
    )

    Write-Host "[start_all] Launching relay in background"
    $relayProcess = Start-Process `
        -FilePath "powershell" `
        -ArgumentList $argumentList `
        -RedirectStandardOutput $stdoutLog `
        -RedirectStandardError $stderrLog `
        -PassThru `
        -WindowStyle Hidden

    Wait-RelayHealthy `
        -BaseUrl $relayRootUrl `
        -TimeoutSeconds $RelayHealthTimeoutSeconds `
        -Process $relayProcess `
        -StdOutLog $stdoutLog `
        -StdErrLog $stderrLog

    Write-Host "[start_all] Relay started successfully (PID $($relayProcess.Id))"
    Write-Host "[start_all] Relay logs: $stdoutLog"
}

if ($RunSmoke) {
    Write-Host "[start_all] Running relay smoke test"
    powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "relay_smoke.ps1") -BaseUrl $relayRootUrl
    if ($LASTEXITCODE -ne 0) {
        throw "relay smoke test failed"
    }
}

$health = Invoke-RestMethod -Uri $relayHealthUrl -Method Get
Write-Host "[start_all] System ready"
Write-Host "[start_all] UI URL: $relayRootUrl/"
Write-Host "[start_all] Health URL: $relayHealthUrl"

if ($OpenUi) {
    Write-Host "[start_all] Opening browser UI"
    Start-Process $relayRootUrl | Out-Null
}

$health | ConvertTo-Json -Depth 5
exit 0