#requires -version 5.1
[CmdletBinding()]
param(
    [switch] $Launch,

    [switch] $GitCaptureSelfTest,

    [switch] $BranchSwitchSelfTest,

    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._/-]{0,127}$')]
    [string] $Branch = 'main',

    [ValidatePattern('^(|[0-9a-fA-F]{40})$')]
    [string] $ExpectedCommit = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$CanonicalRepository = 'https://github.com/yaelkroy/MechoFly.git'
$Target = 'D:\Projects\MechoFly'
$Parent = Split-Path -Parent $Target
$LegacyProduct = 'Desktop' + 'Fly'

function Invoke-MechoFlyGit {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyCollection()]
        [string[]] $Arguments,

        [switch] $Capture
    )

    $StandardErrorPath = [System.IO.Path]::Combine(
        [System.IO.Path]::GetTempPath(),
        'MechoFly-Git-Stderr-' + [Guid]::NewGuid().ToString('N') + '.txt')
    $GitOutput = @()
    $GitStandardError = ''
    $GitExitCode = -1
    $PreviousErrorActionPreference = $ErrorActionPreference
    try {
        # Windows PowerShell 5.1 converts native stderr merged with 2>&1 into
        # NativeCommandError records. With ErrorActionPreference=Stop, normal
        # Git progress would terminate the setup before LASTEXITCODE is read.
        # Keep stdout and stderr on separate channels. PS5.1 still applies the
        # preference to redirected native stderr, so relax it only around the
        # process and then make the captured exit code authoritative.
        $ErrorActionPreference = 'Continue'
        $GitOutput = @(& $script:GitExecutable @Arguments 2> $StandardErrorPath)
        $GitExitCode = $LASTEXITCODE
        if (Test-Path -LiteralPath $StandardErrorPath -PathType Leaf) {
            $GitStandardError = [System.IO.File]::ReadAllText($StandardErrorPath)
        }
    }
    finally {
        $ErrorActionPreference = $PreviousErrorActionPreference
        Remove-Item -LiteralPath $StandardErrorPath -Force -ErrorAction SilentlyContinue
    }

    $GitStandardOutput = (($GitOutput | ForEach-Object { [string]$_ }) -join
        [Environment]::NewLine).Trim()
    if ($GitExitCode -ne 0) {
        $Details = @($GitStandardOutput, $GitStandardError.Trim()) |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
        throw ('git {0} failed with exit code {1}.{2}{3}' -f
            ($Arguments -join ' '),
            $GitExitCode,
            [Environment]::NewLine,
            ($Details -join [Environment]::NewLine))
    }

    if ($Capture) {
        return $GitStandardOutput
    }
    if (-not [string]::IsNullOrWhiteSpace($GitStandardOutput)) {
        Write-Host $GitStandardOutput
    }
    if (-not [string]::IsNullOrWhiteSpace($GitStandardError)) {
        Write-Host $GitStandardError.Trim()
    }
}

function ConvertTo-NormalizedRepositoryUrl {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Url
    )

    $Normalized = $Url.Trim().Replace('\', '/').TrimEnd('/')
    if ($Normalized.StartsWith('git@github.com:', [StringComparison]::OrdinalIgnoreCase)) {
        $Normalized = 'https://github.com/' + $Normalized.Substring('git@github.com:'.Length)
    }
    if ($Normalized.EndsWith('.git', [StringComparison]::OrdinalIgnoreCase)) {
        $Normalized = $Normalized.Substring(0, $Normalized.Length - 4)
    }
    return $Normalized.ToLowerInvariant()
}

