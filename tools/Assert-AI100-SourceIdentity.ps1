#requires -version 5.1
[CmdletBinding()]
param(
    [string] $RepositoryRoot = (Split-Path -Parent $PSScriptRoot),

    [string] $ProfilePath = (Join-Path $env:LOCALAPPDATA `
        'MechoFly\runtime-profile.json'),

    [switch] $RefreshRemote,

    [switch] $PassThru
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$CanonicalRepository = 'https://github.com/yaelkroy/MechoFly.git'

function Invoke-IdentityGit {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyCollection()]
        [string[]] $Arguments
    )

    $StandardErrorPath = [System.IO.Path]::Combine(
        [System.IO.Path]::GetTempPath(),
        'MechoFly-Identity-Git-' + [Guid]::NewGuid().ToString('N') + '.txt')
    $Output = @()
    $StandardError = ''
    $ExitCode = -1
    $PreviousErrorActionPreference = $ErrorActionPreference
    try {
        # Windows PowerShell 5.1 promotes ordinary native stderr to error
        # records. Keep channels separate and trust Git's exit code.
        $ErrorActionPreference = 'Continue'
        $Output = @(& $script:IdentityGitExecutable @Arguments 2> $StandardErrorPath)
        $ExitCode = $LASTEXITCODE
        if (Test-Path -LiteralPath $StandardErrorPath -PathType Leaf) {
            $StandardError = [System.IO.File]::ReadAllText($StandardErrorPath)
        }
    }
    finally {
        $ErrorActionPreference = $PreviousErrorActionPreference
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
    return $StandardOutput
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

function Get-RequiredProfileText {
    param(
        [Parameter(Mandatory = $true)]
        [object] $Profile,

        [Parameter(Mandatory = $true)]
        [string] $Name
    )

    $Property = $Profile.PSObject.Properties[$Name]
    if ($null -eq $Property -or
        [string]::IsNullOrWhiteSpace([string]$Property.Value)) {
        throw ('Runtime profile is missing required identity field ' + $Name + '.')
    }
    return [string]$Property.Value
}

$GitCommand = Get-Command 'git.exe' -ErrorAction SilentlyContinue
if ($null -eq $GitCommand) {
    $GitCommand = Get-Command 'git' -ErrorAction SilentlyContinue
}
if ($null -eq $GitCommand) {
    throw 'Git for Windows was not found on PATH.'
}
$script:IdentityGitExecutable = $GitCommand.Source

if (-not (Test-Path -LiteralPath $RepositoryRoot -PathType Container)) {
    throw ('MechoFly repository was not found: ' + $RepositoryRoot)
}
$RepositoryRoot = (Resolve-Path -LiteralPath $RepositoryRoot).Path
if (-not (Test-Path -LiteralPath (Join-Path $RepositoryRoot '.git') `
    -PathType Container)) {
    throw ('MechoFly runtime root is not a Git checkout: ' + $RepositoryRoot)
}
if (-not (Test-Path -LiteralPath $ProfilePath -PathType Leaf)) {
    throw ('MechoFly runtime profile was not found: ' + $ProfilePath +
        '. Rerun tools\Setup-AI100-MechoFly.ps1.')
}

$Profile = Get-Content -LiteralPath $ProfilePath -Raw | ConvertFrom-Json
$ExpectedWorkspace = Get-RequiredProfileText -Profile $Profile -Name 'workspace'
$ExpectedRepository = Get-RequiredProfileText `
    -Profile $Profile `
    -Name 'canonical_repository'
$ExpectedBranch = Get-RequiredProfileText -Profile $Profile -Name 'source_branch'
$ExpectedCommit = Get-RequiredProfileText -Profile $Profile -Name 'source_commit'
$ExpectedTree = Get-RequiredProfileText -Profile $Profile -Name 'source_tree'
$ExpectedExecutableHash = Get-RequiredProfileText `
    -Profile $Profile `
    -Name 'executable_sha256'

