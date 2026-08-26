#requires -version 5.1
[CmdletBinding()]
param(
    [string] $RepositoryRoot = 'D:\Projects\MechoFly',

    [string] $Downloads = (Join-Path $env:USERPROFILE 'Downloads'),

    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._/-]{0,127}$')]
    [string] $TargetBranch = 'feat/transparent-pet-dark-brainlab-v3',

    [ValidatePattern('^(|[0-9a-fA-F]{40})$')]
    [string] $TargetCommit = '',

    [ValidatePattern('^[0-9a-fA-F]{40}$')]
    [string] $ExpectedDirtyHead =
        'fa0ca7653bda7042028c7353d1e648de60f5f9e8',

    [ValidatePattern('^[0-9a-fA-F]{40}$')]
    [string] $ExpectedDirtyIndexTree =
        '8222b59929069f166bb274837a9edf52b3201924',

    [switch] $Launch,

    [switch] $GuardSelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$CanonicalRepository = 'https://github.com/yaelkroy/MechoFly.git'
$ExpectedSetupHash =
    'CF8FBC4788AD98F63043AB0B18887FAC4B0EDEBC293D88EB687622C843B4F945'
$ExpectedCollectorHash =
    '4FF1A49B34FCFF517A782360C3C1B585B48E730B5F40C0663D2948A7645BEE5E'