function Switch-MechoFlyBranch {
    param(
        [Parameter(Mandatory = $true)]
        [string] $RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string] $BranchName,

        [Parameter(Mandatory = $true)]
        [string] $RemoteBranch
    )

    $CurrentBranch = Invoke-MechoFlyGit -Arguments @(
        '-C', $RepositoryRoot, 'rev-parse', '--abbrev-ref', 'HEAD') -Capture
    if ($CurrentBranch -eq $BranchName) {
        return
    }

    $ExistingBranch = Invoke-MechoFlyGit -Arguments @(
        '-C', $RepositoryRoot, 'branch', '--list',
        '--format=%(refname:short)', $BranchName) -Capture
    if ([string]::IsNullOrWhiteSpace($ExistingBranch)) {
        # A checkout originally cloned with --single-branch may contain an
        # explicitly fetched refs/remotes/origin/... ref that Git refuses as
        # a --track starting point because remote.origin.fetch still names
        # only the original branch. The explicit start point is sufficient;
        # setup always fetches and fast-forwards the named remote ref itself.
        Invoke-MechoFlyGit -Arguments @(
            '-C', $RepositoryRoot, 'switch', '--create', $BranchName,
            '--no-track', $RemoteBranch)
    }
    else {
        Invoke-MechoFlyGit -Arguments @(
            '-C', $RepositoryRoot, 'switch', $BranchName)
    }
}

