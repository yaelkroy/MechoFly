#requires -version 5.1
[CmdletBinding()]
param(
    [switch] $Rebuild,

    [ValidateSet('drosophila', 'firefly')]
    [string] $Skin
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Executable = Join-Path $PSScriptRoot 'bin\MechoFly.exe'
if ($Rebuild -or -not (Test-Path -LiteralPath $Executable -PathType Leaf)) {
    & (Join-Path $PSScriptRoot 'build.ps1')
}

if (-not $PSBoundParameters.ContainsKey('Skin')) {
    $Skin = 'drosophila'
    $ProfilePath = Join-Path $env:LOCALAPPDATA 'MechoFly\runtime-profile.json'
    if (Test-Path -LiteralPath $ProfilePath -PathType Leaf) {
        try {
            $Profile = Get-Content -LiteralPath $ProfilePath -Raw | ConvertFrom-Json
            if ($null -ne $Profile -and
                $Profile.PSObject.Properties['skin'] -and
                @('drosophila', 'firefly') -contains [string]$Profile.skin) {
                $Skin = [string]$Profile.skin
            }
        }
        catch {
            Write-Warning ('Ignoring invalid runtime profile: ' + $ProfilePath)
        }
    }
}

$Process = Start-Process `
    -FilePath $Executable `
    -WorkingDirectory (Split-Path -Parent $Executable) `
    -ArgumentList @('--skin', $Skin) `
    -PassThru
Write-Host ('MECHOFLY_STARTED=' + $Executable)
Write-Host ('MECHOFLY_SKIN=' + $Skin)
Write-Host ('MECHOFLY_PROCESS_ID=' + [string]$Process.Id)