function Invoke-RecoveryGit {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyCollection()]
        [string[]] $Arguments,

        [switch] $Capture
    )

    $StandardErrorPath = [System.IO.Path]::Combine(
        [System.IO.Path]::GetTempPath(),
        'MechoFly-Recovery-Git-' + [Guid]::NewGuid().ToString('N') + '.txt')
    $Output = @()
    $StandardError = ''
    $ExitCode = -1
    $PreviousPreference = $ErrorActionPreference
    try {
        # Windows PowerShell 5.1 promotes ordinary native stderr to error
        # records. Keep stdout and stderr separate and trust Git's exit code.
        $ErrorActionPreference = 'Continue'
        $Output = @(& $script:RecoveryGitExecutable @Arguments 2> $StandardErrorPath)
        $ExitCode = $LASTEXITCODE
        if (Test-Path -LiteralPath $StandardErrorPath -PathType Leaf) {
            $StandardError = [System.IO.File]::ReadAllText($StandardErrorPath)
        }
    }
    finally {
        $ErrorActionPreference = $PreviousPreference
        Remove-Item -LiteralPath $StandardErrorPath -Force `
            -ErrorAction SilentlyContinue
    }

    $StandardOutput = (($Output | ForEach-Object { [string]$_ }) -join
        [Environment]::NewLine).Trim()
    if ($ExitCode -ne 0) {
        $Details = @($StandardOutput, $StandardError.Trim()) |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
        throw ('git {0} failed with exit code {1}.{2}{3}' -f
            ($Arguments -join ' '),
            $ExitCode,
            [Environment]::NewLine,
            ($Details -join [Environment]::NewLine))
    }

    if ($Capture) {
        return $StandardOutput
    }
    if (-not [string]::IsNullOrWhiteSpace($StandardOutput)) {
        Write-Host $StandardOutput
    }
    if (-not [string]::IsNullOrWhiteSpace($StandardError)) {
        Write-Host $StandardError.Trim()
    }
}

function ConvertTo-NormalizedRepositoryUrl {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Url
    )

    $Normalized = $Url.Trim().Replace('\', '/').TrimEnd('/')
    if ($Normalized.StartsWith(
        'git@github.com:',
        [StringComparison]::OrdinalIgnoreCase)) {
        $Normalized = 'https://github.com/' +
            $Normalized.Substring('git@github.com:'.Length)
    }
    if ($Normalized.EndsWith('.git', [StringComparison]::OrdinalIgnoreCase)) {
        $Normalized = $Normalized.Substring(0, $Normalized.Length - 4)
    }
    return $Normalized.ToLowerInvariant()
}

function Get-RecoveryState {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Root
    )

    $Status = Invoke-RecoveryGit -Arguments @(
        '-C', $Root, 'status', '--porcelain=v1', '--untracked-files=all') -Capture
    $Unstaged = Invoke-RecoveryGit -Arguments @(
        '-C', $Root, 'diff', '--name-only') -Capture
    $Untracked = Invoke-RecoveryGit -Arguments @(
        '-C', $Root, 'ls-files', '--others', '--exclude-standard') -Capture

    return [pscustomobject][ordered]@{
        branch = Invoke-RecoveryGit -Arguments @(
            '-C', $Root, 'rev-parse', '--abbrev-ref', 'HEAD') -Capture
        head = Invoke-RecoveryGit -Arguments @(
            '-C', $Root, 'rev-parse', 'HEAD') -Capture
        index_tree = Invoke-RecoveryGit -Arguments @(
            '-C', $Root, 'write-tree') -Capture
        status = $Status
        unstaged = $Unstaged
        untracked = $Untracked
    }
}

function Assert-ExactRecoverableState {
    param(
        [Parameter(Mandatory = $true)]
        [object] $State,

        [Parameter(Mandatory = $true)]
        [string] $RequiredHead,

        [Parameter(Mandatory = $true)]
        [string] $RequiredIndexTree
    )

    if ([string]::IsNullOrWhiteSpace([string]$State.status)) {
        throw 'Recovery was requested, but the checkout is already clean.'
    }
    if (-not [string]::IsNullOrWhiteSpace([string]$State.unstaged)) {
        throw ('Recovery refused because unstaged changes are present.' +
            [Environment]::NewLine + [string]$State.unstaged)
    }
    if (-not [string]::IsNullOrWhiteSpace([string]$State.untracked)) {
        throw ('Recovery refused because untracked files are present.' +
            [Environment]::NewLine + [string]$State.untracked)
    }
    if ([string]$State.head -ne $RequiredHead.ToLowerInvariant()) {
        throw ('Recovery refused because HEAD differs from the diagnosed ' +
            'AI100 state. Expected ' + $RequiredHead.ToLowerInvariant() +
            '; received ' + [string]$State.head)
    }
    if ([string]$State.index_tree -ne
        $RequiredIndexTree.ToLowerInvariant()) {
        throw ('Recovery refused because the staged tree differs from the ' +
            'reviewed Rust candidate. Expected ' +
            $RequiredIndexTree.ToLowerInvariant() + '; received ' +
            [string]$State.index_tree)
    }
}

function Invoke-RecoveryGuardSelfTest {
    $Fixture = Join-Path ([System.IO.Path]::GetTempPath()) (
        'MechoFly-Recovery-SelfTest-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $Fixture -Force | Out-Null
    try {
        Invoke-RecoveryGit -Arguments @('-C', $Fixture, 'init')
        Invoke-RecoveryGit -Arguments @(
            '-C', $Fixture, 'config', 'user.name', 'MechoFly Recovery Test')
        Invoke-RecoveryGit -Arguments @(
            '-C', $Fixture, 'config', 'user.email', 'recovery-test@invalid')
        $FixtureFile = Join-Path $Fixture 'candidate.txt'
        [System.IO.File]::WriteAllText(
            $FixtureFile,
            'base' + [Environment]::NewLine,
            (New-Object System.Text.UTF8Encoding($false)))
        Invoke-RecoveryGit -Arguments @('-C', $Fixture, 'add', 'candidate.txt')
        Invoke-RecoveryGit -Arguments @(
            '-C', $Fixture, 'commit', '-m', 'fixture base')
        $FixtureHead = Invoke-RecoveryGit -Arguments @(
            '-C', $Fixture, 'rev-parse', 'HEAD') -Capture

        [System.IO.File]::WriteAllText(
            $FixtureFile,
            'candidate' + [Environment]::NewLine,
            (New-Object System.Text.UTF8Encoding($false)))
        Invoke-RecoveryGit -Arguments @('-C', $Fixture, 'add', 'candidate.txt')
        $FixtureTree = Invoke-RecoveryGit -Arguments @(
            '-C', $Fixture, 'write-tree') -Capture
        $State = Get-RecoveryState -Root $Fixture
        Assert-ExactRecoverableState `
            -State $State `
            -RequiredHead $FixtureHead `
            -RequiredIndexTree $FixtureTree

        [System.IO.File]::WriteAllText(
            (Join-Path $Fixture 'unexpected.txt'),
            'must be rejected' + [Environment]::NewLine,
            (New-Object System.Text.UTF8Encoding($false)))
        $Rejected = $false
        try {
            $UnexpectedState = Get-RecoveryState -Root $Fixture
            Assert-ExactRecoverableState `
                -State $UnexpectedState `
                -RequiredHead $FixtureHead `
                -RequiredIndexTree $FixtureTree
        }
        catch {
            $Rejected = $true
        }
        if (-not $Rejected) {
            throw 'Recovery guard accepted an unexpected untracked file.'
        }
    }
    finally {
        if (Test-Path -LiteralPath $Fixture -PathType Container) {
            Remove-Item -LiteralPath $Fixture -Recurse -Force
        }
    }
    Write-Host 'MECHOFLY_RECOVERY_GUARD_SELF_TEST=PASS'
}

$GitCommand = Get-Command 'git.exe' -ErrorAction SilentlyContinue
if ($null -eq $GitCommand) {
    $GitCommand = Get-Command 'git' -ErrorAction SilentlyContinue
}
if ($null -eq $GitCommand) {
    throw 'Git for Windows was not found on PATH.'
}
$script:RecoveryGitExecutable = $GitCommand.Source

if ($GuardSelfTest) {
    Invoke-RecoveryGuardSelfTest
    exit 0
}

if ([string]::IsNullOrWhiteSpace($TargetCommit)) {
    throw 'TargetCommit is required outside the recovery guard self-test.'
}
$TargetCommit = $TargetCommit.ToLowerInvariant()
$ExpectedDirtyHead = $ExpectedDirtyHead.ToLowerInvariant()
$ExpectedDirtyIndexTree = $ExpectedDirtyIndexTree.ToLowerInvariant()

if (-not (Test-Path -LiteralPath $RepositoryRoot -PathType Container) -or
    -not (Test-Path -LiteralPath (Join-Path $RepositoryRoot '.git') `
        -PathType Container)) {
    throw ('MechoFly Git checkout was not found: ' + $RepositoryRoot)
}
if (-not (Test-Path -LiteralPath $Downloads -PathType Container)) {
    throw ('Downloads directory was not found: ' + $Downloads)
}
$RepositoryRoot = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$Downloads = (Resolve-Path -LiteralPath $Downloads).Path