function Invoke-BranchSwitchSelfTest {
    $FixtureRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
        'MechoFly-SingleBranch-SelfTest-' + [Guid]::NewGuid().ToString('N'))
    $Bare = Join-Path $FixtureRoot 'remote.git'
    $Seed = Join-Path $FixtureRoot 'seed'
    $Clone = Join-Path $FixtureRoot 'clone'
    New-Item -ItemType Directory -Path $FixtureRoot -Force | Out-Null
    try {
        Invoke-MechoFlyGit -Arguments @('init', '--bare', $Bare)
        Invoke-MechoFlyGit -Arguments @('init', $Seed)
        Invoke-MechoFlyGit -Arguments @(
            '-C', $Seed, 'config', 'user.name', 'MechoFly Branch Test')
        Invoke-MechoFlyGit -Arguments @(
            '-C', $Seed, 'config', 'user.email', 'branch-test@invalid')
        [System.IO.File]::WriteAllText(
            (Join-Path $Seed 'fixture.txt'),
            'main' + [Environment]::NewLine,
            (New-Object System.Text.UTF8Encoding($false)))
        Invoke-MechoFlyGit -Arguments @('-C', $Seed, 'add', 'fixture.txt')
        Invoke-MechoFlyGit -Arguments @(
            '-C', $Seed, 'commit', '-m', 'fixture main')
        Invoke-MechoFlyGit -Arguments @('-C', $Seed, 'branch', '-M', 'main')
        Invoke-MechoFlyGit -Arguments @(
            '-C', $Seed, 'remote', 'add', 'origin', $Bare)
        Invoke-MechoFlyGit -Arguments @(
            '-C', $Seed, 'push', '--set-upstream', 'origin', 'main')
        Invoke-MechoFlyGit -Arguments @(
            '-C', $Bare, 'symbolic-ref', 'HEAD', 'refs/heads/main')

        Invoke-MechoFlyGit -Arguments @(
            '-C', $Seed, 'switch', '--create', 'feat/test')
        [System.IO.File]::WriteAllText(
            (Join-Path $Seed 'fixture.txt'),
            'feature' + [Environment]::NewLine,
            (New-Object System.Text.UTF8Encoding($false)))
        Invoke-MechoFlyGit -Arguments @(
            '-C', $Seed, 'commit', '--all', '-m', 'fixture feature')
        Invoke-MechoFlyGit -Arguments @(
            '-C', $Seed, 'push', '--set-upstream', 'origin', 'feat/test')

        Invoke-MechoFlyGit -Arguments @(
            'clone', '--single-branch', '--branch', 'main', $Bare, $Clone)
        Invoke-MechoFlyGit -Arguments @(
            '-C', $Clone, 'fetch', 'origin',
            'refs/heads/feat/test:refs/remotes/origin/feat/test')
        $ConfiguredFetch = Invoke-MechoFlyGit -Arguments @(
            '-C', $Clone, 'config', '--get-all', 'remote.origin.fetch') -Capture
        if ($ConfiguredFetch.Contains('feat/test')) {
            throw 'Fixture did not retain its single-branch fetch configuration.'
        }

        Switch-MechoFlyBranch `
            -RepositoryRoot $Clone `
            -BranchName 'feat/test' `
            -RemoteBranch 'origin/feat/test'
        $LocalBranch = Invoke-MechoFlyGit -Arguments @(
            '-C', $Clone, 'branch', '--show-current') -Capture
        $LocalCommit = Invoke-MechoFlyGit -Arguments @(
            '-C', $Clone, 'rev-parse', 'HEAD') -Capture
        $RemoteCommit = Invoke-MechoFlyGit -Arguments @(
            '-C', $Clone, 'rev-parse', 'origin/feat/test') -Capture
        $LocalUpstream = Invoke-MechoFlyGit -Arguments @(
            '-C', $Clone, 'for-each-ref',
            '--format=%(upstream:short)', 'refs/heads/feat/test') -Capture
        if ($LocalBranch -ne 'feat/test' -or $LocalCommit -ne $RemoteCommit -or
            -not [string]::IsNullOrWhiteSpace($LocalUpstream)) {
            throw 'Single-branch fixture did not switch to the fetched ref.'
        }
    }
    finally {
        if (Test-Path -LiteralPath $FixtureRoot -PathType Container) {
            Remove-Item -LiteralPath $FixtureRoot -Recurse -Force
        }
    }
    Write-Host 'MECHOFLY_SINGLE_BRANCH_SWITCH=PASS'
}

function New-MechoFlyShortcut {
    param(
        [Parameter(Mandatory = $true)]
        [object] $Shell,

        [Parameter(Mandatory = $true)]
        [string] $ShortcutPath,

        [Parameter(Mandatory = $true)]
        [string] $TargetPath,

        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string] $Arguments,

        [Parameter(Mandatory = $true)]
        [string] $Description,

        [Parameter(Mandatory = $true)]
        [string] $IconLocation
    )

    $Shortcut = $Shell.CreateShortcut($ShortcutPath)
    $Shortcut.TargetPath = $TargetPath
    $Shortcut.Arguments = $Arguments
    $Shortcut.WorkingDirectory = $script:Target
    $Shortcut.Description = $Description
    $Shortcut.IconLocation = $IconLocation
    $Shortcut.Save()
}

if ($GitCaptureSelfTest) {
    $FakeGitPath = [System.IO.Path]::Combine(
        [System.IO.Path]::GetTempPath(),
        'MechoFly-FakeGit-' + [Guid]::NewGuid().ToString('N') + '.cmd')
    $FakeGitLines = @(
        '@echo off',
        'echo NORMAL_GIT_PROGRESS 1>&2',
        'if /I "%~1"=="fail" exit /b 7',
        'echo CAPTURED_STDOUT',
        'exit /b 0'
    )
    try {
        [System.IO.File]::WriteAllLines(
            $FakeGitPath,
            $FakeGitLines,
            (New-Object System.Text.UTF8Encoding($false)))
        $script:GitExecutable = $FakeGitPath

        $CapturedOutput = Invoke-MechoFlyGit -Arguments @() -Capture
        if ($CapturedOutput -ne 'CAPTURED_STDOUT') {
            throw ('Git capture mixed channels: ' + $CapturedOutput)
        }
        Invoke-MechoFlyGit -Arguments @()

        $ExpectedFailureObserved = $false
        try {
            Invoke-MechoFlyGit -Arguments @('fail') -Capture | Out-Null
        }
        catch {
            $FailureText = $_.Exception.Message
            $ExpectedFailureObserved =
                $FailureText.Contains('exit code 7') -and
                $FailureText.Contains('NORMAL_GIT_PROGRESS')
        }
        if (-not $ExpectedFailureObserved) {
            throw 'Git failure-channel regression was not detected correctly.'
        }
    }
    finally {
        Remove-Item -LiteralPath $FakeGitPath -Force -ErrorAction SilentlyContinue
    }
    Write-Host 'MECHOFLY_PS51_GIT_STDERR_CAPTURE=PASS'
    exit 0
}

$GitCommand = Get-Command 'git.exe' -ErrorAction SilentlyContinue
if ($null -eq $GitCommand) {
    $GitCommand = Get-Command 'git' -ErrorAction SilentlyContinue
}
if ($null -eq $GitCommand) {
    throw 'Git for Windows was not found on PATH.'
}
$script:GitExecutable = $GitCommand.Source
$script:Target = $Target
if ($BranchSwitchSelfTest) {
    Invoke-BranchSwitchSelfTest
    exit 0
}
Invoke-MechoFlyGit -Arguments @('check-ref-format', '--branch', $Branch)
$RemoteRef = 'origin/' + $Branch
$FetchRef = 'refs/heads/' + $Branch + ':refs/remotes/origin/' + $Branch

if (-not (Test-Path -LiteralPath 'D:\' -PathType Container)) {
    throw 'AI100 setup requires the D: drive.'
}
if (-not (Test-Path -LiteralPath $Parent -PathType Container)) {
    New-Item -ItemType Directory -Path $Parent -Force | Out-Null
}

$RepositoryExists = Test-Path -LiteralPath (Join-Path $Target '.git') -PathType Container
if (-not $RepositoryExists -and (Test-Path -LiteralPath $Target -PathType Container)) {
    $ExistingItems = @(Get-ChildItem -LiteralPath $Target -Force)
    if ($ExistingItems.Count -gt 0) {
        throw ('Refusing to replace non-repository content in ' + $Target)
    }
}

if (-not $RepositoryExists) {
    Invoke-MechoFlyGit -Arguments @(
        'clone',
        '--branch', $Branch,
        '--single-branch',
        $CanonicalRepository,
        $Target
    )
}
else {
    $Origin = Invoke-MechoFlyGit -Arguments @('-C', $Target, 'remote', 'get-url', 'origin') -Capture
    if ((ConvertTo-NormalizedRepositoryUrl -Url $Origin) -ne
        (ConvertTo-NormalizedRepositoryUrl -Url $CanonicalRepository)) {
        throw ('Refusing to sync a checkout whose origin is not ' + $CanonicalRepository)
    }

    $WorkingTreeStatus = Invoke-MechoFlyGit -Arguments @('-C', $Target, 'status', '--porcelain') -Capture
    if (-not [string]::IsNullOrWhiteSpace($WorkingTreeStatus)) {
        throw ('Refusing to sync a dirty working tree in ' + $Target)
    }

    Invoke-MechoFlyGit -Arguments @(
        '-C', $Target, 'fetch', '--prune', 'origin', $FetchRef)
    $RemoteCommitBeforeSwitch = Invoke-MechoFlyGit -Arguments @(
        '-C', $Target, 'rev-parse', $RemoteRef) -Capture
    if (-not [string]::IsNullOrWhiteSpace($ExpectedCommit) -and
        $RemoteCommitBeforeSwitch -ne $ExpectedCommit.ToLowerInvariant()) {
        throw ('GitHub branch ' + $Branch + ' is at ' +
            $RemoteCommitBeforeSwitch + '; expected pinned commit ' +
            $ExpectedCommit.ToLowerInvariant() + '.')
    }

    Switch-MechoFlyBranch `
        -RepositoryRoot $Target `
        -BranchName $Branch `
        -RemoteBranch $RemoteRef
    Invoke-MechoFlyGit -Arguments @(
        '-C', $Target, 'merge', '--ff-only', $RemoteRef)
}

