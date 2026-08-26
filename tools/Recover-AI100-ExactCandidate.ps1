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

    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._/-]{0,127}$')]
    [string] $ExpectedDirtyBranch = 'main',

    [ValidatePattern('^[0-9a-fA-F]{40}$')]
    [string] $ExpectedInterruptedCommit =
        'e75c417d8fbee70f1789f7299399ed157ff232fc',

    [switch] $Launch,

    [switch] $GuardSelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$CanonicalRepository = 'https://github.com/yaelkroy/MechoFly.git'
$ExpectedSetupHash =
    '7333BB8855954B1A27A9F7C52225EA501BADE3E7D712D6A932EFFDA9CFD0A40C'
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
        [string] $RequiredIndexTree,

        [Parameter(Mandatory = $true)]
        [string] $RequiredBranch
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
    if ([string]$State.branch -ne $RequiredBranch) {
        throw ('Recovery refused because the current branch differs from the ' +
            'diagnosed AI100 state. Expected ' + $RequiredBranch +
            '; received ' + [string]$State.branch)
    }
    if ([string]$State.head -ne $RequiredHead.ToLowerInvariant()) {
        throw ('Recovery refused because HEAD differs from the diagnosed ' +
            'AI100 state. Expected ' + $RequiredHead.ToLowerInvariant() +
            '; received ' + [string]$State.head)
    }
    if ([string]$State.index_tree -ne
        $RequiredIndexTree.ToLowerInvariant()) {
        throw ('Recovery refused because the staged tree differs from the ' +
            'required exact state. Expected ' +
            $RequiredIndexTree.ToLowerInvariant() + '; received ' +
            [string]$State.index_tree)
    }
}

