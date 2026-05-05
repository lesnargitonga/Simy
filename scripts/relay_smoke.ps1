param(
    [string]$BaseUrl = "http://127.0.0.1:8081",
    [int]$MaxPollAttempts = 5,
    [int]$PollDelayMilliseconds = 300,
    [string]$OutputPath,
    [int]$MaxRetrieveLatencyMs = 3000,
    [int]$MaxTotalLatencyMs = 8000,
    [switch]$FailOnLatencySlo
)

$ErrorActionPreference = "Stop"
$scriptStopwatch = [System.Diagnostics.Stopwatch]::StartNew()

$events = New-Object System.Collections.ArrayList

function Add-SmokeEvent {
    param(
        [string]$Step,
        [string]$Status,
        [hashtable]$Details = @{}
    )

    $entry = [pscustomobject]@{
        time = (Get-Date).ToString("o")
        step = $Step
        status = $Status
        details = $Details
    }

    [void]$events.Add($entry)
}

function New-RandomTokenBase64 {
    $bytes = New-Object byte[] 32
    $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    $rng.GetBytes($bytes)
    $rng.Dispose()
    return [Convert]::ToBase64String($bytes)
}

$mailboxId = [guid]::NewGuid().ToString()
$mailboxToken = New-RandomTokenBase64
Add-SmokeEvent -Step "mailbox_prepare" -Status "ok" -Details @{ mailbox_id = $mailboxId }

$createBody = @{
    mailbox_id = $mailboxId
    mailbox_token_b64 = $mailboxToken
    codename = "SmokeRunner"
} | ConvertTo-Json

$null = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes" -Method Post -ContentType "application/json" -Body $createBody
Add-SmokeEvent -Step "mailbox_create" -Status "ok"

$plaintext = "relay smoke payload $(Get-Date -Format o)"
$ciphertextB64 = [Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes($plaintext))
$messageId = [guid]::NewGuid().ToString()

$postMessageBody = @{
    ciphertext_b64 = $ciphertextB64
    sender_device_hint = "smoke-script"
    ttl_seconds = 600
    message_id = $messageId
    replay_token = [guid]::NewGuid().ToString()
} | ConvertTo-Json

$null = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$mailboxId/messages" -Method Post -ContentType "application/json" -Body $postMessageBody
Add-SmokeEvent -Step "message_post" -Status "ok" -Details @{ message_id = $messageId }

$messages = @()
$found = $null
$retrieveStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
for ($i = 0; $i -lt $MaxPollAttempts; $i++) {
    $response = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$mailboxId/messages" -Headers @{ "x-mailbox-token" = $mailboxToken }

    if ($null -ne $response.messages) {
        $messages = @($response.messages)
    } elseif ($response -is [System.Array]) {
        $messages = @($response)
    } else {
        $messages = @($response)
    }

    $found = $messages | Where-Object { $_.message_id -eq $messageId }
    if ($found) {
        Add-SmokeEvent -Step "message_retrieve" -Status "ok" -Details @{ attempts = ($i + 1); retrieved_count = @($messages).Count }
        break
    }

    Start-Sleep -Milliseconds $PollDelayMilliseconds
}

if (-not $found) {
    Add-SmokeEvent -Step "message_retrieve" -Status "failed" -Details @{ attempts = $MaxPollAttempts; retrieved_count = @($messages).Count }
    throw "smoke test failed: posted message not found"
}

$retrieveStopwatch.Stop()
$retrieveLatencyMs = [int]$retrieveStopwatch.Elapsed.TotalMilliseconds
if ($retrieveLatencyMs -gt $MaxRetrieveLatencyMs) {
    Add-SmokeEvent -Step "latency_retrieve" -Status "warning" -Details @{ latency_ms = $retrieveLatencyMs; slo_ms = $MaxRetrieveLatencyMs }
    if ($FailOnLatencySlo) {
        throw "smoke test failed: retrieve latency ${retrieveLatencyMs}ms exceeded SLO ${MaxRetrieveLatencyMs}ms"
    }
} else {
    Add-SmokeEvent -Step "latency_retrieve" -Status "ok" -Details @{ latency_ms = $retrieveLatencyMs; slo_ms = $MaxRetrieveLatencyMs }
}

$health = Invoke-RestMethod -Uri "$BaseUrl/healthz"
if ($health.status -ne "ok") {
    Add-SmokeEvent -Step "health_check" -Status "failed" -Details @{ health = $health.status }
    throw "smoke test failed: health status is not ok"
}
Add-SmokeEvent -Step "health_check" -Status "ok" -Details @{ health = $health.status }

$scriptStopwatch.Stop()
$totalLatencyMs = [int]$scriptStopwatch.Elapsed.TotalMilliseconds
if ($totalLatencyMs -gt $MaxTotalLatencyMs) {
    Add-SmokeEvent -Step "latency_total" -Status "warning" -Details @{ latency_ms = $totalLatencyMs; slo_ms = $MaxTotalLatencyMs }
    if ($FailOnLatencySlo) {
        throw "smoke test failed: total latency ${totalLatencyMs}ms exceeded SLO ${MaxTotalLatencyMs}ms"
    }
} else {
    Add-SmokeEvent -Step "latency_total" -Status "ok" -Details @{ latency_ms = $totalLatencyMs; slo_ms = $MaxTotalLatencyMs }
}

$result = [pscustomobject]@{
    base_url = $BaseUrl
    generated_at = (Get-Date).ToString("o")
    health = $health.status
    mailbox_id = $mailboxId
    message_id = $messageId
    retrieved_count = @($messages).Count
    latency = [pscustomobject]@{
        retrieve_ms = $retrieveLatencyMs
        total_ms = $totalLatencyMs
    }
    slo = [pscustomobject]@{
        max_retrieve_ms = $MaxRetrieveLatencyMs
        max_total_ms = $MaxTotalLatencyMs
        fail_on_latency_slo = [bool]$FailOnLatencySlo
    }
    events = $events
}

$resultJson = $result | ConvertTo-Json -Depth 8

if (-not [string]::IsNullOrWhiteSpace($OutputPath)) {
    $outputDirectory = Split-Path -Parent $OutputPath
    if (-not [string]::IsNullOrWhiteSpace($outputDirectory)) {
        New-Item -Path $outputDirectory -ItemType Directory -Force | Out-Null
    }

    Set-Content -Path $OutputPath -Value $resultJson -Encoding UTF8
}

$resultJson