$LocalCommit = Invoke-MechoFlyGit -Arguments @('-C', $Target, 'rev-parse', 'HEAD') -Capture
$RemoteCommit = Invoke-MechoFlyGit -Arguments @('-C', $Target, 'rev-parse', $RemoteRef) -Capture
if ($LocalCommit -ne $RemoteCommit) {
    throw ('AI100 did not converge to the exact ' + $RemoteRef + ' commit.')
}
$LocalTree = Invoke-MechoFlyGit -Arguments @(
    '-C', $Target, 'rev-parse', 'HEAD^{tree}') -Capture
$RemoteTree = Invoke-MechoFlyGit -Arguments @(
    '-C', $Target, 'rev-parse', ($RemoteRef + '^{tree}')) -Capture
if ($LocalTree -ne $RemoteTree) {
    throw ('AI100 tree does not match the exact ' + $RemoteRef + ' tree.')
}
if (-not [string]::IsNullOrWhiteSpace($ExpectedCommit) -and
    $LocalCommit -ne $ExpectedCommit.ToLowerInvariant()) {
    throw ('AI100 commit ' + $LocalCommit + ' differs from pinned commit ' +
        $ExpectedCommit.ToLowerInvariant() + '.')
}

& (Join-Path $Target 'tools\Verify-Tree.ps1')
& (Join-Path $Target 'host-windows\build.ps1')

