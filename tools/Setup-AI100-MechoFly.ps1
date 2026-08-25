#requires -version 5.1
[CmdletBinding()]
param(
    [switch] $Launch,

    [switch] $GitCaptureSelfTest
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
        '--branch', 'main',
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

    $Branch = Invoke-MechoFlyGit -Arguments @('-C', $Target, 'rev-parse', '--abbrev-ref', 'HEAD') -Capture
    if ($Branch -ne 'main') {
        throw ('Refusing to sync branch ' + $Branch + '; AI100 must remain on main.')
    }

    Invoke-MechoFlyGit -Arguments @('-C', $Target, 'fetch', '--prune', 'origin', 'main')
    Invoke-MechoFlyGit -Arguments @('-C', $Target, 'merge', '--ff-only', 'origin/main')
}

$LocalCommit = Invoke-MechoFlyGit -Arguments @('-C', $Target, 'rev-parse', 'HEAD') -Capture
$RemoteCommit = Invoke-MechoFlyGit -Arguments @('-C', $Target, 'rev-parse', 'origin/main') -Capture
if ($LocalCommit -ne $RemoteCommit) {
    throw 'AI100 did not converge to the exact origin/main commit.'
}

$ProfileDirectory = Join-Path $env:LOCALAPPDATA 'MechoFly'
$ProfilePath = Join-Path $ProfileDirectory 'runtime-profile.json'
if (-not (Test-Path -LiteralPath $ProfileDirectory -PathType Container)) {
    New-Item -ItemType Directory -Path $ProfileDirectory -Force | Out-Null
}
$Profile = [ordered]@{
    schema_version = 2
    machine_role = 'ai100-development'
    skin = 'firefly'
    compute = 'auto'
    reduced_motion = $false
    canonical_repository = $CanonicalRepository
    workspace = $Target
    main_commit = $LocalCommit
    live_hardware_authority = 'NONE'
    generated_utc = [DateTime]::UtcNow.ToString('o')
}
$ProfileJson = $Profile | ConvertTo-Json -Depth 3
[System.IO.File]::WriteAllText($ProfilePath, $ProfileJson + [Environment]::NewLine,
    (New-Object System.Text.UTF8Encoding($false)))

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
    $Receipt.implementation -ne 'independent-rust-rebuild') {
    throw 'MechoFly self-test receipt did not satisfy the AI100 safety checks.'
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

Write-Host ('MECHOFLY_AI100_SYNC=PASS commit=' + $LocalCommit)
Write-Host ('MECHOFLY_AI100_WORKSPACE=' + $Target)
Write-Host ('MECHOFLY_AI100_PROFILE=' + $ProfilePath)
Write-Host ('MECHOFLY_LEGACY_SHORTCUTS_REMOVED=' + [string]$RemovedShortcutCount)
$CreatedShortcuts | ForEach-Object { Write-Host ('MECHOFLY_SHORTCUT=' + $_) }
Write-Host ('MECHOFLY_SELF_TEST_RECEIPT=' + $ReceiptPath)