function Get-VerifiedPreservation {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Root,

        [Parameter(Mandatory = $true)]
        [string] $DownloadRoot,

        [Parameter(Mandatory = $true)]
        [string] $RequiredHead,

        [Parameter(Mandatory = $true)]
        [string] $RequiredIndexTree
    )

    $BackupRefs = Invoke-RecoveryGit -Arguments @(
        '-C', $Root, 'for-each-ref',
        '--sort=-refname',
        '--format=%(refname:short)|%(objectname)',
        'refs/heads/backup/ai100-pre-sync-*') -Capture
    $SelectedBranch = $null
    $SelectedCommit = $null
    foreach ($Line in @($BackupRefs -split '[\r\n]+' |
        Where-Object { -not [string]::IsNullOrWhiteSpace($_) })) {
        $Parts = @($Line -split '\|', 2)
        if ($Parts.Count -ne 2) {
            continue
        }
        $CandidateTree = Invoke-RecoveryGit -Arguments @(
            '-C', $Root, 'rev-parse', ($Parts[1] + '^{tree}')) -Capture
        $CandidateFirstParent = Invoke-RecoveryGit -Arguments @(
            '-C', $Root, 'rev-parse', ($Parts[1] + '^1')) -Capture
        if ($CandidateTree -eq $RequiredIndexTree.ToLowerInvariant() -and
            $CandidateFirstParent -eq $RequiredHead.ToLowerInvariant()) {
            $SelectedBranch = $Parts[0]
            $SelectedCommit = $Parts[1]
            break
        }
    }
    if ($null -eq $SelectedCommit) {
        throw ('Recovery cannot resume because no backup branch preserves ' +
            'tree ' + $RequiredIndexTree.ToLowerInvariant() + '.')
    }

    $SelectedBundle = $null
    $BundleCandidates = @(Get-ChildItem `
        -LiteralPath $DownloadRoot `
        -Filter 'PRESERVED_MechoFly_AI100_PreSync_*.bundle' `
        -File |
        Sort-Object LastWriteTimeUtc -Descending)
    foreach ($Candidate in $BundleCandidates) {
        try {
            $BundleHeads = Invoke-RecoveryGit -Arguments @(
                'bundle', 'list-heads', $Candidate.FullName) -Capture
            if ($BundleHeads -match ('(?m)^' +
                [regex]::Escape($SelectedCommit) + '\s+')) {
                Invoke-RecoveryGit -Arguments @(
                    '-C', $Root, 'bundle', 'verify', $Candidate.FullName)
                $SelectedBundle = $Candidate.FullName
                break
            }
        }
        catch {
            Write-Warning ('Skipping an invalid preservation bundle: ' +
                $Candidate.FullName)
        }
    }
    if ($null -eq $SelectedBundle) {
        throw ('Recovery cannot resume because no verified Downloads bundle ' +
            'contains preserved commit ' + $SelectedCommit + '.')
    }

    return [pscustomobject][ordered]@{
        stash_commit = $SelectedCommit
        backup_branch = $SelectedBranch
        bundle = $SelectedBundle
    }
}

function Save-ExactStagedState {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Root,

        [Parameter(Mandatory = $true)]
        [string] $DownloadRoot,

        [Parameter(Mandatory = $true)]
        [object] $State,

        [Parameter(Mandatory = $true)]
        [string] $RequiredHead,

        [Parameter(Mandatory = $true)]
        [string] $RequiredIndexTree,

        [Parameter(Mandatory = $true)]
        [string] $RequiredBranch,

        [Parameter(Mandatory = $true)]
        [string] $BackupBranchPrefix,

        [Parameter(Mandatory = $true)]
        [string] $BundleFilePrefix,

        [Parameter(Mandatory = $true)]
        [string] $StashMessagePrefix
    )

    Assert-ExactRecoverableState `
        -State $State `
        -RequiredHead $RequiredHead `
        -RequiredIndexTree $RequiredIndexTree `
        -RequiredBranch $RequiredBranch

    $Stamp = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ')
    $BackupBranch = $BackupBranchPrefix + $Stamp.ToLowerInvariant()
    $BundlePath = Join-Path $DownloadRoot (
        $BundleFilePrefix + '_' + $Stamp + '_' +
        $RequiredIndexTree.Substring(0, 12) + '.bundle')
    $StashMessage = $StashMessagePrefix + ' ' + $Stamp

    Invoke-RecoveryGit -Arguments @(
        '-C', $Root, 'stash', 'push', '--include-untracked',
        '--message', $StashMessage)
    $StashCommit = Invoke-RecoveryGit -Arguments @(
        '-C', $Root, 'rev-parse', 'refs/stash') -Capture
    $StashTree = Invoke-RecoveryGit -Arguments @(
        '-C', $Root, 'rev-parse', ($StashCommit + '^{tree}')) -Capture
    $StashFirstParent = Invoke-RecoveryGit -Arguments @(
        '-C', $Root, 'rev-parse', ($StashCommit + '^1')) -Capture
    if ($StashTree -ne $RequiredIndexTree.ToLowerInvariant() -or
        $StashFirstParent -ne $RequiredHead.ToLowerInvariant()) {
        throw ('Preserved stash identity mismatch. The stash remains at ' +
            $StashCommit + '; expected tree ' +
            $RequiredIndexTree.ToLowerInvariant() + ' with first parent ' +
            $RequiredHead.ToLowerInvariant() + '; received tree ' +
            $StashTree + ' with first parent ' + $StashFirstParent)
    }

    Invoke-RecoveryGit -Arguments @(
        '-C', $Root, 'branch', $BackupBranch, $StashCommit)
    Invoke-RecoveryGit -Arguments @(
        '-C', $Root, 'bundle', 'create', $BundlePath,
        ('refs/heads/' + $BackupBranch))
    Invoke-RecoveryGit -Arguments @(
        '-C', $Root, 'bundle', 'verify', $BundlePath)

    $CleanStatus = Invoke-RecoveryGit -Arguments @(
        '-C', $Root, 'status', '--porcelain=v1',
        '--untracked-files=all') -Capture
    if (-not [string]::IsNullOrWhiteSpace($CleanStatus)) {
        throw ('Preservation did not leave a clean checkout.' +
            [Environment]::NewLine + $CleanStatus)
    }

    return [pscustomobject][ordered]@{
        stash_commit = $StashCommit
        backup_branch = $BackupBranch
        bundle = $BundlePath
    }
}

function Invoke-RecoveryGuardSelfTest {
    $FixtureRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
        'MechoFly-Recovery-SelfTest-' + [Guid]::NewGuid().ToString('N'))
    $Fixture = Join-Path $FixtureRoot 'repository'
    $FixtureDownloads = Join-Path $FixtureRoot 'downloads'
    New-Item -ItemType Directory -Path $Fixture -Force | Out-Null
    New-Item -ItemType Directory -Path $FixtureDownloads -Force | Out-Null
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
        Invoke-RecoveryGit -Arguments @('-C', $Fixture, 'branch', '-M', 'main')
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
            -RequiredIndexTree $FixtureTree `
            -RequiredBranch 'main'

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
                -RequiredIndexTree $FixtureTree `
                -RequiredBranch 'main'
        }
        catch {
            $Rejected = $true
        }
        if (-not $Rejected) {
            throw 'Recovery guard accepted an unexpected untracked file.'
        }

        Remove-Item -LiteralPath (Join-Path $Fixture 'unexpected.txt') -Force
        $MissingPreservationRejected = $false
        try {
            Get-VerifiedPreservation `
                -Root $Fixture `
                -DownloadRoot $FixtureDownloads `
                -RequiredHead $FixtureHead `
                -RequiredIndexTree $FixtureTree | Out-Null
        }
        catch {
            $MissingPreservationRejected = $true
        }
        if (-not $MissingPreservationRejected) {
            throw 'Recovery accepted residue without an original preservation.'
        }

        $OriginalPreservation = Save-ExactStagedState `
            -Root $Fixture `
            -DownloadRoot $FixtureDownloads `
            -State (Get-RecoveryState -Root $Fixture) `
            -RequiredHead $FixtureHead `
            -RequiredIndexTree $FixtureTree `
            -RequiredBranch 'main' `
            -BackupBranchPrefix 'backup/ai100-pre-sync-' `
            -BundleFilePrefix 'PRESERVED_MechoFly_AI100_PreSync' `
            -StashMessagePrefix 'recovery resume self-test'

        $FoundOriginal = Get-VerifiedPreservation `
            -Root $Fixture `
            -DownloadRoot $FixtureDownloads `
            -RequiredHead $FixtureHead `
            -RequiredIndexTree $FixtureTree
        if ($FoundOriginal.stash_commit -ne
                $OriginalPreservation.stash_commit -or
            $FoundOriginal.backup_branch -ne
                $OriginalPreservation.backup_branch -or
            $FoundOriginal.bundle -ne $OriginalPreservation.bundle) {
            throw 'Recovery resume did not select the exact preservation.'
        }

        [System.IO.File]::WriteAllText(
            $FixtureFile,
            'interrupted branch switch residue' + [Environment]::NewLine,
            (New-Object System.Text.UTF8Encoding($false)))
        Invoke-RecoveryGit -Arguments @('-C', $Fixture, 'add', 'candidate.txt')
        $InterruptedTree = Invoke-RecoveryGit -Arguments @(
            '-C', $Fixture, 'write-tree') -Capture
        if ($InterruptedTree -eq $FixtureTree) {
            throw 'Interrupted-switch fixture did not produce a distinct tree.'
        }

        # The original backup must be verifiable before the generated residue
        # is moved out of the index and working tree.
        $FoundWhileDirty = Get-VerifiedPreservation `
            -Root $Fixture `
            -DownloadRoot $FixtureDownloads `
            -RequiredHead $FixtureHead `
            -RequiredIndexTree $FixtureTree
        if ($FoundWhileDirty.stash_commit -ne
            $OriginalPreservation.stash_commit) {
            throw 'Original preservation was unavailable while residue existed.'
        }

        $InterruptedPreservation = Save-ExactStagedState `
            -Root $Fixture `
            -DownloadRoot $FixtureDownloads `
            -State (Get-RecoveryState -Root $Fixture) `
            -RequiredHead $FixtureHead `
            -RequiredIndexTree $InterruptedTree `
            -RequiredBranch 'main' `
            -BackupBranchPrefix 'backup/ai100-interrupted-switch-' `
            -BundleFilePrefix 'PRESERVED_MechoFly_AI100_InterruptedSwitch' `
            -StashMessagePrefix 'interrupted branch switch self-test'
        if (-not (Test-Path -LiteralPath $InterruptedPreservation.bundle `
            -PathType Leaf)) {
            throw 'Interrupted-switch preservation bundle was not created.'
        }

        $FinalState = Get-RecoveryState -Root $Fixture
        if (-not [string]::IsNullOrWhiteSpace([string]$FinalState.status) -or
            [string]$FinalState.branch -ne 'main' -or
            [string]$FinalState.head -ne $FixtureHead) {
            throw 'Interrupted-switch recovery did not restore the clean base.'
        }

        $FoundAfterResidue = Get-VerifiedPreservation `
            -Root $Fixture `
            -DownloadRoot $FixtureDownloads `
            -RequiredHead $FixtureHead `
            -RequiredIndexTree $FixtureTree
        if ($FoundAfterResidue.stash_commit -ne
            $OriginalPreservation.stash_commit) {
            throw 'Original preservation changed after residue preservation.'
        }
    }
    finally {
        if (Test-Path -LiteralPath $FixtureRoot -PathType Container) {
            Remove-Item -LiteralPath $FixtureRoot -Recurse -Force
        }
    }
    Write-Host 'MECHOFLY_RECOVERY_GUARD_AND_RESUME_SELF_TEST=PASS'
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
$ExpectedInterruptedCommit = $ExpectedInterruptedCommit.ToLowerInvariant()

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
Invoke-RecoveryGit -Arguments @(
    '-C', $RepositoryRoot, 'merge-base', '--is-ancestor',
    $ExpectedInterruptedCommit, $TargetCommit)
