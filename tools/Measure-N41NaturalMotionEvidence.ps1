#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $TracePath,

    [Parameter(Mandatory = $true)]
    [string] $CaptureDirectory,

    [Parameter(Mandatory = $true)]
    [string] $ReceiptPath,

    [string] $CollectorPath
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$script:Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Assert-MeasurementCondition {
    param(
        [bool] $Condition,
        [string] $Message
    )
    if (-not $Condition) { throw $Message }
}

if ([string]::IsNullOrWhiteSpace($CollectorPath)) {
    $CollectorPath = Join-Path $PSScriptRoot 'Invoke-N41VisualReview.ps1'
}
$collector = (Resolve-Path -LiteralPath $CollectorPath).Path
$trace = (Resolve-Path -LiteralPath $TracePath).Path
$captures = (Resolve-Path -LiteralPath $CaptureDirectory).Path
$receiptParent = Split-Path -Parent $ReceiptPath
if ($receiptParent -and
    -not (Test-Path -LiteralPath $receiptParent -PathType Container)) {
    New-Item -ItemType Directory -Path $receiptParent -Force | Out-Null
}

$tokens = $null
$parseErrors = $null
$collectorAst = [System.Management.Automation.Language.Parser]::ParseFile(
    $collector,
    [ref]$tokens,
    [ref]$parseErrors)
Assert-MeasurementCondition ($parseErrors.Count -eq 0) (
    'The visual-review collector does not parse: ' +
    (($parseErrors | ForEach-Object { $_.Message }) -join '; '))

$definitions = @($collectorAst.FindAll({
    param($node)
    $node -is [System.Management.Automation.Language.FunctionDefinitionAst]
}, $true))
$requiredFunctions = @(
    'Assert-Condition',
    'Get-MeanAndCv',
    'Get-NearestRank',
    'Get-SquaredCorrelation',
    'Add-CompletedWalkBout',
    'Add-CompletedFlightBout',
    'Get-NaturalMotionMetrics'
)
foreach ($name in $requiredFunctions) {
    $matches = @($definitions | Where-Object { $_.Name -ceq $name })
    Assert-MeasurementCondition ($matches.Count -eq 1) (
        "Expected exactly one collector function named $name; found " +
        [string]$matches.Count)
    Invoke-Expression $matches[0].Extent.Text
}

$metrics = Get-NaturalMotionMetrics `
    -TracePath $trace `
    -CaptureDirectory $captures
$captureRecords = @(Get-ChildItem -LiteralPath $captures -Filter '*.png' -File |
    Sort-Object Name |
    ForEach-Object {
        [pscustomobject][ordered]@{
            name = $_.Name
            bytes = $_.Length
            sha256 = (Get-FileHash `
                -LiteralPath $_.FullName `
                -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    })
$receipt = [pscustomobject][ordered]@{
    schema_version = 2
    status = 'PASS'
    classification = 'offline_n4_1_natural_motion_evidence_measurement'
    collector = $collector
    collector_sha256 = (Get-FileHash `
        -LiteralPath $collector `
        -Algorithm SHA256).Hash.ToLowerInvariant()
    trace = $trace
    trace_bytes = (Get-Item -LiteralPath $trace).Length
    trace_sha256 = (Get-FileHash `
        -LiteralPath $trace `
        -Algorithm SHA256).Hash.ToLowerInvariant()
    capture_directory = $captures
    captures = $captureRecords
    objective_natural_motion = $metrics
    screen_capture_used = $false
    appdata_write_authorized = $false
    promotion_authorized = $false
    deployment_authorized = $false
    measured_utc = [DateTime]::UtcNow.ToString('o')
}
[System.IO.File]::WriteAllText(
    $ReceiptPath,
    ($receipt | ConvertTo-Json -Depth 24) + [Environment]::NewLine,
    $script:Utf8NoBom)
Write-Host ('N41_NATURAL_MOTION_EVIDENCE=' +
    [string]$metrics.status +
    ' walk_bouts=' + [string]$metrics.autonomous_complete_walk_bouts +
    ' flight_bouts=' + [string]$metrics.complete_flight_bouts +
    ' grooming_bouts=' + [string]$metrics.grooming_bouts)
