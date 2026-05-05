param(
    [string]$BaseUrl = "http://127.0.0.1:8081",
    [string]$OutputPath = "artifacts/readiness/live-feature-check.json"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function New-TokenB64 {
    $bytes = New-Object byte[] 32
    [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    return [Convert]::ToBase64String($bytes)
}

function New-CiphertextB64 {
    param([string]$Text)
    return [Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes($Text))
}

function Sha256B64 {
    param([byte[]]$Bytes)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $hash = $sha.ComputeHash($Bytes)
        return [Convert]::ToBase64String($hash)
    } finally {
        $sha.Dispose()
    }
}

function Get-RelayAdminToken {
    $token = [Environment]::GetEnvironmentVariable("RELAY_ADMIN_TOKEN")
    if (-not [string]::IsNullOrWhiteSpace($token)) {
        return $token
    }

    $envPath = ".env"
    if (-not (Test-Path -Path $envPath -PathType Leaf)) {
        return $null
    }

    $line = Get-Content -Path $envPath | Where-Object { $_ -match '^\s*RELAY_ADMIN_TOKEN\s*=' } | Select-Object -First 1
    if (-not $line) {
        return $null
    }

    $parsed = ($line -split '=', 2)[1].Trim()
    if ($parsed.StartsWith('"') -and $parsed.EndsWith('"') -and $parsed.Length -ge 2) {
        $parsed = $parsed.Substring(1, $parsed.Length - 2)
    }

    return $parsed
}

function Add-Check {
    param(
        [System.Collections.Generic.List[object]]$Checks,
        [string]$Name,
        [bool]$Passed,
        [string]$Details
    )

    $Checks.Add([PSCustomObject]@{
        check = $Name
        passed = $Passed
        details = $Details
    }) | Out-Null
}

    function ConvertTo-HeaderMap {
        param([object]$InputObject)

        $map = @{}
        if ($null -eq $InputObject) {
            return $map
        }

        foreach ($prop in $InputObject.PSObject.Properties) {
            $map[$prop.Name] = [string]$prop.Value
        }

        return $map
    }

$checks = New-Object System.Collections.Generic.List[object]

try {
    $adminToken = Get-RelayAdminToken
    if ([string]::IsNullOrWhiteSpace($adminToken)) {
        throw "RELAY_ADMIN_TOKEN is required for live feature check"
    }

    $health = Invoke-RestMethod -Uri "$BaseUrl/healthz" -Method Get
    Add-Check -Checks $checks -Name "health" -Passed ($health.status -eq "ok") -Details "status=$($health.status)"

    $adminMailbox = [guid]::NewGuid().ToString()
    $adminMailboxToken = New-TokenB64
    $mailboxA = [guid]::NewGuid().ToString()
    $mailboxAToken = New-TokenB64
    $mailboxB = [guid]::NewGuid().ToString()
    $mailboxBToken = New-TokenB64

    $adminCreate = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes" -Method Post -Headers @{ "x-admin-token" = $adminToken } -ContentType "application/json" -Body (@{
        mailbox_id = $adminMailbox
        mailbox_token_b64 = $adminMailboxToken
        codename = "Root-Admin"
    } | ConvertTo-Json)
    Add-Check -Checks $checks -Name "admin_mailbox_create" -Passed ($null -ne $adminCreate) -Details "ok"

    $adminBootstrap = Invoke-RestMethod -Uri "$BaseUrl/v1/admin/bootstrap-account" -Method Post -Headers @{
        "x-admin-token" = $adminToken
        "x-mailbox-token" = $adminMailboxToken
    } -ContentType "application/json" -Body (@{
        mailbox_id = $adminMailbox
        mailbox_token_b64 = $adminMailboxToken
        codename = "Root-Admin"
    } | ConvertTo-Json)
    Add-Check -Checks $checks -Name "admin_bootstrap" -Passed ($adminBootstrap.role -eq "admin") -Details "role=$($adminBootstrap.role)"

    $aCreate = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes" -Method Post -Headers @{ 
        "x-origin-mailbox-id" = $adminMailbox
        "x-origin-mailbox-token" = $adminMailboxToken
    } -ContentType "application/json" -Body (@{
        mailbox_id = $mailboxA
        mailbox_token_b64 = $mailboxAToken
        codename = "Pilot-A"
    } | ConvertTo-Json)
    Add-Check -Checks $checks -Name "mailbox_a_create" -Passed ($null -ne $aCreate) -Details "ok"

    $bCreate = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes" -Method Post -Headers @{ 
        "x-origin-mailbox-id" = $mailboxA
        "x-origin-mailbox-token" = $mailboxAToken
    } -ContentType "application/json" -Body (@{
        mailbox_id = $mailboxB
        mailbox_token_b64 = $mailboxBToken
        codename = "Pilot-B"
    } | ConvertTo-Json)
    Add-Check -Checks $checks -Name "mailbox_b_create" -Passed ($null -ne $bCreate) -Details "ok"

    $null = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$mailboxA/contacts" -Method Post -Headers @{ "x-mailbox-token" = $mailboxAToken } -ContentType "application/json" -Body (@{
        contact_mailbox_id = $mailboxB
        codename = "Contact-B"
    } | ConvertTo-Json)
    Add-Check -Checks $checks -Name "contact_link" -Passed $true -Details "A->B linked"

    $messageId = [guid]::NewGuid().ToString()
    $null = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$mailboxA/messages" -Method Post -ContentType "application/json" -Body (@{
        ciphertext_b64 = (New-CiphertextB64 -Text "hello from live feature check")
        sender_device_hint = "device-live-check"
        ttl_seconds = 600
        message_id = $messageId
        replay_token = ([guid]::NewGuid().ToString())
    } | ConvertTo-Json)

    $retrievedA = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$mailboxA/messages" -Method Get -Headers @{ "x-mailbox-token" = $mailboxAToken }
    $messages = @($retrievedA.messages)
    if ($messages.Count -eq 0 -and $retrievedA.message_id) {
        $messages = @($retrievedA)
    }
    $messageFound = @($messages | Where-Object { $_.message_id -eq $messageId }).Count -gt 0
    Add-Check -Checks $checks -Name "message_round_trip" -Passed $messageFound -Details "message_id=$messageId"

    $postId = [guid]::NewGuid().ToString()
    $feedBody = @{
        post_id = $postId
        audience = "contacts"
        reply_policy = "contacts_only"
        author_ciphertext_b64 = (New-CiphertextB64 -Text "author feed payload")
        deliveries = @(
            @{
                recipient_mailbox_id = $mailboxB
                ciphertext_b64 = (New-CiphertextB64 -Text "recipient feed payload")
            }
        )
        ttl_seconds = 600
    } | ConvertTo-Json -Depth 6
    $null = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$mailboxA/feed/posts" -Method Post -Headers @{ "x-mailbox-token" = $mailboxAToken } -ContentType "application/json" -Body $feedBody

    $feedA = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$mailboxA/feed/posts" -Method Get -Headers @{ "x-mailbox-token" = $mailboxAToken }
    $feedPostsA = @($feedA.posts)
    if ($feedPostsA.Count -eq 0 -and $feedA.post_id) {
        $feedPostsA = @($feedA)
    }
    $feedPostFoundA = @($feedPostsA | Where-Object { $_.post_id -eq $postId }).Count -gt 0

    $feedB = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$mailboxB/feed/posts" -Method Get -Headers @{ "x-mailbox-token" = $mailboxBToken }
    $feedPostsB = @($feedB.posts)
    if ($feedPostsB.Count -eq 0 -and $feedB.post_id) {
        $feedPostsB = @($feedB)
    }
    $feedPostFoundB = @($feedPostsB | Where-Object { $_.post_id -eq $postId }).Count -gt 0
    Add-Check -Checks $checks -Name "feed_post_visible_author" -Passed $feedPostFoundA -Details "post_id=$postId"
    Add-Check -Checks $checks -Name "feed_post_visible_recipient" -Passed $feedPostFoundB -Details "post_id=$postId"

    $cipherBytes = New-Object byte[] 512
    [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($cipherBytes)
    $cipherHashB64 = Sha256B64 -Bytes $cipherBytes

    $intent = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$mailboxA/media/upload-intents" -Method Post -Headers @{ "x-mailbox-token" = $mailboxAToken } -ContentType "application/json" -Body (@{
        media_type = "image/jpeg"
        original_size_bytes = 512
        content_sha256_b64 = $cipherHashB64
    } | ConvertTo-Json)

        $uploadHeaders = ConvertTo-HeaderMap -InputObject $intent.presigned_upload.headers
        $uploadResponse = Invoke-WebRequest -UseBasicParsing -Uri $intent.presigned_upload.url -Method $intent.presigned_upload.method -Headers $uploadHeaders -Body $cipherBytes
    Add-Check -Checks $checks -Name "media_upload" -Passed ($uploadResponse.StatusCode -ge 200 -and $uploadResponse.StatusCode -lt 300) -Details "status=$($uploadResponse.StatusCode)"

    $manifest = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$mailboxA/media/manifests" -Method Post -Headers @{ "x-mailbox-token" = $mailboxAToken } -ContentType "application/json" -Body (@{
        intent_id = $intent.intent_id
        content_sha256_b64 = $cipherHashB64
    } | ConvertTo-Json)
    Add-Check -Checks $checks -Name "media_manifest" -Passed ($null -ne $manifest.object_id) -Details "object_id=$($manifest.object_id)"

    $grant = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$mailboxA/media/access-grants" -Method Post -Headers @{ "x-mailbox-token" = $mailboxAToken } -ContentType "application/json" -Body (@{
        object_key = $manifest.object_key
        operation = "download"
        ttl_seconds = 600
    } | ConvertTo-Json)

    $download = Invoke-WebRequest -UseBasicParsing -Uri "$BaseUrl/v1/media/access-grants/$($grant.grant_token)/content" -Method Get
    $downloadBytes = $download.Content
    if ($downloadBytes -is [string]) {
        $downloadBytes = [System.Text.Encoding]::UTF8.GetBytes($downloadBytes)
    }

    $downloadHash = Sha256B64 -Bytes $downloadBytes
    $mediaRoundTripOk = ($downloadHash -eq $cipherHashB64)
    Add-Check -Checks $checks -Name "media_download_integrity" -Passed $mediaRoundTripOk -Details "hash_match=$mediaRoundTripOk"

    $failed = @($checks | Where-Object { -not $_.passed })
    $artifact = [PSCustomObject]@{
        generated_at = (Get-Date).ToString("o")
        base_url = $BaseUrl
        passed = ($failed.Count -eq 0)
        checks = $checks
        mailbox_a = $mailboxA
        mailbox_b = $mailboxB
    }

    $outDir = Split-Path -Parent $OutputPath
    if ($outDir) {
        New-Item -ItemType Directory -Force -Path $outDir | Out-Null
    }
    $artifact | ConvertTo-Json -Depth 7 | Set-Content -Path $OutputPath -Encoding UTF8

    Write-Host "[live-feature-check] Wrote artifact: $OutputPath"
    if (-not $artifact.passed) {
        throw "live feature check failed; see $OutputPath"
    }
    Write-Host "[live-feature-check] PASS"
}
catch {
    $artifact = [PSCustomObject]@{
        generated_at = (Get-Date).ToString("o")
        base_url = $BaseUrl
        passed = $false
        error = $_.Exception.Message
        checks = $checks
    }

    $outDir = Split-Path -Parent $OutputPath
    if ($outDir) {
        New-Item -ItemType Directory -Force -Path $outDir | Out-Null
    }
    $artifact | ConvertTo-Json -Depth 7 | Set-Content -Path $OutputPath -Encoding UTF8
    throw
}