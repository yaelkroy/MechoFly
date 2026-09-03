#requires -version 5.1
[CmdletBinding()]
param(
    [string] $RepositoryRoot = 'D:\Projects\MechoFly',

    [ValidateRange(3, 30)]
    [int] $ObservationSeconds = 6
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Downloads = Join-Path $env:USERPROFILE 'Downloads'
if (-not (Test-Path -LiteralPath $Downloads -PathType Container)) {
    throw ('Downloads directory was not found: ' + $Downloads)
}
$StartedUtc = [DateTime]::UtcNow
$Stamp = $StartedUtc.ToString('yyyyMMddTHHmmssZ')
$Staging = Join-Path ([System.IO.Path]::GetTempPath()) (
    'MechoFly-AI100-Evidence-' + [Guid]::NewGuid().ToString('N'))
$ZipPath = Join-Path $Downloads (
    'UPLOAD_MechoFly_AI100_ExactSource_Design_' + $Stamp + '.zip')
$Failure = $null
$Identity = $null
$CaseResults = New-Object System.Collections.Generic.List[object]

function Write-JsonFile {
    param(
        [Parameter(Mandatory = $true)]
        [string] $LiteralPath,

        [Parameter(Mandatory = $true)]
        [object] $Value
    )

    $Json = $Value | ConvertTo-Json -Depth 12
    [System.IO.File]::WriteAllText(
        $LiteralPath,
        $Json + [Environment]::NewLine,
        (New-Object System.Text.UTF8Encoding($false)))
}

function Get-MechoFlyWindows {
    param(
        [Parameter(Mandatory = $true)]
        [int] $ProcessId
    )

    return @([MechoFly.Evidence.WindowProbe]::ForProcess(
        [uint32]$ProcessId))
}

function Save-WindowCapture {
    param(
        [Parameter(Mandatory = $true)]
        [object] $Window,

        [Parameter(Mandatory = $true)]
        [string] $LiteralPath,

        [ValidateRange(0, 64)]
        [int] $Padding = 16
    )

    $Virtual = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $Left = [Math]::Max($Virtual.Left, [int]$Window.x - $Padding)
    $Top = [Math]::Max($Virtual.Top, [int]$Window.y - $Padding)
    $Right = [Math]::Min(
        $Virtual.Right,
        [int]$Window.x + [int]$Window.width + $Padding)
    $Bottom = [Math]::Min(
        $Virtual.Bottom,
        [int]$Window.y + [int]$Window.height + $Padding)
    $Width = $Right - $Left
    $Height = $Bottom - $Top
    if ($Width -le 0 -or $Height -le 0) {
        throw 'Resolved application-window capture rectangle was empty.'
    }

    $Bitmap = New-Object System.Drawing.Bitmap($Width, $Height)
    $Graphics = [System.Drawing.Graphics]::FromImage($Bitmap)
    try {
        $Graphics.CopyFromScreen(
            $Left,
            $Top,
            0,
            0,
            (New-Object System.Drawing.Size($Width, $Height)),
            [System.Drawing.CopyPixelOperation]::SourceCopy)
        $Bitmap.Save($LiteralPath, [System.Drawing.Imaging.ImageFormat]::Png)
    }
    finally {
        $Graphics.Dispose()
        $Bitmap.Dispose()
    }

    return [pscustomobject][ordered]@{
        path = $LiteralPath
        sha256 = (Get-FileHash -LiteralPath $LiteralPath -Algorithm SHA256).Hash
        capture_x = $Left
        capture_y = $Top
        width = $Width
        height = $Height
        source_window = $Window
    }
}

function Invoke-DesignCase {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,

        [Parameter(Mandatory = $true)]
        [string[]] $Arguments,

        [ValidateSet('smallest', 'largest')]
        [string] $WindowSelection
    )

    $CaseDirectory = Join-Path $Staging $Name
    New-Item -ItemType Directory -Path $CaseDirectory -Force | Out-Null
    $StandardOutput = Join-Path $CaseDirectory 'stdout.txt'
    $StandardError = Join-Path $CaseDirectory 'stderr.txt'
    $CaseStartedUtc = [DateTime]::UtcNow
    $Process = Start-Process `
        -FilePath $script:Executable `
        -WorkingDirectory $RepositoryRoot `
        -ArgumentList $Arguments `
        -RedirectStandardOutput $StandardOutput `
        -RedirectStandardError $StandardError `
        -PassThru

    try {
        $Windows = @()
        $Deadline = [DateTime]::UtcNow.AddSeconds($ObservationSeconds)
        while ([DateTime]::UtcNow -lt $Deadline) {
            Start-Sleep -Milliseconds 250
            $Process.Refresh()
            if ($Process.HasExited) {
                $ErrorText = ''
                if (Test-Path -LiteralPath $StandardError -PathType Leaf) {
                    $ErrorText = [System.IO.File]::ReadAllText($StandardError)
                }
                throw ('MechoFly case ' + $Name + ' exited before capture. ' +
                    'ExitCode=' + [string]$Process.ExitCode +
                    [Environment]::NewLine + $ErrorText)
            }
            $Windows = @(Get-MechoFlyWindows -ProcessId $Process.Id |
                Where-Object { $_.width -ge 32 -and $_.height -ge 32 })
            if ($Windows.Count -gt 0) {
                break
            }
        }
        if ($Windows.Count -eq 0) {
            throw ('MechoFly case ' + $Name +
                ' created no visible application window before timeout.')
        }

        # Allow one extra render interval after the native window becomes
        # visible, then enumerate again so the screenshot is not a first-frame
        # placeholder.
        Start-Sleep -Milliseconds 1200
        $Process.Refresh()
        if ($Process.HasExited) {
            throw ('MechoFly case ' + $Name + ' exited before its design frame.')
        }
        $Windows = @(Get-MechoFlyWindows -ProcessId $Process.Id |
            Where-Object { $_.width -ge 32 -and $_.height -ge 32 })
        Write-JsonFile `
            -LiteralPath (Join-Path $CaseDirectory 'windows.json') `
            -Value @($Windows)

        $SortedWindows = @($Windows | Sort-Object {
            [int64]$_.width * [int64]$_.height
        })
        if ($WindowSelection -eq 'largest') {
            $SelectedWindow = $SortedWindows[$SortedWindows.Count - 1]
        }
        else {
            $SelectedWindow = $SortedWindows[0]
        }
        $ScreenshotPath = Join-Path $CaseDirectory ($Name + '.png')
        $Screenshot = Save-WindowCapture `
            -Window $SelectedWindow `
            -LiteralPath $ScreenshotPath

        $ObservedUntil = [DateTime]::UtcNow.AddSeconds(2)
        while ([DateTime]::UtcNow -lt $ObservedUntil) {
            Start-Sleep -Milliseconds 250
            $Process.Refresh()
            if ($Process.HasExited) {
                throw ('MechoFly case ' + $Name +
                    ' exited during its post-capture survival check.')
            }
        }

        $Result = [pscustomobject][ordered]@{
            status = 'PASS'
            case = $Name
            arguments = @($Arguments)
            process_id = $Process.Id
            started_utc = $CaseStartedUtc.ToString('o')
            observed_until_utc = [DateTime]::UtcNow.ToString('o')
            survived_capture_boundary = $true
            visible_window_count = $Windows.Count
            selected_window_policy = $WindowSelection
            screenshot = $Screenshot
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

New-Item -ItemType Directory -Path $Staging -Force | Out-Null

try {
    $ExistingProcesses = @(Get-CimInstance Win32_Process `
        -Filter "Name='MechoFly.exe'" `
        -ErrorAction SilentlyContinue)
    if ($ExistingProcesses.Count -gt 0) {
        throw ('Quit the existing MechoFly instance before capture. ' +
            'The collector will never stop a process it did not start.')
    }

    $Identity = & (Join-Path $RepositoryRoot `
        'tools\Assert-AI100-SourceIdentity.ps1') `
        -RepositoryRoot $RepositoryRoot `
        -RefreshRemote `
        -PassThru
    $ShortCommit = ([string]$Identity.source_commit).Substring(0, 12)
    $ZipPath = Join-Path $Downloads (
        'UPLOAD_MechoFly_AI100_ExactSource_Design_' + $Stamp + '_' +
        $ShortCommit + '.zip')
    Write-JsonFile `
        -LiteralPath (Join-Path $Staging 'exact-source-identity.json') `
        -Value $Identity

    $script:Executable = [string]$Identity.executable
    foreach ($EvidencePath in @(
        [string]$Identity.profile,
        [string]$Identity.receipt,
        (Join-Path $RepositoryRoot 'artifacts\ai100-self-test.json')
    )) {
        if (Test-Path -LiteralPath $EvidencePath -PathType Leaf) {
            Copy-Item -LiteralPath $EvidencePath -Destination $Staging
        }
    }

    $Machine = [ordered]@{
        schema_version = 1
        collected_utc = [DateTime]::UtcNow.ToString('o')
        os = @(Get-CimInstance Win32_OperatingSystem | Select-Object `
            Caption, Version, BuildNumber, OSArchitecture,
            TotalVisibleMemorySize)
        cpu = @(Get-CimInstance Win32_Processor | Select-Object `
            Name, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed)
        gpu = @(Get-CimInstance Win32_VideoController | Select-Object `
            Name, DriverVersion, AdapterRAM, CurrentHorizontalResolution,
            CurrentVerticalResolution)
        powershell = $PSVersionTable.PSVersion.ToString()
        live_hardware_authority = 'NONE'
    }
    Write-JsonFile `
        -LiteralPath (Join-Path $Staging 'machine-capacity-context.json') `
        -Value $Machine

    Add-Type -AssemblyName System.Drawing
    Add-Type -AssemblyName System.Windows.Forms
    if ($null -eq ('MechoFly.Evidence.WindowProbe' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace MechoFly.Evidence
{
    public sealed class WindowInfo
    {
        public long handle { get; set; }
        public int x { get; set; }
        public int y { get; set; }
        public int width { get; set; }
        public int height { get; set; }
        public string title { get; set; }
        public string class_name { get; set; }
    }

    public static class WindowProbe
    {
        private delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr parameter);

        [StructLayout(LayoutKind.Sequential)]
        private struct Rect
        {
            public int Left;
            public int Top;
            public int Right;
            public int Bottom;
        }

        [DllImport("user32.dll")]
        private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);

        [DllImport("user32.dll")]
        private static extern bool IsWindowVisible(IntPtr hwnd);

        [DllImport("user32.dll")]
        private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);

        [DllImport("user32.dll")]
        private static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int count);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetClassName(IntPtr hwnd, StringBuilder text, int count);

        public static WindowInfo[] ForProcess(uint expectedProcessId)
        {
            List<WindowInfo> result = new List<WindowInfo>();
            EnumWindows(delegate(IntPtr hwnd, IntPtr parameter)
            {
                uint processId;
                GetWindowThreadProcessId(hwnd, out processId);
                if (processId != expectedProcessId || !IsWindowVisible(hwnd))
                {
                    return true;
                }

                Rect rect;
                if (!GetWindowRect(hwnd, out rect))
                {
                    return true;
                }
                int width = rect.Right - rect.Left;
                int height = rect.Bottom - rect.Top;
                if (width <= 0 || height <= 0)
                {
                    return true;
                }

                StringBuilder title = new StringBuilder(512);
                StringBuilder className = new StringBuilder(256);
                GetWindowText(hwnd, title, title.Capacity);
                GetClassName(hwnd, className, className.Capacity);
                result.Add(new WindowInfo
                {
                    handle = hwnd.ToInt64(),
                    x = rect.Left,
                    y = rect.Top,
                    width = width,
                    height = height,
                    title = title.ToString(),
                    class_name = className.ToString()
                });
                return true;
            }, IntPtr.Zero);
            return result.ToArray();
        }
    }
}
'@
    }

    $PreviousBacktrace = $env:RUST_BACKTRACE
    $PreviousLog = $env:RUST_LOG
    try {
        $env:RUST_BACKTRACE = '1'
        $env:RUST_LOG = 'mechofly_app=debug,wgpu=warn,eframe=info'
        $CaseResults.Add((Invoke-DesignCase `
            -Name 'firefly-transparent-desktop-pet' `
            -Arguments @('--skin', 'firefly', '--compute', 'auto') `
            -WindowSelection 'smallest'))
        $CaseResults.Add((Invoke-DesignCase `
            -Name 'firefly-auto-neural-observatory' `
            -Arguments @('--skin', 'firefly', '--compute', 'auto', '--brain-lab') `
            -WindowSelection 'largest'))
        $CaseResults.Add((Invoke-DesignCase `
            -Name 'drosophila-cpu-neural-observatory' `
            -Arguments @('--skin', 'drosophila', '--compute', 'cpu', '--brain-lab') `
            -WindowSelection 'largest'))
    }
    finally {
        $env:RUST_BACKTRACE = $PreviousBacktrace
        $env:RUST_LOG = $PreviousLog
    }

    $Events = @()
    try {
        $Events = @(Get-WinEvent `
            -FilterHashtable @{
                LogName = 'Application'
                StartTime = $StartedUtc
            } `
            -MaxEvents 500 `
            -ErrorAction Stop |
            Where-Object { $_.Message -match 'MechoFly|MechoFly\.exe' } |
            Select-Object TimeCreated, ProviderName, Id, LevelDisplayName, Message)
    }
    catch {
        $Events = @([pscustomobject]@{
            collection_error = $_.Exception.Message
        })
    }
    Write-JsonFile `
        -LiteralPath (Join-Path $Staging 'application-events.json') `
        -Value @($Events)

    $Manifest = [ordered]@{
        schema_version = 1
        status = 'PASS'
        purpose = 'exact-source runtime diagnostics and visual iteration review'
        source_identity = $Identity
        # Windows PowerShell 5.1 can throw "Argument types do not match" when
        # its array-subexpression binder enumerates List[object] directly.
        # ToArray() preserves the same values without invoking that binder.
        cases = [object[]]$CaseResults.ToArray()
        screenshot_scope = 'MechoFly application windows with 16-pixel context only'
        full_desktop_captured = $false
        source_mutation = $false
        live_hardware_authority = 'NONE'
        started_utc = $StartedUtc.ToString('o')
        completed_utc = [DateTime]::UtcNow.ToString('o')
    }
    Write-JsonFile `
        -LiteralPath (Join-Path $Staging 'manifest.json') `
        -Value $Manifest
}
catch {
    $Failure = $_
    $FailureRecord = [ordered]@{
        schema_version = 1
        status = 'FAIL'
        message = $_.Exception.Message
        details = ($_ | Out-String)
        source_identity = $Identity
        completed_cases = [object[]]$CaseResults.ToArray()
        source_mutation = $false
        live_hardware_authority = 'NONE'
        started_utc = $StartedUtc.ToString('o')
        failed_utc = [DateTime]::UtcNow.ToString('o')
    }
    Write-JsonFile `
        -LiteralPath (Join-Path $Staging 'failure.json') `
        -Value $FailureRecord
}
finally {
    try {
        Compress-Archive `
            -Path (Join-Path $Staging '*') `
            -DestinationPath $ZipPath `
            -CompressionLevel Optimal `
            -Force
    }
    finally {
        if (Test-Path -LiteralPath $Staging -PathType Container) {
            Remove-Item -LiteralPath $Staging -Recurse -Force
        }
    }
}

Write-Host ('UPLOAD_THIS_ZIP=' + $ZipPath)
if ($null -ne $Failure) {
    throw ('Evidence collection failed, but partial evidence was preserved in ' +
        $ZipPath + '. ' + $Failure.Exception.Message)
}
Write-Host 'MECHOFLY_AI100_EVIDENCE=PASS'
