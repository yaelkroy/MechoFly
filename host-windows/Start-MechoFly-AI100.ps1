#requires -version 5.1
[CmdletBinding()]
param(
    [switch] $Rebuild,
    [switch] $BrainLab
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'Start-MechoFly.ps1') `
    -Rebuild:$Rebuild `
    -BrainLab:$BrainLab `
    -Skin 'drosophila' `
    -Compute 'auto'
