param(
    [string]$OutputDir = "artifacts/readiness",
    [switch]$RunFullValidation,
    [switch]$SkipProductionGate,
    [switch]$SkipHealthCheck,
    [string]$RelayBaseUrl = "http://127.0.0.1:8081",
    [int]$RelayHealthTimeoutSeconds = 20
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function New-Result {
    param(
        [string]$Name,
        [bool]$Passed,
        [string]$Details
    )

    [PSCustomObject]@{
        check = $Name
        passed = $Passed
        details = $Details
    }
}

function Invoke-Check {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [scriptblock]$Body
    )

    try {
        & $Body | Out-Null
        return New-Result -Name $Name -Passed $true -Details "ok"
    } catch {
        return New-Result -Name $Name -Passed $false -Details $_.Exception.Message
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
                return $health
            }
        } catch {
            # relay may still be booting
        }
        Start-Sleep -Milliseconds 400
    }

    throw "Relay health endpoint did not return ok within $TimeoutSeconds seconds at $BaseUrl/healthz"
}

Write-Host "[readiness] Starting production readiness snapshot"

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$startedAt = Get-Date
$checks = New-Object System.Collections.Generic.List[object]
$healthPayload = $null

if (-not $SkipProductionGate) {
    Write-Host "[readiness] Running production gate"
    $checks.Add((Invoke-Check -Name "production_gate" -Body {
        powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/production_gate.ps1
        if ($LASTEXITCODE -ne 0) {
            throw "production gate failed"
        }
    }))
}

if ($RunFullValidation) {
    Write-Host "[readiness] Running full validation suite"
    $checks.Add((Invoke-Check -Name "test_all_with_relay_smoke" -Body {
        powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/test_all.ps1 -RunRelaySmoke -RelayHealthTimeoutSeconds $RelayHealthTimeoutSeconds
        if ($LASTEXITCODE -ne 0) {
            throw "test_all failed"
        }
    }))
}

Write-Host "[readiness] Running frontend smoke check"
$checks.Add((Invoke-Check -Name "frontend_smoke" -Body {
    powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/frontend_smoke.ps1 -BaseUrl $RelayBaseUrl -OutputPath (Join-Path $OutputDir "frontend-smoke.json") -HealthTimeoutSeconds $RelayHealthTimeoutSeconds
    if ($LASTEXITCODE -ne 0) {
        throw "frontend smoke failed"
    }
}))

Write-Host "[readiness] Running live feature check"
$checks.Add((Invoke-Check -Name "live_feature_check" -Body {
    powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/live_feature_check.ps1 -BaseUrl $RelayBaseUrl -OutputPath (Join-Path $OutputDir "live-feature-check.json")
    if ($LASTEXITCODE -ne 0) {
        throw "live feature check failed"
    }
}))

Write-Host "[readiness] Running managed user flow check"
$checks.Add((Invoke-Check -Name "managed_user_flow_check" -Body {
    powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/managed_user_flow_check.ps1 -BaseUrl $RelayBaseUrl -OutputPath (Join-Path $OutputDir "managed-user-flow-check.json")
    if ($LASTEXITCODE -ne 0) {
        throw "managed user flow check failed"
    }
}))

if (-not $SkipHealthCheck) {
    Write-Host "[readiness] Checking relay health"
    $checks.Add((Invoke-Check -Name "relay_health" -Body {
        $script:healthPayload = Wait-RelayHealth -BaseUrl $RelayBaseUrl -TimeoutSeconds $RelayHealthTimeoutSeconds
    }))
}

$failedChecks = @()
foreach ($check in $checks) {
    if (($check.PSObject.Properties.Name -contains "passed") -and (-not $check.passed)) {
        $failedChecks += $check
    }
}
$opsReady = ($failedChecks.Count -eq 0)

$securityGateSummary = @(
    "Independent cryptography and protocol review",
    "Reproducible signed releases and verified update chain",
    "Complete device trust and revocation lifecycle",
    "Incident response and key-compromise runbook exercised"
)

$releaseClass = if ($opsReady) { "Operationally ready for controlled production pilot" } else { "Not ready" }
$gaDecision = "Not approved for general availability until production-readiness blockers are fully closed"

$endedAt = Get-Date
$durationSeconds = [int]([math]::Round(($endedAt - $startedAt).TotalSeconds, 0))

$snapshot = [PSCustomObject]@{
    generated_at = $endedAt.ToString("o")
    duration_seconds = $durationSeconds
    relay_base_url = $RelayBaseUrl
    operational_readiness = $releaseClass
    general_availability_decision = $gaDecision
    checks = $checks
    frontend_smoke_artifact = (Join-Path $OutputDir "frontend-smoke.json")
    live_feature_artifact = (Join-Path $OutputDir "live-feature-check.json")
    managed_user_flow_artifact = (Join-Path $OutputDir "managed-user-flow-check.json")
    relay_health = $healthPayload
    security_blockers = $securityGateSummary
}

$jsonPath = Join-Path $OutputDir "production-readiness-snapshot.json"
$mdPath = Join-Path $OutputDir "production-readiness-snapshot.md"

$snapshot | ConvertTo-Json -Depth 6 | Set-Content -Path $jsonPath -Encoding UTF8

$checkLines = @()
foreach ($check in $checks) {
    $status = if ($check.passed) { "PASS" } else { "FAIL" }
    $checkLines += "| $($check.check) | $status | $($check.details) |"
}

$securityLines = @()
foreach ($item in $securityGateSummary) {
    $securityLines += "- $item"
}

$healthSummary = if ($healthPayload) {
    "- Relay health status: $($healthPayload.status)"
} else {
    "- Relay health status: not captured"
}

$markdown = @"
# Secure Communications Production Readiness Snapshot

Generated at: $($endedAt.ToString("u"))

## Executive Summary

- Operational readiness classification: $releaseClass
- General availability decision: $gaDecision
- Snapshot duration: $durationSeconds seconds

## Evidence

| Check | Status | Details |
|---|---|---|
$($checkLines -join "`n")

$healthSummary

## Security And Release Blockers For GA

$($securityLines -join "`n")

## Presentation Guidance

- This snapshot supports a readiness claim for controlled pilot use only when all evidence checks pass.
- This snapshot does not by itself authorize a full production GA claim.
"@

Set-Content -Path $mdPath -Value $markdown -Encoding UTF8

Write-Host "[readiness] Wrote snapshot: $mdPath"
Write-Host "[readiness] Wrote snapshot: $jsonPath"

if (-not $opsReady) {
    throw "One or more readiness checks failed"
}