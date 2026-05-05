param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Assert-FileExists {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    if (-not (Test-Path -Path $Path -PathType Leaf)) {
        throw "Required file is missing: $Path"
    }
}

function Assert-Contains {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Text,
        [Parameter(Mandatory = $true)]
        [string]$Needle,
        [Parameter(Mandatory = $true)]
        [string]$ContextLabel
    )

    if ($Text -notmatch [regex]::Escape($Needle)) {
        throw "Missing required content in ${ContextLabel}: $Needle"
    }
}

Write-Host "[production-gate] Verifying required files"
Assert-FileExists -Path "README.md"
Assert-FileExists -Path "docs/production-readiness.md"
Assert-FileExists -Path ".github/workflows/core-relay-tests.yml"

Write-Host "[production-gate] Verifying README production disclaimers"
$readme = Get-Content "README.md" -Raw
Assert-Contains -Text $readme -Needle "does not yet implement a production-ready end-user X3DH plus Double Ratchet messaging client" -ContextLabel "README.md"
Assert-Contains -Text $readme -Needle "browser secure mode is not the final production protocol client" -ContextLabel "README.md"

Write-Host "[production-gate] Verifying production readiness checklist content"
$prodDoc = Get-Content "docs/production-readiness.md" -Raw
$requiredChecklistItems = @(
    "Independent cryptography and protocol review",
    "Reproducible signed releases",
    "Device trust and revocation lifecycle",
    "End-to-end interoperability and recovery tests",
    "Incident response and key compromise runbook"
)

foreach ($item in $requiredChecklistItems) {
    Assert-Contains -Text $prodDoc -Needle $item -ContextLabel "docs/production-readiness.md"
}

Write-Host "[production-gate] Verifying CI wiring"
$workflow = Get-Content ".github/workflows/core-relay-tests.yml" -Raw
Assert-Contains -Text $workflow -Needle "./scripts/production_gate.ps1" -ContextLabel ".github/workflows/core-relay-tests.yml"

Write-Host "[production-gate] PASS"