$ResolvedExpectedWorkspace = [System.IO.Path]::GetFullPath(
    $ExpectedWorkspace).TrimEnd('\')
$ResolvedRepositoryRoot = [System.IO.Path]::GetFullPath(
    $RepositoryRoot).TrimEnd('\')
if (-not $ResolvedRepositoryRoot.Equals(
    $ResolvedExpectedWorkspace,
    [StringComparison]::OrdinalIgnoreCase)) {
    throw ('Runtime profile workspace does not match this checkout. Profile=' +
        $ResolvedExpectedWorkspace + '; checkout=' + $ResolvedRepositoryRoot)
}
if ((ConvertTo-NormalizedRepositoryUrl -Url $ExpectedRepository) -ne
    (ConvertTo-NormalizedRepositoryUrl -Url $CanonicalRepository)) {
    throw 'Runtime profile does not name the canonical MechoFly repository.'
}

Invoke-IdentityGit -Arguments @('check-ref-format', '--branch', $ExpectedBranch) |
    Out-Null
$Origin = Invoke-IdentityGit -Arguments @(
    '-C', $RepositoryRoot, 'remote', 'get-url', 'origin')
if ((ConvertTo-NormalizedRepositoryUrl -Url $Origin) -ne
    (ConvertTo-NormalizedRepositoryUrl -Url $CanonicalRepository)) {
    throw ('Checkout origin is not the canonical MechoFly repository: ' + $Origin)
}

$Status = Invoke-IdentityGit -Arguments @(
    '-C', $RepositoryRoot, 'status', '--porcelain=v1', '--untracked-files=all')
if (-not [string]::IsNullOrWhiteSpace($Status)) {
    throw ('Refusing to run an unrecorded dirty MechoFly checkout.' +
        [Environment]::NewLine + $Status)
}

$Branch = Invoke-IdentityGit -Arguments @(
    '-C', $RepositoryRoot, 'rev-parse', '--abbrev-ref', 'HEAD')
$Commit = Invoke-IdentityGit -Arguments @(
    '-C', $RepositoryRoot, 'rev-parse', 'HEAD')
$Tree = Invoke-IdentityGit -Arguments @(
    '-C', $RepositoryRoot, 'rev-parse', 'HEAD^{tree}')
if ($Branch -ne $ExpectedBranch) {
    throw ('Checkout branch differs from the installed profile. Expected ' +
        $ExpectedBranch + '; received ' + $Branch)
}
if ($Commit -ne $ExpectedCommit) {
    throw ('Checkout commit differs from the installed profile. Expected ' +
        $ExpectedCommit + '; received ' + $Commit)
}
if ($Tree -ne $ExpectedTree) {
    throw ('Checkout tree differs from the installed profile. Expected ' +
        $ExpectedTree + '; received ' + $Tree)
}

$RemoteCommit = $null
$RemoteTree = $null
if ($RefreshRemote) {
    $FetchRef = 'refs/heads/' + $Branch + ':refs/remotes/origin/' + $Branch
    Invoke-IdentityGit -Arguments @(
        '-C', $RepositoryRoot, 'fetch', '--prune', 'origin', $FetchRef) |
        Out-Null
    $RemoteRef = 'origin/' + $Branch
    $RemoteCommit = Invoke-IdentityGit -Arguments @(
        '-C', $RepositoryRoot, 'rev-parse', $RemoteRef)
    $RemoteTree = Invoke-IdentityGit -Arguments @(
        '-C', $RepositoryRoot, 'rev-parse', ($RemoteRef + '^{tree}'))
    if ($RemoteCommit -ne $Commit -or $RemoteTree -ne $Tree) {
        throw ('AI100 is not synchronized with GitHub branch ' + $Branch +
            '. Local=' + $Commit + '; remote=' + $RemoteCommit +
            '. Rerun tools\Setup-AI100-MechoFly.ps1 -Branch "' + $Branch + '".')
    }
}

$Executable = Join-Path $RepositoryRoot 'host-windows\bin\MechoFly.exe'
if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) {
    throw ('Installed MechoFly executable was not found: ' + $Executable)
}
$ExecutableHash = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash
if ($ExecutableHash -ne $ExpectedExecutableHash) {
    throw ('Installed executable hash differs from the build recorded for ' +
        $Commit + '. Rerun tools\Setup-AI100-MechoFly.ps1.')
}

$ReceiptPath = Join-Path $RepositoryRoot 'artifacts\ai100-source-identity.json'
if (-not (Test-Path -LiteralPath $ReceiptPath -PathType Leaf)) {
    throw ('AI100 source identity receipt was not found: ' + $ReceiptPath)
}
$Receipt = Get-Content -LiteralPath $ReceiptPath -Raw | ConvertFrom-Json
if ([string]$Receipt.status -ne 'PASS' -or
    [bool]$Receipt.source_dirty -or
    [string]$Receipt.source_branch -ne $Branch -or
    [string]$Receipt.source_commit -ne $Commit -or
    [string]$Receipt.source_tree -ne $Tree -or
    [string]$Receipt.executable_sha256 -ne $ExecutableHash) {
    throw 'AI100 source identity receipt does not match the runnable checkout.'
}

$Identity = [pscustomobject][ordered]@{
    schema_version = 1
    status = 'PASS'
    canonical_repository = $CanonicalRepository
    workspace = $RepositoryRoot
    source_branch = $Branch
    source_commit = $Commit
    source_tree = $Tree
    source_dirty = $false
    remote_checked = [bool]$RefreshRemote
    remote_commit = $RemoteCommit
    remote_tree = $RemoteTree
    executable = $Executable
    executable_sha256 = $ExecutableHash
    profile = $ProfilePath
    receipt = $ReceiptPath
    verified_utc = [DateTime]::UtcNow.ToString('o')
}

Write-Host ('MECHOFLY_SOURCE_IDENTITY=PASS branch=' + $Branch +
    ' commit=' + $Commit + ' tree=' + $Tree)
if ($PassThru) {
    Write-Output $Identity
}
