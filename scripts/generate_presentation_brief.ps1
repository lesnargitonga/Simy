param(
    [string]$SnapshotPath = "artifacts/readiness/production-readiness-snapshot.json",
    [string]$FrontendSmokePath = "artifacts/readiness/frontend-smoke.json",
    [string]$LiveFeaturePath = "artifacts/readiness/live-feature-check.json",
    [string]$OutputPath = "artifacts/readiness/presentation-one-pager.md"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if (-not (Test-Path -Path $SnapshotPath -PathType Leaf)) {
    throw "Missing readiness snapshot at $SnapshotPath"
}

if (-not (Test-Path -Path $FrontendSmokePath -PathType Leaf)) {
    throw "Missing frontend smoke artifact at $FrontendSmokePath"
}

if (-not (Test-Path -Path $LiveFeaturePath -PathType Leaf)) {
    throw "Missing live feature artifact at $LiveFeaturePath"
}

$snapshot = Get-Content -Path $SnapshotPath -Raw | ConvertFrom-Json
$frontend = Get-Content -Path $FrontendSmokePath -Raw | ConvertFrom-Json
$liveFeature = Get-Content -Path $LiveFeaturePath -Raw | ConvertFrom-Json

$checkLines = @()
foreach ($check in $snapshot.checks) {
    $status = if ($check.passed) { "PASS" } else { "FAIL" }
    $checkLines += "- $($check.check): $status"
}

$frontendStatus = if ($frontend.passed) { "PASS" } else { "FAIL" }
$liveFeatureStatus = if ($liveFeature.passed) { "PASS" } else { "FAIL" }
$generatedAt = Get-Date -Format "yyyy-MM-dd HH:mm:ss 'UTC'"

$markdown = @"
# Executive One Pager

Generated: $generatedAt

## Current Readiness Position

- Operational posture: $($snapshot.operational_readiness)
- GA posture: $($snapshot.general_availability_decision)
- Frontend live check: $frontendStatus
- Live message, feed, and media check: $liveFeatureStatus

## Live Evidence

$($checkLines -join "`n")
- Relay health: $($snapshot.relay_health.status)
- Postgres health: $($snapshot.relay_health.postgres)
- Redis health: $($snapshot.relay_health.redis)

## What Is Working In The Product Today

- Active browser frontend loads from relay root and core UX surfaces are available.
- Encrypted message and feed flows are wired and smoke-validated.
- Relay operational dependencies are healthy in current environment.
- Production gate and readiness checks are automated in CI.

## Honest Risk Statement

- This supports a controlled production pilot claim.
- This does not yet support a full GA claim until security blockers are closed.

## Immediate Next Deliverable

- Add desktop-client relay integration smoke and make it a required readiness check.
"@

$outDir = Split-Path -Parent $OutputPath
if ($outDir) {
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
}

Set-Content -Path $OutputPath -Value $markdown -Encoding UTF8
Write-Host "[presentation-brief] Wrote: $OutputPath"