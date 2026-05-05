Set-StrictMode -Version Latest

function Import-SimyEnvFile {
    [CmdletBinding()]
    param(
        [string]$Path = ".env",
        [switch]$RequireFile,
        [switch]$ValidateRequired,
        [string[]]$RequiredVariables = @(
            "RELAY_BIND_ADDR",
            "RELAY_ADMIN_TOKEN",
            "POSTGRES_DSN",
            "REDIS_URL",
            "MEDIA_OBJECT_STORE_ENDPOINT",
            "MEDIA_OBJECT_STORE_BUCKET",
            "MEDIA_OBJECT_STORE_ACCESS_KEY_ID",
            "MEDIA_OBJECT_STORE_SECRET_ACCESS_KEY"
        )
    )

    $resolvedPath = Resolve-Path -LiteralPath $Path -ErrorAction SilentlyContinue
    if (-not $resolvedPath) {
        if ($RequireFile) {
            throw "Environment file '$Path' not found."
        }
        return @{}
    }

    $loaded = @{}
    $lineNumber = 0
    foreach ($line in Get-Content -LiteralPath $resolvedPath.Path) {
        $lineNumber += 1
        $trimmed = $line.Trim()
        if ([string]::IsNullOrWhiteSpace($trimmed) -or $trimmed.StartsWith("#")) {
            continue
        }

        if ($trimmed -notmatch "^[A-Za-z_][A-Za-z0-9_]*=.*$") {
            throw "Invalid env line at $($resolvedPath.Path):$lineNumber -> '$line'"
        }

        $name, $value = $trimmed -split "=", 2
        $name = $name.Trim()
        $value = $value.Trim()

        # Strip symmetric quote wrappers while preserving inner characters.
        if (
            ($value.StartsWith('"') -and $value.EndsWith('"')) -or
            ($value.StartsWith("'") -and $value.EndsWith("'"))
        ) {
            if ($value.Length -ge 2) {
                $value = $value.Substring(1, $value.Length - 2)
            }
        }

        Set-Item -Path ("Env:" + $name) -Value $value
        $loaded[$name] = $value
    }

    if ($ValidateRequired) {
        $missing = @()
        foreach ($requiredName in $RequiredVariables) {
            $currentValue = [Environment]::GetEnvironmentVariable($requiredName, [EnvironmentVariableTarget]::Process)
            if ([string]::IsNullOrWhiteSpace($currentValue)) {
                $missing += $requiredName
            }
        }

        if ($missing.Count -gt 0) {
            throw "Missing required environment variables: $($missing -join ', ')"
        }
    }

    return $loaded
}
