param(
    [string]$EnvFile = ".env",
    [switch]$SkipEnvValidation,
    [switch]$EnforceNonDefaultAdminToken,
    [switch]$RequireDocker,
    [switch]$RequireCompose,
    [switch]$CheckDependencyPorts,
    [string]$PostgresHost = "127.0.0.1",
    [int]$PostgresPort = 5432,
    [string]$RedisHost = "127.0.0.1",
    [int]$RedisPort = 6379
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. "$PSScriptRoot/load_env.ps1"

$knownInsecureAdminTokens = @(
    "",
    "change-this-to-a-long-random-secret",
    "changeme",
    "default",
    "admin",
    "password"
)

function Assert-CommandAvailable {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    $cmd = Get-Command $Name -ErrorAction SilentlyContinue
    if (-not $cmd) {
        throw "Required command '$Name' is not available on PATH"
    }
}

function Test-TcpPort {
    param(
        [string]$HostName,
        [int]$Port
    )

    try {
        $client = New-Object Net.Sockets.TcpClient
        $client.Connect($HostName, $Port)
        $client.Dispose()
        return $true
    } catch {
        return $false
    }
}

Write-Host "[preflight] Checking required commands"
Assert-CommandAvailable -Name cargo

if ($RequireDocker -or $RequireCompose) {
    Assert-CommandAvailable -Name docker
}

if ($RequireCompose) {
    # This validates plugin-style compose availability.
    docker compose version | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose is required but not available"
    }
}

Write-Host "[preflight] Checking rust toolchain"
cargo --version | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "cargo is installed but not functioning correctly"
}

if (-not $SkipEnvValidation) {
    Write-Host "[preflight] Validating environment file: $EnvFile"
    $loaded = Import-SimyEnvFile -Path $EnvFile -RequireFile -ValidateRequired
    Write-Host "[preflight] Loaded $($loaded.Count) variables"
}

if ($EnforceNonDefaultAdminToken) {
    $adminToken = [Environment]::GetEnvironmentVariable("RELAY_ADMIN_TOKEN", [EnvironmentVariableTarget]::Process)
    if ($knownInsecureAdminTokens -contains $adminToken) {
        throw "RELAY_ADMIN_TOKEN is using an insecure placeholder value"
    }

    if ([string]::IsNullOrWhiteSpace($adminToken) -or $adminToken.Length -lt 24) {
        throw "RELAY_ADMIN_TOKEN must be set to a non-default value with at least 24 characters"
    }
}

if ($CheckDependencyPorts) {
    Write-Host "[preflight] Checking dependency reachability"

    if (-not (Test-TcpPort -HostName $PostgresHost -Port $PostgresPort)) {
        throw "PostgreSQL not reachable at ${PostgresHost}:${PostgresPort}"
    }

    if (-not (Test-TcpPort -HostName $RedisHost -Port $RedisPort)) {
        throw "Redis not reachable at ${RedisHost}:${RedisPort}"
    }

    Write-Host "[preflight] Dependency ports are reachable"
}

Write-Host "[preflight] OK"
