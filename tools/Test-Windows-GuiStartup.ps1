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

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
if ($null -eq ('MechoFly.RuntimeSmoke.WindowProbe' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace MechoFly.RuntimeSmoke
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

        [DllImport("user32.dll")]
        private static extern bool SetForegroundWindow(IntPtr hwnd);

        public static bool Activate(long handle)
        {
            return SetForegroundWindow(new IntPtr(handle));
        }

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
                if (width < 32 || height < 32)
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

function Save-WindowCapture {
    param(
        [Parameter(Mandatory = $true)]
        [object] $Window,

        [Parameter(Mandatory = $true)]
        [string] $LiteralPath
    )

    [void][MechoFly.RuntimeSmoke.WindowProbe]::Activate(
        [int64]$Window.handle)
    Start-Sleep -Milliseconds 250
    $Virtual = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $Left = [Math]::Max($Virtual.Left, [int]$Window.x)
    $Top = [Math]::Max($Virtual.Top, [int]$Window.y)
    $Right = [Math]::Min(
        $Virtual.Right,
        [int]$Window.x + [int]$Window.width)
    $Bottom = [Math]::Min(
        $Virtual.Bottom,
        [int]$Window.y + [int]$Window.height)
    $Width = $Right - $Left
    $Height = $Bottom - $Top
    if ($Width -le 0 -or $Height -le 0) {
        throw ('Visible application window was outside the virtual screen: ' +
            [string]$Window.title)
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
        $Bitmap.Save(
            $LiteralPath,
            [System.Drawing.Imaging.ImageFormat]::Png)
    }
    finally {
        $Graphics.Dispose()
        $Bitmap.Dispose()
    }

    return [pscustomobject][ordered]@{
        path = $LiteralPath
        sha256 = (Get-FileHash `
            -LiteralPath $LiteralPath `
            -Algorithm SHA256).Hash
        width = $Width
        height = $Height
        source_window = $Window
    }
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

        $Windows = @([MechoFly.RuntimeSmoke.WindowProbe]::ForProcess(
            [uint32]$Process.Id))
        $LiveBrainVisible = @($Windows | Where-Object {
            [string]$_.title -like '*Live Brain*'
        }).Count -gt 0
        $BrainLabVisible = @($Windows | Where-Object {
            [string]$_.title -like '*Brain Lab*'
        }).Count -gt 0
        $DesktopPetVisible = @($Windows | Where-Object {
            [string]$_.title -eq 'MechoFly desktop pet'
        }).Count -gt 0
        if ($Arguments -contains '--brain-lab' -and
            (-not $LiveBrainVisible -or -not $BrainLabVisible)) {
            throw ('The comprehensive neural windows were not both visible. ' +
                'LiveBrain=' + [string]$LiveBrainVisible +
                '; BrainLab=' + [string]$BrainLabVisible)
        }
        if (-not $DesktopPetVisible) {
            throw 'The normal-scale layered desktop pet window was not visible.'
        }

        $Captures = New-Object System.Collections.Generic.List[object]
        $CaptureIndex = 0
        foreach ($Window in @($Windows | Sort-Object title, handle)) {
            $CaptureIndex++
            $CapturePath = Join-Path $CaseDirectory (
                'window-' + $CaptureIndex.ToString('00') + '.png')
            $Captures.Add((Save-WindowCapture `
                -Window $Window `
                -LiteralPath $CapturePath))
        }

        $Result = [ordered]@{
            schema_version = 2
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
            visible_window_count = $Windows.Count
            visible_window_titles = @($Windows | ForEach-Object {
                [string]$_.title
            })
            live_brain_visible = $LiveBrainVisible
            brain_lab_visible = $BrainLabVisible
            desktop_pet_visible = $DesktopPetVisible
            screenshots = [object[]]$Captures.ToArray()
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
        schema_version = 2
        status = 'PASS'
        cases = @($Cases)
        normal_scale_desktop_capture = $true
        comprehensive_neural_window_capture = $true
        source_mutation = $false
        live_hardware_authority = 'NONE'
    })

Write-Host ('MECHOFLY_GUI_STARTUP_SMOKE=PASS cases=' + [string]$Cases.Count)
