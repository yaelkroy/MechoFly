#requires -version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Get-Process -Name 'MechoFly' -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction Stop
Write-Host 'MECHOFLY_STOPPED=PASS'