$ArtifactDirectory = Join-Path $Target 'artifacts'
if (-not (Test-Path -LiteralPath $ArtifactDirectory -PathType Container)) {
    New-Item -ItemType Directory -Path $ArtifactDirectory -Force | Out-Null
}
$ReceiptPath = Join-Path $ArtifactDirectory 'ai100-self-test.json'
$Executable = Join-Path $Target 'host-windows\bin\MechoFly.exe'
$SelfTestArguments = '--self-test "{0}"' -f $ReceiptPath
$SelfTestProcess = Start-Process `
    -FilePath $Executable `
    -WorkingDirectory $Target `
    -ArgumentList $SelfTestArguments `
    -Wait `
    -PassThru
if ($SelfTestProcess.ExitCode -ne 0) {
    throw ('MechoFly self-test exited with code ' + [string]$SelfTestProcess.ExitCode)
}
if (-not (Test-Path -LiteralPath $ReceiptPath -PathType Leaf)) {
    throw 'MechoFly self-test did not create its receipt.'
}
$Receipt = Get-Content -LiteralPath $ReceiptPath -Raw | ConvertFrom-Json
if ($Receipt.status -ne 'PASS' -or -not $Receipt.live_state_unchanged -or
    $Receipt.default_skin -ne 'drosophila' -or -not $Receipt.firefly_skin_available -or
    -not $Receipt.cpu_without_gpu_supported -or -not $Receipt.reevaluation_control -or
    -not $Receipt.global_hotkey_contract_passed -or
    $Receipt.global_hotkey_count -ne 8 -or
    -not $Receipt.asynchronous_hotkey_fallback -or
    $Receipt.firefly_visual_style -ne 'neurofly_prism_firefly' -or
    -not $Receipt.firefly_visual_contract_passed -or
    -not $Receipt.firefly_rest_temporal_invariant -or
    -not $Receipt.firefly_escape_wing_responsive -or
    $Receipt.anatomical_context_points -ne 23210 -or
    $Receipt.anatomical_context_measured -or
    $Receipt.implementation -ne 'independent-rust-rebuild') {
    throw 'MechoFly self-test receipt did not satisfy the AI100 safety checks.'
}

# Recheck GitHub after the build so the receipt cannot silently describe a
# branch that advanced while AI100 was compiling. No tracked source file may
# have changed during the build.
Invoke-MechoFlyGit -Arguments @(
    '-C', $Target, 'fetch', '--prune', 'origin', $FetchRef)
$FinalBranch = Invoke-MechoFlyGit -Arguments @(
    '-C', $Target, 'rev-parse', '--abbrev-ref', 'HEAD') -Capture
$FinalCommit = Invoke-MechoFlyGit -Arguments @(
    '-C', $Target, 'rev-parse', 'HEAD') -Capture
$FinalTree = Invoke-MechoFlyGit -Arguments @(
    '-C', $Target, 'rev-parse', 'HEAD^{tree}') -Capture
$FinalRemoteCommit = Invoke-MechoFlyGit -Arguments @(
    '-C', $Target, 'rev-parse', $RemoteRef) -Capture
$FinalRemoteTree = Invoke-MechoFlyGit -Arguments @(
    '-C', $Target, 'rev-parse', ($RemoteRef + '^{tree}')) -Capture
$FinalStatus = Invoke-MechoFlyGit -Arguments @(
    '-C', $Target, 'status', '--porcelain=v1', '--untracked-files=all') -Capture
if ($FinalBranch -ne $Branch -or
    $FinalCommit -ne $LocalCommit -or
    $FinalTree -ne $LocalTree -or
    $FinalRemoteCommit -ne $LocalCommit -or
    $FinalRemoteTree -ne $LocalTree -or
    -not [string]::IsNullOrWhiteSpace($FinalStatus)) {
    throw ('AI100 source identity changed during the build. Rerun setup; ' +
        'no unverified executable will be installed.')
}

$ExecutableHash = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash
$ProfileDirectory = Join-Path $env:LOCALAPPDATA 'MechoFly'
$ProfilePath = Join-Path $ProfileDirectory 'runtime-profile.json'
if (-not (Test-Path -LiteralPath $ProfileDirectory -PathType Container)) {
    New-Item -ItemType Directory -Path $ProfileDirectory -Force | Out-Null
}
$GeneratedUtc = [DateTime]::UtcNow.ToString('o')
$Profile = [ordered]@{
    schema_version = 3
    machine_role = 'ai100-development'
    skin = 'firefly'
    compute = 'auto'
    reduced_motion = $false
    canonical_repository = $CanonicalRepository
    workspace = $Target
    source_branch = $Branch
    source_commit = $LocalCommit
    source_tree = $LocalTree
    source_dirty = $false
    executable_sha256 = $ExecutableHash
    live_hardware_authority = 'NONE'
    generated_utc = $GeneratedUtc
}
$ProfileJson = $Profile | ConvertTo-Json -Depth 4
[System.IO.File]::WriteAllText(
    $ProfilePath,
    $ProfileJson + [Environment]::NewLine,
    (New-Object System.Text.UTF8Encoding($false)))

$SourceIdentityPath = Join-Path $ArtifactDirectory 'ai100-source-identity.json'
$SourceIdentity = [ordered]@{
    schema_version = 1
    status = 'PASS'
    canonical_repository = $CanonicalRepository
    workspace = $Target
    source_branch = $Branch
    source_commit = $LocalCommit
    source_tree = $LocalTree
    source_dirty = $false
    remote_commit = $FinalRemoteCommit
    remote_tree = $FinalRemoteTree
    remote_exact_match = $true
    failure = $null
    executable = $Executable
    executable_sha256 = $ExecutableHash
    self_test_receipt = $ReceiptPath
    self_test_status = [string]$Receipt.status
    live_state_unchanged = [bool]$Receipt.live_state_unchanged
    live_hardware_authority = 'NONE'
    generated_utc = $GeneratedUtc
}
$SourceIdentityJson = $SourceIdentity | ConvertTo-Json -Depth 5
[System.IO.File]::WriteAllText(
    $SourceIdentityPath,
    $SourceIdentityJson + [Environment]::NewLine,
    (New-Object System.Text.UTF8Encoding($false)))

try {
    & (Join-Path $Target 'tools\Assert-AI100-SourceIdentity.ps1') `
        -RepositoryRoot $Target `
        -ProfilePath $ProfilePath `
        -RefreshRemote
}
catch {
    # If the branch moved during the final network check, leave an explicit
    # FAIL receipt so the Start shortcut cannot run the now-stale build.
    $SourceIdentity['status'] = 'FAIL'
    $SourceIdentity['remote_exact_match'] = $false
    $SourceIdentity['failure'] = $_.Exception.Message
    $SourceIdentityJson = $SourceIdentity | ConvertTo-Json -Depth 5
    [System.IO.File]::WriteAllText(
        $SourceIdentityPath,
        $SourceIdentityJson + [Environment]::NewLine,
        (New-Object System.Text.UTF8Encoding($false)))
    throw
}

