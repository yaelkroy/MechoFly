#requires -version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Root = Split-Path -Parent $PSScriptRoot
$Source = Join-Path $Root 'src\MechoFly'
$Output = Join-Path $PSScriptRoot 'bin'
if (-not (Test-Path -LiteralPath $Output -PathType Container)) {
    New-Item -ItemType Directory -Path $Output -Force | Out-Null
}

$CompilerCandidates = @(
    (Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'),
    (Join-Path $env:WINDIR 'Microsoft.NET\Framework\v4.0.30319\csc.exe')
)
$Compiler = $CompilerCandidates |
    Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
    Select-Object -First 1
if ([string]::IsNullOrWhiteSpace([string]$Compiler)) {
    throw '.NET Framework C# compiler was not found.'
}

$Sources = @(Get-ChildItem -LiteralPath $Source -Filter '*.cs' -File |
    Sort-Object Name |
    ForEach-Object { $_.FullName })
if ($Sources.Count -lt 8) {
    throw 'The MechoFly source set is incomplete.'
}

$Executable = Join-Path $Output 'MechoFly.exe'
$Arguments = @(
    '/nologo',
    '/target:winexe',
    '/optimize+',
    '/checked+',
    '/platform:x64',
    ('/out:' + $Executable),
    '/reference:System.dll',
    '/reference:System.Core.dll',
    '/reference:System.Drawing.dll',
    '/reference:System.Windows.Forms.dll'
) + $Sources

& $Compiler $Arguments
if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $Executable -PathType Leaf)) {
    throw 'MechoFly compilation failed.'
}

Write-Host ('MECHOFLY_BUILD=PASS executable=' + $Executable)

