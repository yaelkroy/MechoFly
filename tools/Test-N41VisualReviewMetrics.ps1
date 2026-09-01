#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $OutputDirectory
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$script:Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Assert-TestCondition {
    param(
        [bool] $Condition,
        [string] $Message
    )
    if (-not $Condition) { throw $Message }
}

function Write-SyntheticTrace {
    param(
        [string] $LiteralPath,
        [bool] $IncludeCompletedWalk
    )
    $writer = New-Object System.IO.StreamWriter(
        $LiteralPath,
        $false,
        $script:Utf8NoBom)
    try {
        for ($frame = 0; $frame -lt 1000; $frame++) {
            $walking = $IncludeCompletedWalk -and $frame -lt 3
            $behavior = if ($walking) { 'walk' } else { 'quiet' }
            $age = if ($walking) { $frame } else {
                if ($IncludeCompletedWalk) { $frame - 3 } else { $frame }
            }
            $x = if ($walking) { 100.0 + 2.0 * $frame } else {
                if ($IncludeCompletedWalk) { 104.0 } else { 100.0 }
            }
            $record = [pscustomobject][ordered]@{
                sequence = $frame + 1
                wall_elapsed_ms = $frame * 33
                model_frame = $frame
                modeled_ms = $frame * 33
                behavior = $behavior
                behavior_age_frames = $age
                grooming_substate = $null
                screen_x = $x
                screen_y = 200.0
                heading_radians = 0.0
                speed_pixels_per_second = $(if ($walking) { 60.0 } else { 0.0 })
                cursor_hovered = $false
                dragging = $false
                evidence_hold = $false
            }
            $writer.WriteLine(($record | ConvertTo-Json -Compress))
        }
    }
    finally {
        $writer.Dispose()
    }
}

$output = [System.IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $output -Force | Out-Null
$measure = Join-Path $PSScriptRoot 'Measure-N41NaturalMotionEvidence.ps1'
$collector = Join-Path $PSScriptRoot 'Invoke-N41VisualReview.ps1'
foreach ($path in @($measure, $collector)) {
    Assert-TestCondition (Test-Path -LiteralPath $path -PathType Leaf) (
        'Required metrics artifact is missing: ' + $path)
}

$quietRoot = Join-Path $output 'zero-bout'
$walkRoot = Join-Path $output 'first-bout'
New-Item -ItemType Directory -Path @(
    (Join-Path $quietRoot 'captures'),
    (Join-Path $walkRoot 'captures')
) -Force | Out-Null
$quietTrace = Join-Path $quietRoot 'trace.jsonl'
$walkTrace = Join-Path $walkRoot 'trace.jsonl'
$quietReceipt = Join-Path $quietRoot 'metrics.json'
$walkReceipt = Join-Path $walkRoot 'metrics.json'
Write-SyntheticTrace -LiteralPath $quietTrace -IncludeCompletedWalk $false
Write-SyntheticTrace -LiteralPath $walkTrace -IncludeCompletedWalk $true

& $measure `
    -CollectorPath $collector `
    -TracePath $quietTrace `
    -CaptureDirectory (Join-Path $quietRoot 'captures') `
    -ReceiptPath $quietReceipt
& $measure `
    -CollectorPath $collector `
    -TracePath $walkTrace `
    -CaptureDirectory (Join-Path $walkRoot 'captures') `
    -ReceiptPath $walkReceipt

$quiet = Get-Content -LiteralPath $quietReceipt -Raw | ConvertFrom-Json
$walk = Get-Content -LiteralPath $walkReceipt -Raw | ConvertFrom-Json
Assert-TestCondition ([string]$quiet.status -ceq 'PASS') (
    'Zero-bout measurement did not complete.')
Assert-TestCondition (
    [int]$quiet.objective_natural_motion.autonomous_complete_walk_bouts -eq 0) (
    'Zero-bout regression produced a completed Walk bout.')
Assert-TestCondition (
    [int]$quiet.objective_natural_motion.unique_modeled_frames -eq 1000) (
    'Zero-bout regression lost modeled frames.')
Assert-TestCondition ([string]$walk.status -ceq 'PASS') (
    'First-bout measurement did not complete.')
Assert-TestCondition (
    [int]$walk.objective_natural_motion.autonomous_complete_walk_bouts -eq 1) (
    'The first completed Walk bout was not collected into an initially empty list.')
Assert-TestCondition (
    [double]$walk.objective_natural_motion.walk_duration_min_seconds -eq 0.099) (
    'First-bout duration regression changed unexpectedly.')

$regression = [pscustomobject][ordered]@{
    schema_version = 1
    status = 'PASS'
    classification = 'n4_1_visual_metrics_empty_collection_regression'
    zero_bout_measurement_completed = $true
    first_bout_added_to_empty_list = $true
    empty_numeric_distributions_supported = $true
    collector_sha256 = (Get-FileHash `
        -LiteralPath $collector `
        -Algorithm SHA256).Hash.ToLowerInvariant()
    promotion_authorized = $false
    deployment_authorized = $false
}
[System.IO.File]::WriteAllText(
    (Join-Path $output 'regression-receipt.json'),
    ($regression | ConvertTo-Json -Depth 12) + [Environment]::NewLine,
    $script:Utf8NoBom)
Write-Host 'N41_VISUAL_METRICS_REGRESSION=PASS empty_collections=true first_bout=true'