$ExistingProcesses = @(Get-CimInstance Win32_Process `
    -Filter "Name='MechoFly.exe'" `
    -ErrorAction SilentlyContinue)
if ($ExistingProcesses.Count -gt 0) {
    throw ('Quit MechoFly before recovery. Running process IDs: ' +
        (($ExistingProcesses | ForEach-Object {
            [string]$_.ProcessId
        }) -join ', '))
}

$Origin = Invoke-RecoveryGit -Arguments @(
    '-C', $RepositoryRoot, 'remote', 'get-url', 'origin') -Capture
if ((ConvertTo-NormalizedRepositoryUrl -Url $Origin) -ne
    (ConvertTo-NormalizedRepositoryUrl -Url $CanonicalRepository)) {
    throw ('Recovery refused because origin is not the canonical repository: ' +
        $Origin)
}

Invoke-RecoveryGit -Arguments @('check-ref-format', '--branch', $TargetBranch)
$FetchRef = 'refs/heads/' + $TargetBranch +
    ':refs/remotes/origin/' + $TargetBranch
Invoke-RecoveryGit -Arguments @(
    '-C', $RepositoryRoot, 'fetch', '--prune', 'origin', $FetchRef)
$RemoteCommit = Invoke-RecoveryGit -Arguments @(
    '-C', $RepositoryRoot, 'rev-parse', ('origin/' + $TargetBranch)) -Capture
if ($RemoteCommit -ne $TargetCommit) {
    throw ('GitHub branch moved. Expected ' + $TargetCommit + '; received ' +
        $RemoteCommit)
}
$RemoteTree = Invoke-RecoveryGit -Arguments @(
    '-C', $RepositoryRoot, 'rev-parse',
    ('origin/' + $TargetBranch + '^{tree}')) -Capture

