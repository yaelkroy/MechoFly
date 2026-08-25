#requires -version 5.1
[CmdletBinding()]
param([switch] $Rebuild)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Executable = Join-Path $PSScriptRoot 'bin\MechoFly.exe'
if ($Rebuild -or -not (Test-Path -LiteralPath $Executable -PathType Leaf)) {
    & (Join-Path $PSScriptRoot 'build.ps1')
}
Start-Process -FilePath $Executable -WorkingDirectory (Split-Path -Parent $Executable)
Write-Host ('MECHOFLY_STARTED=' + $Executable)