$ExpectedInterruptedTree = Invoke-RecoveryGit -Arguments @(
    '-C', $RepositoryRoot, 'rev-parse',
    ($ExpectedInterruptedCommit + '^{tree}')) -Capture
$ExpectedBaseTree = Invoke-RecoveryGit -Arguments @(
    '-C', $RepositoryRoot, 'rev-parse',
    ($ExpectedDirtyHead + '^{tree}')) -Capture

$State = Get-RecoveryState -Root $RepositoryRoot
$RecoveryMode = ''
$InterruptedPreservation = $null
if (-not [string]::IsNullOrWhiteSpace([string]$State.status)) {
    if ([string]$State.index_tree -eq $ExpectedDirtyIndexTree) {
        $Preservation = Save-ExactStagedState `
            -Root $RepositoryRoot `
            -DownloadRoot $Downloads `
            -State $State `
            -RequiredHead $ExpectedDirtyHead `
            -RequiredIndexTree $ExpectedDirtyIndexTree `
            -RequiredBranch $ExpectedDirtyBranch `
            -BackupBranchPrefix 'backup/ai100-pre-sync-' `
            -BundleFilePrefix 'PRESERVED_MechoFly_AI100_PreSync' `
            -StashMessagePrefix 'AI100 exact candidate pre-sync preservation'
        $RecoveryMode = 'PRESERVED_NOW'
    }
    elseif ([string]$State.index_tree -eq $ExpectedInterruptedTree) {
        Assert-ExactRecoverableState `
            -State $State `
            -RequiredHead $ExpectedDirtyHead `
            -RequiredIndexTree $ExpectedInterruptedTree `
            -RequiredBranch $ExpectedDirtyBranch

        # Refuse to touch the generated residue unless the user's original
        # staged tree is already independently recoverable from both a local
        # backup branch and a complete standalone Downloads bundle.
        $OriginalBefore = Get-VerifiedPreservation `
            -Root $RepositoryRoot `
            -DownloadRoot $Downloads `
            -RequiredHead $ExpectedDirtyHead `
            -RequiredIndexTree $ExpectedDirtyIndexTree

        $InterruptedPreservation = Save-ExactStagedState `
            -Root $RepositoryRoot `
            -DownloadRoot $Downloads `
            -State $State `
            -RequiredHead $ExpectedDirtyHead `
            -RequiredIndexTree $ExpectedInterruptedTree `
            -RequiredBranch $ExpectedDirtyBranch `
            -BackupBranchPrefix 'backup/ai100-interrupted-switch-' `
            -BundleFilePrefix 'PRESERVED_MechoFly_AI100_InterruptedSwitch' `
            -StashMessagePrefix 'AI100 interrupted branch-switch residue'

        $Preservation = Get-VerifiedPreservation `
            -Root $RepositoryRoot `
            -DownloadRoot $Downloads `
            -RequiredHead $ExpectedDirtyHead `
            -RequiredIndexTree $ExpectedDirtyIndexTree
        if ($Preservation.stash_commit -ne $OriginalBefore.stash_commit -or
            $Preservation.bundle -ne $OriginalBefore.bundle) {
            throw 'Original preservation identity changed during residue repair.'
        }
        $RecoveryMode = 'RECOVERED_INTERRUPTED_BRANCH_SWITCH'
    }
    else {
        Assert-ExactRecoverableState `
            -State $State `
            -RequiredHead $ExpectedDirtyHead `
            -RequiredIndexTree $ExpectedDirtyIndexTree `
            -RequiredBranch $ExpectedDirtyBranch
    }
}
else {
    $AtPreservedBase = (
        [string]$State.branch -eq $ExpectedDirtyBranch -and
        [string]$State.head -eq $ExpectedDirtyHead -and
        [string]$State.index_tree -eq $ExpectedBaseTree)
    $AtExactTarget = (
        [string]$State.branch -eq $TargetBranch -and
        [string]$State.head -eq $TargetCommit -and
        [string]$State.index_tree -eq $RemoteTree)
    if (-not $AtPreservedBase -and -not $AtExactTarget) {
        throw ('Recovery cannot resume because the checkout is clean but is ' +
            'neither the preserved base nor the exact target. Branch=' +
            [string]$State.branch + '; HEAD=' + [string]$State.head +
            '; tree=' + [string]$State.index_tree)
    }

    $Preservation = Get-VerifiedPreservation `
        -Root $RepositoryRoot `
        -DownloadRoot $Downloads `
        -RequiredHead $ExpectedDirtyHead `
        -RequiredIndexTree $ExpectedDirtyIndexTree
    $RecoveryMode = 'RESUMED_AFTER_PRESERVATION'
}