$DesktopDirectories = @(
    [Environment]::GetFolderPath('Desktop'),
    [Environment]::GetFolderPath('CommonDesktopDirectory')
) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -Unique
$LegacyShortcutNames = @(
    ($LegacyProduct + '.lnk'),
    ('Start ' + $LegacyProduct + '.lnk'),
    ('Stop ' + $LegacyProduct + '.lnk'),
    ('Emergency Stop ' + $LegacyProduct + '.lnk'),
    ($LegacyProduct + ' Prism.lnk'),
    ($LegacyProduct + ' Brain Lab.lnk'),
    ($LegacyProduct + ' Live Brain.lnk')
)
$RemovedShortcutCount = 0
foreach ($DesktopDirectory in $DesktopDirectories) {
    foreach ($LegacyShortcutName in $LegacyShortcutNames) {
        $LegacyShortcutPath = Join-Path $DesktopDirectory $LegacyShortcutName
        if (Test-Path -LiteralPath $LegacyShortcutPath -PathType Leaf) {
            Remove-Item -LiteralPath $LegacyShortcutPath -Force
            $RemovedShortcutCount++
        }
    }
}

$UserDesktop = [Environment]::GetFolderPath('Desktop')
if ([string]::IsNullOrWhiteSpace($UserDesktop)) {
    throw 'The current user Desktop directory could not be resolved.'
}
$WindowsPowerShell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$IconLocation = $Executable + ',0'
$Shell = New-Object -ComObject WScript.Shell
$CreatedShortcuts = New-Object System.Collections.Generic.List[string]
try {
    $StartShortcut = Join-Path $UserDesktop 'Start MechoFly.lnk'
    New-MechoFlyShortcut `
        -Shell $Shell `
        -ShortcutPath $StartShortcut `
        -TargetPath $WindowsPowerShell `
        -Arguments ('-NoLogo -NoProfile -ExecutionPolicy Bypass -File "' +
            (Join-Path $Target 'host-windows\Start-MechoFly-AI100.ps1') + '"') `
        -Description 'Start MechoFly with the AI100 Firefly skin.' `
        -IconLocation $IconLocation
    $CreatedShortcuts.Add($StartShortcut)

    $SyncShortcut = Join-Path $UserDesktop 'Sync MechoFly with GitHub.lnk'
    New-MechoFlyShortcut `
        -Shell $Shell `
        -ShortcutPath $SyncShortcut `
        -TargetPath $WindowsPowerShell `
        -Arguments ('-NoLogo -NoProfile -ExecutionPolicy Bypass -File "' +
            (Join-Path $Target 'tools\Setup-AI100-MechoFly.ps1') +
            '" -Branch "' + $Branch + '"') `
        -Description ('Fast-forward, rebuild, and verify MechoFly from origin/' +
            $Branch + '.') `
        -IconLocation $IconLocation
    $CreatedShortcuts.Add($SyncShortcut)

    $EvidenceShortcut = Join-Path $UserDesktop 'Capture MechoFly Evidence.lnk'
    New-MechoFlyShortcut `
        -Shell $Shell `
        -ShortcutPath $EvidenceShortcut `
        -TargetPath $WindowsPowerShell `
        -Arguments ('-NoLogo -NoProfile -ExecutionPolicy Bypass -File "' +
            (Join-Path $Target 'tools\Capture-AI100-Evidence.ps1') + '"') `
        -Description 'Capture exact-source logs and cropped design screenshots.' `
        -IconLocation $IconLocation
    $CreatedShortcuts.Add($EvidenceShortcut)

    $StopShortcut = Join-Path $UserDesktop 'Stop MechoFly.lnk'
    New-MechoFlyShortcut `
        -Shell $Shell `
        -ShortcutPath $StopShortcut `
        -TargetPath $WindowsPowerShell `
        -Arguments ('-NoLogo -NoProfile -ExecutionPolicy Bypass -File "' +
            (Join-Path $Target 'host-windows\Stop-MechoFly.ps1') + '"') `
        -Description 'Stop MechoFly.' `
        -IconLocation $IconLocation
    $CreatedShortcuts.Add($StopShortcut)

    $EmergencyShortcut = Join-Path $UserDesktop 'Emergency Stop MechoFly.lnk'
    New-MechoFlyShortcut `
        -Shell $Shell `
        -ShortcutPath $EmergencyShortcut `
        -TargetPath (Join-Path $Target 'host-windows\Emergency-Stop-MechoFly.cmd') `
        -Arguments '' `
        -Description 'Force-stop every MechoFly process.' `
        -IconLocation $IconLocation
    $CreatedShortcuts.Add($EmergencyShortcut)
}
finally {
    if ($null -ne $Shell) {
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($Shell)
    }
}

if ($Launch) {
    & (Join-Path $Target 'host-windows\Start-MechoFly-AI100.ps1')
}

Write-Host ('MECHOFLY_AI100_SYNC=PASS branch=' + $Branch +
    ' commit=' + $LocalCommit + ' tree=' + $LocalTree)
Write-Host ('MECHOFLY_AI100_WORKSPACE=' + $Target)
Write-Host ('MECHOFLY_AI100_PROFILE=' + $ProfilePath)
Write-Host ('MECHOFLY_AI100_SOURCE_IDENTITY=' + $SourceIdentityPath)
Write-Host ('MECHOFLY_EXECUTABLE_SHA256=' + $ExecutableHash)
Write-Host ('MECHOFLY_LEGACY_SHORTCUTS_REMOVED=' + [string]$RemovedShortcutCount)
$CreatedShortcuts | ForEach-Object { Write-Host ('MECHOFLY_SHORTCUT=' + $_) }
Write-Host ('MECHOFLY_SELF_TEST_RECEIPT=' + $ReceiptPath)
