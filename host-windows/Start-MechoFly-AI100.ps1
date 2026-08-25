#requires -version 5.1
[CmdletBinding()]
param([switch] $Rebuild)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'Start-MechoFly.ps1') `
    -Rebuild:$Rebuild `
    -Skin 'firefly'
