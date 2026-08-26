#requires -version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Root = Split-Path -Parent $PSScriptRoot
$Output = Join-Path $PSScriptRoot 'bin'
if (-not (Test-Path -LiteralPath $Output -PathType Container)) {
    New-Item -ItemType Directory -Path $Output -Force | Out-Null
}

$CargoCommand = Get-Command 'cargo.exe' -ErrorAction SilentlyContinue
if ($null -eq $CargoCommand) {
    $CargoCommand = Get-Command 'cargo' -ErrorAction SilentlyContinue
}
if ($null -eq $CargoCommand) {
    $UserCargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
    if (Test-Path -LiteralPath $UserCargo -PathType Leaf) {
        $CargoExecutable = $UserCargo
    }
    else {
        throw ('Rust Cargo was not found. Install the stable Rust toolchain from ' +
            'https://rustup.rs/ and rerun this script.')
    }
}
else {
    $CargoExecutable = $CargoCommand.Source
}

$StandardErrorPath = [System.IO.Path]::Combine(
    [System.IO.Path]::GetTempPath(),
    'MechoFly-Cargo-Stderr-' + [Guid]::NewGuid().ToString('N') + '.txt')
$CargoOutput = @()
$CargoStandardError = ''
$CargoExitCode = -1
$PreviousErrorActionPreference = $ErrorActionPreference
try {
    # Windows PowerShell 5.1 promotes ordinary native stderr progress to
    # NativeCommandError when ErrorActionPreference is Stop. Keep channels
    # separate and make Cargo's exit code authoritative.
    $ErrorActionPreference = 'Continue'
    $CargoOutput = @(& $CargoExecutable `
        'build' `
        '--release' `
        '--locked' `
        '--package' 'mechofly-app' `
        '--bin' 'MechoFly' `
        '--manifest-path' (Join-Path $Root 'Cargo.toml') `
        2> $StandardErrorPath)
    $CargoExitCode = $LASTEXITCODE
    if (Test-Path -LiteralPath $StandardErrorPath -PathType Leaf) {
        $CargoStandardError = [System.IO.File]::ReadAllText($StandardErrorPath)
    }
}
finally {
    $ErrorActionPreference = $PreviousErrorActionPreference
    Remove-Item -LiteralPath $StandardErrorPath -Force -ErrorAction SilentlyContinue
}

$CargoStandardOutput = (($CargoOutput | ForEach-Object { [string]$_ }) -join
    [Environment]::NewLine).Trim()
if (-not [string]::IsNullOrWhiteSpace($CargoStandardOutput)) {
    Write-Host $CargoStandardOutput
}
if (-not [string]::IsNullOrWhiteSpace($CargoStandardError)) {
    Write-Host $CargoStandardError.Trim()
}
if ($CargoExitCode -ne 0) {
    throw ('Cargo build failed with exit code ' + [string]$CargoExitCode + '.')
}

$BuiltExecutable = Join-Path $Root 'target\release\MechoFly.exe'
if (-not (Test-Path -LiteralPath $BuiltExecutable -PathType Leaf)) {
    throw ('Cargo reported success but did not create ' + $BuiltExecutable)
}
$Executable = Join-Path $Output 'MechoFly.exe'
Copy-Item -LiteralPath $BuiltExecutable -Destination $Executable -Force
$BuiltSymbols = Join-Path $Root 'target\release\MechoFly.pdb'
$Symbols = Join-Path $Output 'MechoFly.pdb'
Remove-Item -LiteralPath $Symbols -Force -ErrorAction SilentlyContinue
if (Test-Path -LiteralPath $BuiltSymbols -PathType Leaf) {
    Copy-Item -LiteralPath $BuiltSymbols -Destination $Symbols -Force
}
else {
    Write-Warning ('Cargo did not emit the optional Windows symbol file ' + $BuiltSymbols)
}
$Hash = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash

Write-Host ('MECHOFLY_BUILD=PASS executable=' + $Executable)
Write-Host ('MECHOFLY_IMPLEMENTATION=rust')
Write-Host ('MECHOFLY_EXECUTABLE_SHA256=' + $Hash)
Write-Host ('MECHOFLY_PDB_PRESENT=' +
    (Test-Path -LiteralPath $Symbols -PathType Leaf))
