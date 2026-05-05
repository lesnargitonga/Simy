param(
    [string]$BaseUrl = "http://127.0.0.1:8081",
    [string]$OutputPath = "artifacts/readiness/frontend-smoke.json",
    [int]$HealthTimeoutSeconds = 20
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Wait-RelayHealth {
    param(
        [string]$Url,
        [int]$TimeoutSeconds
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        try {
            $health = Invoke-RestMethod -Uri "$Url/healthz" -Method Get
            if ($health.status -eq "ok") {
                return $health
            }
        } catch {
            # relay may still be starting
        }

        Start-Sleep -Milliseconds 400
    }

    throw "Relay health endpoint did not return ok within $TimeoutSeconds seconds at $Url/healthz"
}

function New-CheckResult {
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

Write-Host "[frontend-smoke] Waiting for relay health"
$health = Wait-RelayHealth -Url $BaseUrl -TimeoutSeconds $HealthTimeoutSeconds

Write-Host "[frontend-smoke] Fetching root UI"
$response = Invoke-WebRequest -UseBasicParsing -Uri "$BaseUrl/"
$html = $response.Content

$requiredMarkers = @(
    'id="setup-screen"',
    'id="message-list"',
    'id="feed-list"',
    'function initializeWorkspace()',
    'function sendMessage()',
    'function postFeedEntry()',
    'function refreshMessages()',
    'function refreshFeed()'
)

$checks = New-Object System.Collections.Generic.List[object]
$checks.Add((New-CheckResult -Name "http_status" -Passed ($response.StatusCode -eq 200) -Details "status=$($response.StatusCode)"))
$checks.Add((New-CheckResult -Name "health_status" -Passed ($health.status -eq 'ok') -Details "health=$($health.status)"))

foreach ($marker in $requiredMarkers) {
    $checks.Add((New-CheckResult -Name "marker:$marker" -Passed ($html -match [regex]::Escape($marker)) -Details ($(if ($html -match [regex]::Escape($marker)) { 'present' } else { 'missing' }))))
}

$failed = @($checks | Where-Object { -not $_.passed })
$passed = ($failed.Count -eq 0)

$artifact = [PSCustomObject]@{
    generated_at = (Get-Date).ToString("o")
    base_url = $BaseUrl
    passed = $passed
    checks = $checks
}

$outDir = Split-Path -Parent $OutputPath
if ($outDir) {
    New-Item -ItemType Directory -Path $outDir -Force | Out-Null
}
$artifact | ConvertTo-Json -Depth 6 | Set-Content -Path $OutputPath -Encoding UTF8

Write-Host "[frontend-smoke] Wrote artifact: $OutputPath"

if (-not $passed) {
    throw "Frontend smoke failed; see $OutputPath"
}

Write-Host "[frontend-smoke] PASS"