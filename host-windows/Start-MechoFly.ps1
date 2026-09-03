#requires -version 5.1
[CmdletBinding()]
param(
    [switch] $Rebuild,

    [ValidateSet('drosophila', 'firefly')]
    [string] $Skin,

    [ValidateSet('auto', 'cpu', 'gpu')]
    [string] $Compute,

    [switch] $BrainLab,

    [switch] $ReducedMotion
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Executable = Join-Path $PSScriptRoot 'bin\MechoFly.exe'
if ($Rebuild -or -not (Test-Path -LiteralPath $Executable -PathType Leaf)) {
    & (Join-Path $PSScriptRoot 'build.ps1')
}

$ProfilePath = Join-Path $env:LOCALAPPDATA 'MechoFly\runtime-profile.json'
$Profile = $null
if (Test-Path -LiteralPath $ProfilePath -PathType Leaf) {
    try {
        $Profile = Get-Content -LiteralPath $ProfilePath -Raw | ConvertFrom-Json
    }
    catch {
        Write-Warning ('Ignoring invalid runtime profile: ' + $ProfilePath)
    }
}

# AI100 launches only a binary that can be traced byte-for-byte to the clean
# checkout recorded by setup. This is an offline identity check; the Sync
# shortcut and evidence collector additionally confirm the current GitHub ref.
if ($null -ne $Profile -and
    $Profile.PSObject.Properties['machine_role'] -and
    [string]$Profile.machine_role -eq 'ai100-development') {
    $RepositoryRoot = Split-Path -Parent $PSScriptRoot
    & (Join-Path $RepositoryRoot 'tools\Assert-AI100-SourceIdentity.ps1') `
        -RepositoryRoot $RepositoryRoot `
        -ProfilePath $ProfilePath
}

if (-not $PSBoundParameters.ContainsKey('Skin')) {
    $Skin = 'firefly'
    if ($null -ne $Profile -and $Profile.PSObject.Properties['skin'] -and
        @('drosophila', 'firefly') -contains [string]$Profile.skin) {
        $Skin = [string]$Profile.skin
    }
}
if (-not $PSBoundParameters.ContainsKey('Compute')) {
    $Compute = 'auto'
    if ($null -ne $Profile -and $Profile.PSObject.Properties['compute'] -and
        @('auto', 'cpu', 'gpu') -contains [string]$Profile.compute) {
        $Compute = [string]$Profile.compute
    }
}
if (-not $PSBoundParameters.ContainsKey('ReducedMotion') -and
    $null -ne $Profile -and $Profile.PSObject.Properties['reduced_motion']) {
    $ReducedMotion = [bool]$Profile.reduced_motion
}

$Arguments = @('--skin', $Skin, '--compute', $Compute)
if ($BrainLab) {
    $Arguments += '--brain-lab'
}
if ($ReducedMotion) {
    $Arguments += '--reduced-motion'
}

$Process = Start-Process `
    -FilePath $Executable `
    -WorkingDirectory (Split-Path -Parent $Executable) `
    -ArgumentList $Arguments `
    -PassThru
Write-Host ('MECHOFLY_STARTED=' + $Executable)
Write-Host ('MECHOFLY_SKIN=' + $Skin)
Write-Host ('MECHOFLY_COMPUTE_PREFERENCE=' + $Compute)
Write-Host ('MECHOFLY_PROCESS_ID=' + [string]$Process.Id)
