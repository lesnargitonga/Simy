param(
    [string]$BaseUrl = "http://127.0.0.1:8081",
    [string]$OutputPath = "artifacts/readiness/managed-user-flow-check.json"
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

$checks = New-Object System.Collections.Generic.List[object]

try {
    $adminToken = Get-RelayAdminToken
    if ([string]::IsNullOrWhiteSpace($adminToken)) {
        throw "RELAY_ADMIN_TOKEN is required for managed user flow check"
    }

    $health = Invoke-RestMethod -Uri "$BaseUrl/healthz" -Method Get
    Add-Check -Checks $checks -Name "health" -Passed ($health.status -eq "ok") -Details "status=$($health.status)"

    $adminMailboxId = [guid]::NewGuid().ToString()
    $adminMailboxToken = New-TokenB64
    $managedCodename = "Managed-" + ([guid]::NewGuid().ToString().Substring(0, 8))

    $adminCreate = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes" -Method Post -Headers @{ "x-admin-token" = $adminToken } -ContentType "application/json" -Body (@{
        mailbox_id = $adminMailboxId
        mailbox_token_b64 = $adminMailboxToken
        codename = "Admin-Flow"
    } | ConvertTo-Json)
    Add-Check -Checks $checks -Name "admin_mailbox_create" -Passed ($null -ne $adminCreate) -Details "ok"

    $bootstrap = Invoke-RestMethod -Uri "$BaseUrl/v1/admin/bootstrap-account" -Method Post -Headers @{
        "x-admin-token" = $adminToken
        "x-mailbox-token" = $adminMailboxToken
    } -ContentType "application/json" -Body (@{
        mailbox_id = $adminMailboxId
        mailbox_token_b64 = $adminMailboxToken
        codename = "Admin-Flow"
    } | ConvertTo-Json)
    Add-Check -Checks $checks -Name "admin_bootstrap" -Passed ($bootstrap.role -eq "admin") -Details "role=$($bootstrap.role)"

    $provision = Invoke-RestMethod -Uri "$BaseUrl/v1/admin/provision-mailbox" -Method Post -Headers @{
        "x-admin-token" = $adminToken
        "x-admin-mailbox-id" = $adminMailboxId
        "x-mailbox-token" = $adminMailboxToken
    }

    $managedMailboxId = $provision.mailbox_id
    $managedMailboxToken = $provision.mailbox_token_b64
    Add-Check -Checks $checks -Name "managed_user_provision" -Passed (![string]::IsNullOrWhiteSpace($managedMailboxId) -and ![string]::IsNullOrWhiteSpace($managedMailboxToken)) -Details "mailbox_id=$managedMailboxId"

    $managedActivate = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes" -Method Post -ContentType "application/json" -Body (@{
        mailbox_id = $managedMailboxId
        mailbox_token_b64 = $managedMailboxToken
        codename = $managedCodename
    } | ConvertTo-Json)
    Add-Check -Checks $checks -Name "managed_user_activate" -Passed ($managedActivate.role -eq "user") -Details "role=$($managedActivate.role),status=$($managedActivate.status)"

    $null = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$adminMailboxId/contacts" -Method Post -Headers @{ "x-mailbox-token" = $adminMailboxToken } -ContentType "application/json" -Body (@{
        contact_mailbox_id = $managedMailboxId
        codename = $managedCodename
    } | ConvertTo-Json)

    $null = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$managedMailboxId/contacts" -Method Post -Headers @{ "x-mailbox-token" = $managedMailboxToken } -ContentType "application/json" -Body (@{
        contact_mailbox_id = $adminMailboxId
        codename = "Admin-Flow"
    } | ConvertTo-Json)

    Add-Check -Checks $checks -Name "contacts_bidirectional" -Passed $true -Details "admin<->managed"

    $adminMessageId = [guid]::NewGuid().ToString()
    $null = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$managedMailboxId/messages" -Method Post -ContentType "application/json" -Body (@{
        ciphertext_b64 = (New-CiphertextB64 -Text "admin-to-managed")
        sender_device_hint = "admin-device"
        ttl_seconds = 600
        message_id = $adminMessageId
        replay_token = ([guid]::NewGuid().ToString())
    } | ConvertTo-Json)

    $managedMessages = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$managedMailboxId/messages" -Method Get -Headers @{ "x-mailbox-token" = $managedMailboxToken }
    $managedList = @($managedMessages.messages)
    if ($managedList.Count -eq 0 -and $managedMessages.message_id) {
        $managedList = @($managedMessages)
    }
    $adminToManagedOk = @($managedList | Where-Object { $_.message_id -eq $adminMessageId }).Count -gt 0

    $managedMessageId = [guid]::NewGuid().ToString()
    $null = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$adminMailboxId/messages" -Method Post -ContentType "application/json" -Body (@{
        ciphertext_b64 = (New-CiphertextB64 -Text "managed-to-admin")
        sender_device_hint = "managed-device"
        ttl_seconds = 600
        message_id = $managedMessageId
        replay_token = ([guid]::NewGuid().ToString())
    } | ConvertTo-Json)

    $adminMessages = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$adminMailboxId/messages" -Method Get -Headers @{ "x-mailbox-token" = $adminMailboxToken }
    $adminList = @($adminMessages.messages)
    if ($adminList.Count -eq 0 -and $adminMessages.message_id) {
        $adminList = @($adminMessages)
    }
    $managedToAdminOk = @($adminList | Where-Object { $_.message_id -eq $managedMessageId }).Count -gt 0

    Add-Check -Checks $checks -Name "messages_bidirectional" -Passed ($adminToManagedOk -and $managedToAdminOk) -Details "admin_to_managed=$adminToManagedOk,managed_to_admin=$managedToAdminOk"

    $postId = [guid]::NewGuid().ToString()
    $null = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$adminMailboxId/feed/posts" -Method Post -Headers @{ "x-mailbox-token" = $adminMailboxToken } -ContentType "application/json" -Body (@{
        post_id = $postId
        audience = "contacts"
        reply_policy = "contacts_only"
        author_ciphertext_b64 = (New-CiphertextB64 -Text "admin feed payload")
        deliveries = @(
            @{
                recipient_mailbox_id = $managedMailboxId
                ciphertext_b64 = (New-CiphertextB64 -Text "managed recipient payload")
            }
        )
        ttl_seconds = 600
    } | ConvertTo-Json -Depth 6)

    $adminFeed = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$adminMailboxId/feed/posts" -Method Get -Headers @{ "x-mailbox-token" = $adminMailboxToken }
    $managedFeed = Invoke-RestMethod -Uri "$BaseUrl/v1/mailboxes/$managedMailboxId/feed/posts" -Method Get -Headers @{ "x-mailbox-token" = $managedMailboxToken }

    $adminPosts = @($adminFeed.posts)
    if ($adminPosts.Count -eq 0 -and $adminFeed.post_id) {
        $adminPosts = @($adminFeed)
    }

    $managedPosts = @($managedFeed.posts)
    if ($managedPosts.Count -eq 0 -and $managedFeed.post_id) {
        $managedPosts = @($managedFeed)
    }

    $adminSeesPost = @($adminPosts | Where-Object { $_.post_id -eq $postId }).Count -gt 0
    $managedSeesPost = @($managedPosts | Where-Object { $_.post_id -eq $postId }).Count -gt 0

    Add-Check -Checks $checks -Name "feed_visibility_admin_and_managed" -Passed ($adminSeesPost -and $managedSeesPost) -Details "admin=$adminSeesPost,managed=$managedSeesPost"

    $failed = @($checks | Where-Object { -not $_.passed })
    $artifact = [PSCustomObject]@{
        generated_at = (Get-Date).ToString("o")
        base_url = $BaseUrl
        passed = ($failed.Count -eq 0)
        checks = $checks
        admin_mailbox_id = $adminMailboxId
        managed_mailbox_id = $managedMailboxId
    }

    $outDir = Split-Path -Parent $OutputPath
    if ($outDir) {
        New-Item -ItemType Directory -Force -Path $outDir | Out-Null
    }

    $artifact | ConvertTo-Json -Depth 7 | Set-Content -Path $OutputPath -Encoding UTF8
    Write-Host "[managed-user-flow-check] Wrote artifact: $OutputPath"

    if (-not $artifact.passed) {
        throw "managed user flow check failed; see $OutputPath"
    }

    Write-Host "[managed-user-flow-check] PASS"
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
