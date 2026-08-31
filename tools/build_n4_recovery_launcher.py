"""Generate a standalone, byte-pinned Windows launcher from the adjacent source."""
from pathlib import Path
import hashlib
import sys

PREFIX = r'''#requires -version 5.1
[CmdletBinding()]
param(
    [string] $RepositoryRoot = 'D:\Projects\MechoFly',
    [string] $ComparisonRoot = ('D:\Projects\Desktop' + 'Fly-V12'),
    [switch] $FullCampaign,
    [switch] $CaptureGui,
    [switch] $VerifyOnly
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ExpectedSourceHash = '__HASH__'
$PythonSource = @'
'''
SUFFIX = r'''
'@
# Normalize the here-string without altering Python source or depending on CRLF.
$PythonSource = $PythonSource.Replace("`r`n", "`n").TrimEnd([char]10) + "`n"
$Utf8 = New-Object System.Text.UTF8Encoding($false)
$Bytes = $Utf8.GetBytes($PythonSource)
$Hasher = [System.Security.Cryptography.SHA256]::Create()
try {
    $Actual = [BitConverter]::ToString($Hasher.ComputeHash($Bytes)).Replace('-', '').ToLowerInvariant()
}
finally { $Hasher.Dispose() }
if ($Actual -ne $ExpectedSourceHash) { throw 'Embedded Python source SHA-256 mismatch. Nothing was changed.' }
$TempRoot = Join-Path $env:TEMP ('MechoFly-N4-Exact-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $TempRoot | Out-Null
$Supervisor = Join-Path $TempRoot 'n4_exact_recovery.py'
[IO.File]::WriteAllBytes($Supervisor, $Bytes)
$Python = $null
$Prefix = @()
foreach ($Name in @('Python312', 'Python313', 'Python314', 'Python311')) {
    $Candidate = Join-Path $env:LOCALAPPDATA ('Programs\Python\' + $Name + '\python.exe')
    if (Test-Path -LiteralPath $Candidate -PathType Leaf) { $Python = $Candidate; break }
}
if ($null -eq $Python) {
    $Command = Get-Command 'py.exe' -ErrorAction SilentlyContinue
    if ($null -ne $Command) { $Python = $Command.Source; $Prefix = @('-3') }
}
if ($null -eq $Python) {
    $Command = Get-Command 'python.exe' -ErrorAction SilentlyContinue
    if ($null -ne $Command) { $Python = $Command.Source }
}
if ($null -eq $Python) { throw 'Python 3.9 or newer was not found.' }
& $Python @Prefix -c 'import sys; sys.exit(0 if sys.version_info >= (3,9) else 1)'
if ($LASTEXITCODE -ne 0) { throw 'Python version check failed.' }
& $Python @Prefix -m py_compile $Supervisor
if ($LASTEXITCODE -ne 0) { throw 'Embedded Python syntax check failed.' }
Write-Host ('N4_RECOVERY_SOURCE_SHA256=' + $Actual)
Write-Host 'SOURCE_PATCHING=NO'
Write-Host 'CANONICAL_DEPLOYMENT=NO'
Write-Host 'OLD_D0_OR_N3_RERUN=NO'
if ($VerifyOnly) {
    Write-Host 'N4_LAUNCHER_VERIFY_ONLY=PASS'
    return
}
$RunArguments = $Prefix + @(
    '-B', $Supervisor, '--repository-root', $RepositoryRoot, '--desktopfly-root', $ComparisonRoot
)
if ($FullCampaign) { $RunArguments += '--full-campaign' }
if ($CaptureGui) { $RunArguments += '--capture-gui' }
& $Python @RunArguments
$ExitCode = $LASTEXITCODE
if ($ExitCode -ne 0) {
    throw ('N4 exact validation stopped, exit=' + $ExitCode + '. Use the printed DIAGNOSTIC_ZIP path.')
}
'''


def build(source: Path, destination: Path) -> str:
    data = source.read_text(encoding='utf-8-sig').replace('\r\n','\n').rstrip('\n') + '\n'
    assert "\n'@" not in data, 'Python source cannot terminate a PowerShell here-string'
    h = hashlib.sha256(data.encode('utf-8')).hexdigest()
    launcher = PREFIX.replace('__HASH__', h) + data.rstrip('\n') + SUFFIX
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(launcher, encoding='utf-8', newline='\n')
    return hashlib.sha256(destination.read_bytes()).hexdigest()


if __name__ == '__main__':
    output = Path(sys.argv[1]) if len(sys.argv)>1 else Path('artifacts/START_MechoFly_N4_ExactValidation_AI100_20260831_V2.ps1')
    source = Path(sys.argv[2]) if len(sys.argv)>2 else Path(__file__).with_name('n4_exact_recovery.py')
    print('N4_LAUNCHER_SHA256=' + build(source, output))
