#requires -version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $ExecutablePath,

    [string] $OutputDirectory = (Join-Path $PSScriptRoot '..\artifacts\runtime-smoke'),

    [ValidateRange(3, 60)]
    [int] $ObservationSeconds = 8
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Executable = (Resolve-Path -LiteralPath $ExecutablePath).Path
if (-not (Test-Path -LiteralPath $OutputDirectory -PathType Container)) {
    New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
}
$OutputDirectory = (Resolve-Path -LiteralPath $OutputDirectory).Path

function Write-JsonFile {
    param(
        [Parameter(Mandatory = $true)]
        [string] $LiteralPath,

        [Parameter(Mandatory = $true)]
        [object] $Value
    )

    $Json = $Value | ConvertTo-Json -Depth 10
    [System.IO.File]::WriteAllText(
        $LiteralPath,
        $Json + [Environment]::NewLine,
        (New-Object System.Text.UTF8Encoding($false)))
}

function Invoke-GuiCase {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,

        [Parameter(Mandatory = $true)]
        [string[]] $Arguments
    )

    $CaseDirectory = Join-Path $OutputDirectory $Name
    New-Item -ItemType Directory -Path $CaseDirectory -Force | Out-Null
    $StandardOutput = Join-Path $CaseDirectory 'stdout.txt'
    $StandardError = Join-Path $CaseDirectory 'stderr.txt'
    $StartedUtc = [DateTime]::UtcNow
    $Process = Start-Process `
        -FilePath $Executable `
        -ArgumentList $Arguments `
        -WorkingDirectory (Split-Path -Parent $Executable) `
        -RedirectStandardOutput $StandardOutput `
        -RedirectStandardError $StandardError `
        -PassThru

    try {
        $Deadline = [DateTime]::UtcNow.AddSeconds($ObservationSeconds)
        while ([DateTime]::UtcNow -lt $Deadline) {
            Start-Sleep -Milliseconds 250
            $Process.Refresh()
            if ($Process.HasExited) {
                $ErrorText = ''
                if (Test-Path -LiteralPath $StandardError -PathType Leaf) {
                    $ErrorText = [System.IO.File]::ReadAllText($StandardError)
                }
                throw (
                    'MechoFly GUI case ' + $Name + ' exited during its ' +
                    [string]$ObservationSeconds + '-second startup boundary. ' +
                    'ExitCode=' + [string]$Process.ExitCode +
                    [Environment]::NewLine + $ErrorText)
            }
        }

        $Result = [ordered]@{
            schema_version = 1
            status = 'PASS'
            case = $Name
            executable = $Executable
            executable_sha256 = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash
            arguments = @($Arguments)
            started_utc = $StartedUtc.ToString('o')
            observed_until_utc = [DateTime]::UtcNow.ToString('o')
            observation_seconds = $ObservationSeconds
            process_id = $Process.Id
            survived_startup_boundary = $true
            collector_stopped_process = $true
            live_hardware_authority = 'NONE'
        }
        Write-JsonFile `
            -LiteralPath (Join-Path $CaseDirectory 'receipt.json') `
            -Value $Result
        return $Result
    }
    finally {
        $Process.Refresh()
        if (-not $Process.HasExited) {
            Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
            $Process.WaitForExit()
        }
    }
}

$PreviousBacktrace = $env:RUST_BACKTRACE
$PreviousLog = $env:RUST_LOG
try {
    $env:RUST_BACKTRACE = '1'
    $env:RUST_LOG = 'mechofly_app=debug,wgpu=warn,eframe=info'
    $Cases = @(
        Invoke-GuiCase `
            -Name 'cpu-brain-lab' `
            -Arguments @('--skin', 'drosophila', '--compute', 'cpu', '--brain-lab', '--reduced-motion')
        Invoke-GuiCase `
            -Name 'auto-brain-lab' `
            -Arguments @('--skin', 'firefly', '--compute', 'auto', '--brain-lab', '--reduced-motion')
    )
}
finally {
    $env:RUST_BACKTRACE = $PreviousBacktrace
    $env:RUST_LOG = $PreviousLog
}

Write-JsonFile `
    -LiteralPath (Join-Path $OutputDirectory 'summary.json') `
    -Value ([ordered]@{
        schema_version = 1
        status = 'PASS'
        cases = @($Cases)
        source_mutation = $false
        live_hardware_authority = 'NONE'
    })

Write-Host ('MECHOFLY_GUI_STARTUP_SMOKE=PASS cases=' + [string]$Cases.Count)