$StashCommit = $Preservation.stash_commit
$BackupBranch = $Preservation.backup_branch
$BundlePath = $Preservation.bundle
Write-Host ('MECHOFLY_RECOVERY_MODE=' + $RecoveryMode)
Write-Host ('MECHOFLY_PRESERVED_STASH=' + $StashCommit)
Write-Host ('MECHOFLY_PRESERVED_BRANCH=' + $BackupBranch)
Write-Host ('MECHOFLY_PRESERVED_BUNDLE=' + $BundlePath)
if ($null -ne $InterruptedPreservation) {
    Write-Host ('MECHOFLY_INTERRUPTED_SWITCH_COMMIT=' +
        $ExpectedInterruptedCommit)
    Write-Host ('MECHOFLY_INTERRUPTED_SWITCH_TREE=' +
        $ExpectedInterruptedTree)
    Write-Host ('MECHOFLY_INTERRUPTED_SWITCH_STASH=' +
        $InterruptedPreservation.stash_commit)
    Write-Host ('MECHOFLY_INTERRUPTED_SWITCH_BRANCH=' +
        $InterruptedPreservation.backup_branch)
    Write-Host ('MECHOFLY_INTERRUPTED_SWITCH_BUNDLE=' +
        $InterruptedPreservation.bundle)
}

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