$State = Get-RecoveryState -Root $RepositoryRoot
Assert-ExactRecoverableState `
    -State $State `
    -RequiredHead $ExpectedDirtyHead `
    -RequiredIndexTree $ExpectedDirtyIndexTree

$Stamp = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssZ')
$BackupBranch = 'backup/ai100-pre-sync-' + $Stamp.ToLowerInvariant()
$BundlePath = Join-Path $Downloads (
    'PRESERVED_MechoFly_AI100_PreSync_' + $Stamp + '_' +
    $ExpectedDirtyIndexTree.Substring(0, 12) + '.bundle')
$StashMessage = 'AI100 exact candidate pre-sync preservation ' + $Stamp

Invoke-RecoveryGit -Arguments @(
    '-C', $RepositoryRoot, 'stash', 'push', '--include-untracked',
    '--message', $StashMessage)
$StashCommit = Invoke-RecoveryGit -Arguments @(
    '-C', $RepositoryRoot, 'rev-parse', 'refs/stash') -Capture
$StashTree = Invoke-RecoveryGit -Arguments @(
    '-C', $RepositoryRoot, 'rev-parse', ($StashCommit + '^{tree}')) -Capture
if ($StashTree -ne $ExpectedDirtyIndexTree) {
    throw ('Preserved stash tree mismatch. The stash remains available at ' +
        $StashCommit + '; expected tree ' + $ExpectedDirtyIndexTree +
        '; received ' + $StashTree)
}
Invoke-RecoveryGit -Arguments @(
    '-C', $RepositoryRoot, 'branch', $BackupBranch, $StashCommit)
Invoke-RecoveryGit -Arguments @(
    '-C', $RepositoryRoot, 'bundle', 'create', $BundlePath,
    ('refs/heads/' + $BackupBranch))
Invoke-RecoveryGit -Arguments @(
    '-C', $RepositoryRoot, 'bundle', 'verify', $BundlePath)

$CleanStatus = Invoke-RecoveryGit -Arguments @(
    '-C', $RepositoryRoot, 'status', '--porcelain=v1',
    '--untracked-files=all') -Capture
if (-not [string]::IsNullOrWhiteSpace($CleanStatus)) {
    throw ('Recovery preservation did not leave a clean checkout.' +
        [Environment]::NewLine + $CleanStatus)
}

Write-Host ('MECHOFLY_PRESERVED_STASH=' + $StashCommit)
Write-Host ('MECHOFLY_PRESERVED_BRANCH=' + $BackupBranch)
Write-Host ('MECHOFLY_PRESERVED_BUNDLE=' + $BundlePath)

$SetupPath = Join-Path ([System.IO.Path]::GetTempPath()) (
    'Setup-AI100-MechoFly-' + $TargetCommit.Substring(0, 12) + '.ps1')
$SetupUri = 'https://raw.githubusercontent.com/yaelkroy/MechoFly/' +
    $TargetCommit + '/tools/Setup-AI100-MechoFly.ps1'
Invoke-WebRequest -UseBasicParsing -Uri $SetupUri -OutFile $SetupPath
$SetupHash = (Get-FileHash -LiteralPath $SetupPath -Algorithm SHA256).Hash
if ($SetupHash -ne $ExpectedSetupHash) {
    throw ('Setup hash mismatch. Expected ' + $ExpectedSetupHash +
        '; received ' + $SetupHash)
}
Unblock-File -LiteralPath $SetupPath -ErrorAction SilentlyContinue

$Tokens = $null
$ParseErrors = $null
[System.Management.Automation.Language.Parser]::ParseFile(
    $SetupPath,
    [ref]$Tokens,
    [ref]$ParseErrors) | Out-Null
if (@($ParseErrors).Count -gt 0) {
    $ParseErrors | Format-List Message, ErrorId, Extent -Force
    throw 'Pinned setup failed the Windows PowerShell 5.1 parser.'
}

& "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
    -NoLogo `
    -NoProfile `
    -ExecutionPolicy Bypass `
    -File $SetupPath `
    -Branch $TargetBranch `
    -ExpectedCommit $TargetCommit
$SetupExitCode = $LASTEXITCODE
if ($SetupExitCode -ne 0) {
    throw ('Pinned MechoFly setup exited with code ' +
        [string]$SetupExitCode + '. Preserved bundle: ' + $BundlePath)
}

$Collector = Join-Path $RepositoryRoot 'tools\Capture-AI100-Evidence.ps1'
if (-not (Test-Path -LiteralPath $Collector -PathType Leaf)) {
    throw ('Evidence collector was not found after setup: ' + $Collector)
}
$CollectorHash = (Get-FileHash -LiteralPath $Collector -Algorithm SHA256).Hash
if ($CollectorHash -ne $ExpectedCollectorHash) {
    throw ('Evidence collector hash mismatch. Expected ' +
        $ExpectedCollectorHash + '; received ' + $CollectorHash)
}

$CaptureStartedUtc = [DateTime]::UtcNow
& "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
    -NoLogo `
    -NoProfile `
    -ExecutionPolicy Bypass `
    -File $Collector `
    -RepositoryRoot $RepositoryRoot
$CollectorExitCode = $LASTEXITCODE

$EvidenceZip = Get-ChildItem `
    -LiteralPath $Downloads `
    -Filter 'UPLOAD_MechoFly_AI100_ExactSource_Design_*.zip' `
    -File |
    Where-Object { $_.LastWriteTimeUtc -ge $CaptureStartedUtc.AddSeconds(-2) } |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
if ($null -eq $EvidenceZip) {
    throw ('Evidence collector exited with code ' +
        [string]$CollectorExitCode + ' and produced no new evidence ZIP.')
}
Write-Host ('UPLOAD_THIS_ZIP=' + $EvidenceZip.FullName)
if ($CollectorExitCode -ne 0) {
    throw ('Evidence collection exited with code ' +
        [string]$CollectorExitCode +
        '. Upload the preserved partial evidence ZIP.')
}

if ($Launch) {
    $StartScript = Join-Path $RepositoryRoot `
        'host-windows\Start-MechoFly-AI100.ps1'
    & "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
        -NoLogo `
        -NoProfile `
        -ExecutionPolicy Bypass `
        -File $StartScript
    if ($LASTEXITCODE -ne 0) {
        throw ('MechoFly launch exited with code ' + [string]$LASTEXITCODE)
    }
}

Write-Host ('MECHOFLY_TARGET_BRANCH=' + $TargetBranch)
Write-Host ('MECHOFLY_TARGET_COMMIT=' + $TargetCommit)
Write-Host ('MECHOFLY_TARGET_TREE=' + $RemoteTree)
Write-Host 'MECHOFLY_AI100_RECOVERY_INSTALL_CAPTURE=PASS'
