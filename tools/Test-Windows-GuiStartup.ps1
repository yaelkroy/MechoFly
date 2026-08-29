#requires -version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $ExecutablePath,

    [string] $OutputDirectory = (Join-Path $PSScriptRoot '..\artifacts\runtime-smoke'),

    [ValidateRange(3, 60)]
    [int] $ObservationSeconds = 12
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
$WindowProbeReferences = @(
    [System.Drawing.Bitmap].Assembly.Location
    [System.Windows.Forms.Application].Assembly.Location
)
if ($null -eq ('MechoFly.RuntimeSmoke.WindowProbe' -as [type])) {
    Add-Type `
        -ReferencedAssemblies $WindowProbeReferences `
        -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Drawing;
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

        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool SetWindowPos(
            IntPtr hwnd,
            IntPtr insertAfter,
            int x,
            int y,
            int width,
            int height,
            uint flags);

        [DllImport("dwmapi.dll")]
        private static extern int DwmFlush();

        [DllImport("user32.dll", EntryPoint = "PostMessageW", SetLastError = true)]
        private static extern bool PostMessage(
            IntPtr hwnd,
            uint message,
            UIntPtr wParam,
            IntPtr lParam);

        [DllImport("user32.dll")]
        private static extern bool PrintWindow(
            IntPtr hwnd,
            IntPtr targetDeviceContext,
            uint flags);

        public static bool Activate(long handle)
        {
            return SetForegroundWindow(new IntPtr(handle));
        }

        public static bool SetCaptureLayer(long handle, bool topmost)
        {
            const uint NoSize = 0x0001;
            const uint NoMove = 0x0002;
            const uint NoActivate = 0x0010;
            const uint ShowWindow = 0x0040;
            IntPtr insertAfter = topmost ? new IntPtr(-1) : new IntPtr(1);
            return SetWindowPos(
                new IntPtr(handle),
                insertAfter,
                0,
                0,
                0,
                0,
                NoSize | NoMove | NoActivate | ShowWindow);
        }

        public static bool FlushComposition()
        {
            return DwmFlush() == 0;
        }

        public static bool PostHotkey(long handle, int hotkeyId)
        {
            const uint HotkeyMessage = 0x0312;
            return PostMessage(
                new IntPtr(handle),
                HotkeyMessage,
                new UIntPtr((uint)hotkeyId),
                IntPtr.Zero);
        }

        public static bool PostEvidenceHold(long handle)
        {
            const uint EvidenceHoldMessage = 0x804D;
            return PostMessage(
                new IntPtr(handle),
                EvidenceHoldMessage,
                new UIntPtr(1),
                IntPtr.Zero);
        }

        public static bool Print(long handle, IntPtr targetDeviceContext)
        {
            const uint RenderFullContent = 2;
            return PrintWindow(
                new IntPtr(handle),
                targetDeviceContext,
                RenderFullContent);
        }

        public static WindowInfo Current(long handle)
        {
            IntPtr hwnd = new IntPtr(handle);
            if (!IsWindowVisible(hwnd))
            {
                return null;
            }
            Rect rect;
            if (!GetWindowRect(hwnd, out rect))
            {
                return null;
            }
            int width = rect.Right - rect.Left;
            int height = rect.Bottom - rect.Top;
            if (width < 32 || height < 32)
            {
                return null;
            }
            StringBuilder title = new StringBuilder(512);
            StringBuilder className = new StringBuilder(256);
            GetWindowText(hwnd, title, title.Capacity);
            GetClassName(hwnd, className, className.Capacity);
            return new WindowInfo
            {
                handle = handle,
                x = rect.Left,
                y = rect.Top,
                width = width,
                height = height,
                title = title.ToString(),
                class_name = className.ToString()
            };
        }

        public static int CountDifferentPixels(
            string firstPath,
            string secondPath,
            int threshold)
        {
            using (Bitmap first = new Bitmap(firstPath))
            using (Bitmap second = new Bitmap(secondPath))
            {
                if (first.Width != second.Width || first.Height != second.Height)
                {
                    return -1;
                }
                int different = 0;
                for (int y = 0; y < first.Height; y++)
                {
                    for (int x = 0; x < first.Width; x++)
                    {
                        Color left = first.GetPixel(x, y);
                        Color right = second.GetPixel(x, y);
                        int delta = Math.Abs(left.R - right.R)
                            + Math.Abs(left.G - right.G)
                            + Math.Abs(left.B - right.B);
                        if (delta > threshold)
                        {
                            different++;
                        }
                    }
                }
                return different;
            }
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

function Save-DesktopRectangle {
    param(
        [Parameter(Mandatory = $true)]
        [int] $Left,

        [Parameter(Mandatory = $true)]
        [int] $Top,

        [Parameter(Mandatory = $true)]
        [int] $Width,

        [Parameter(Mandatory = $true)]
        [int] $Height,

        [Parameter(Mandatory = $true)]
        [string] $LiteralPath
    )

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
}

function Save-WindowCapture {
    param(
        [Parameter(Mandatory = $true)]
        [object] $Window,

        [Parameter(Mandatory = $true)]
        [string] $LiteralPath
    )

    $Window = [MechoFly.RuntimeSmoke.WindowProbe]::Current(
        [int64]$Window.handle)
    if ($null -eq $Window) {
        throw 'The evidence window disappeared before capture.'
    }

    $CaptureMethod = 'desktop-composition-current-rect'
    $CaptureWidth = 0
    $CaptureHeight = 0
    $ForegroundDifferencePixels = 0
    if ([string]$Window.class_name -eq 'Window Class') {
        $CaptureMethod = 'print-window-full-content'
        $CaptureWidth = [int]$Window.width
        $CaptureHeight = [int]$Window.height
        $Bitmap = New-Object System.Drawing.Bitmap(
            $CaptureWidth,
            $CaptureHeight)
        $Graphics = [System.Drawing.Graphics]::FromImage($Bitmap)
        try {
            $DeviceContext = $Graphics.GetHdc()
            try {
                $Printed = [MechoFly.RuntimeSmoke.WindowProbe]::Print(
                    [int64]$Window.handle,
                    $DeviceContext)
            }
            finally {
                $Graphics.ReleaseHdc($DeviceContext)
            }
            if (-not $Printed) {
                throw ('Full-content window capture failed: ' +
                    [string]$Window.title)
            }
            $Bitmap.Save(
                $LiteralPath,
                [System.Drawing.Imaging.ImageFormat]::Png)
        }
        finally {
            $Graphics.Dispose()
            $Bitmap.Dispose()
        }
    }
    else {
        $CaptureMethod = 'desktop-composition-isolated-foreground'
        $Virtual = [System.Windows.Forms.SystemInformation]::VirtualScreen
        $BackgroundPath = $LiteralPath + '.background.png'
        try {
            if (-not [MechoFly.RuntimeSmoke.WindowProbe]::SetCaptureLayer(
                [int64]$Window.handle,
                $false)) {
                throw 'Could not demote the layered pet for background isolation.'
            }
            [void][MechoFly.RuntimeSmoke.WindowProbe]::FlushComposition()
            Start-Sleep -Milliseconds 80

            $Left = [Math]::Max($Virtual.Left, [int]$Window.x)
            $Top = [Math]::Max($Virtual.Top, [int]$Window.y)
            $Right = [Math]::Min(
                $Virtual.Right,
                [int]$Window.x + [int]$Window.width)
            $Bottom = [Math]::Min(
                $Virtual.Bottom,
                [int]$Window.y + [int]$Window.height)
            $CaptureWidth = $Right - $Left
            $CaptureHeight = $Bottom - $Top
            if ($CaptureWidth -le 0 -or $CaptureHeight -le 0) {
                throw 'The layered pet was outside the virtual screen.'
            }
            Save-DesktopRectangle `
                -Left $Left `
                -Top $Top `
                -Width $CaptureWidth `
                -Height $CaptureHeight `
                -LiteralPath $BackgroundPath

            if (-not [MechoFly.RuntimeSmoke.WindowProbe]::SetCaptureLayer(
                [int64]$Window.handle,
                $true)) {
                throw 'Could not promote the layered pet for foreground capture.'
            }
            [void][MechoFly.RuntimeSmoke.WindowProbe]::FlushComposition()
            Start-Sleep -Milliseconds 80
            $Window = [MechoFly.RuntimeSmoke.WindowProbe]::Current(
                [int64]$Window.handle)
            if ($null -eq $Window) {
                throw 'The layered pet disappeared after foreground promotion.'
            }

            $Left = [Math]::Max($Virtual.Left, [int]$Window.x)
            $Top = [Math]::Max($Virtual.Top, [int]$Window.y)
            $Right = [Math]::Min(
                $Virtual.Right,
                [int]$Window.x + [int]$Window.width)
            $Bottom = [Math]::Min(
                $Virtual.Bottom,
                [int]$Window.y + [int]$Window.height)
            $CaptureWidth = $Right - $Left
            $CaptureHeight = $Bottom - $Top
            Save-DesktopRectangle `
                -Left $Left `
                -Top $Top `
                -Width $CaptureWidth `
                -Height $CaptureHeight `
                -LiteralPath $LiteralPath
            $ForegroundDifferencePixels =
                [MechoFly.RuntimeSmoke.WindowProbe]::CountDifferentPixels(
                    $BackgroundPath,
                    $LiteralPath,
                    18)
            if ($ForegroundDifferencePixels -lt 500) {
                throw (
                    'The layered-pet capture did not contain a visible ' +
                    'foreground. differing_pixels=' +
                    [string]$ForegroundDifferencePixels)
            }
        }
        finally {
            Remove-Item `
                -LiteralPath $BackgroundPath `
                -Force `
                -ErrorAction SilentlyContinue
            [void][MechoFly.RuntimeSmoke.WindowProbe]::SetCaptureLayer(
                [int64]$Window.handle,
                $true)
            [void][MechoFly.RuntimeSmoke.WindowProbe]::FlushComposition()
        }
    }

    return [pscustomobject][ordered]@{
        path = $LiteralPath
        sha256 = (Get-FileHash `
            -LiteralPath $LiteralPath `
            -Algorithm SHA256).Hash
        width = $CaptureWidth
        height = $CaptureHeight
        capture_method = $CaptureMethod
        foreground_difference_pixels = $ForegroundDifferencePixels
        full_window = (
            $CaptureWidth -eq [int]$Window.width -and
            $CaptureHeight -eq [int]$Window.height)
        source_window = $Window
    }
}

function Invoke-GuiCase {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,

        [Parameter(Mandatory = $true)]
        [string[]] $Arguments,

        [Parameter(Mandatory = $true)]
        [string] $ExpectedSkinLabel,

        [int] $StimulusHotkeyId = 0,

        [ValidateRange(0, 5000)]
        [int] $StimulusSettleMilliseconds = 0
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

        $StimulusPosted = $false
        if ($StimulusHotkeyId -ne 0) {
            $PetWindow = @($Windows | Where-Object {
                [string]$_.title -eq 'MechoFly desktop pet'
            }) | Select-Object -First 1
            $StimulusPosted = [MechoFly.RuntimeSmoke.WindowProbe]::PostHotkey(
                [int64]$PetWindow.handle,
                $StimulusHotkeyId)
            if (-not $StimulusPosted) {
                throw ('Could not post bounded behavior stimulus 0x' +
                    $StimulusHotkeyId.ToString('X4') + '.')
            }
            if ($StimulusSettleMilliseconds -gt 0) {
                Start-Sleep -Milliseconds $StimulusSettleMilliseconds
            }
            $Process.Refresh()
            if ($Process.HasExited) {
                throw ('MechoFly exited after bounded behavior stimulus 0x' +
                    $StimulusHotkeyId.ToString('X4') + '.')
            }
            $Windows = @([MechoFly.RuntimeSmoke.WindowProbe]::ForProcess(
                [uint32]$Process.Id))
        }

        $PetWindow = @($Windows | Where-Object {
            [string]$_.title -eq 'MechoFly desktop pet'
        }) | Select-Object -First 1
        $EvidenceHoldPosted =
            [MechoFly.RuntimeSmoke.WindowProbe]::PostEvidenceHold(
                [int64]$PetWindow.handle)
        if (-not $EvidenceHoldPosted) {
            throw 'Could not place the model and presentation in evidence hold.'
        }
        $EvidenceSettleTimeoutMilliseconds = 5000
$EvidencePollMilliseconds = 50
$EvidencePollCount = 0
$EvidenceSettleStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
$Windows = @()
$NeuralWindows = @()
$NeuralWindowTitles = @()
$ExpectedSkinTitlePassed = $false
$NeuralFrames = @()
$NeuralFramesSynchronized = $false
do {
    $EvidencePollCount++
    $Process.Refresh()
    if ($Process.HasExited) {
        throw 'MechoFly exited while entering synchronized evidence hold.'
    }
    $Windows = @([MechoFly.RuntimeSmoke.WindowProbe]::ForProcess(
        [uint32]$Process.Id))
    $NeuralWindows = @($Windows | Where-Object {
        [string]$_.title -like '*Live Brain*' -or
        [string]$_.title -like '*Brain Lab*'
    })
    $NeuralWindowTitles = @($NeuralWindows | ForEach-Object {
        [string]$_.title
    })
    $ExpectedSkinTitlePassed = (
        $NeuralWindows.Count -eq 2 -and
        @($NeuralWindowTitles | Where-Object {
  -not $_.StartsWith(
      $ExpectedSkinLabel + ' ',
      [System.StringComparison]::Ordinal)
        }).Count -eq 0)

    $NeuralFrames = @()
    $AllNeuralTitlesFrozen = $NeuralWindows.Count -eq 2
    if ($AllNeuralTitlesFrozen) {
        foreach ($NeuralWindowTitle in $NeuralWindowTitles) {
  $Match = [regex]::Match(
      $NeuralWindowTitle,
      'frame (?<frame>[0-9]{8})$')
  if (-not $Match.Success) {
      $AllNeuralTitlesFrozen = $false
      break
  }
  $NeuralFrames += [uint64]$Match.Groups['frame'].Value
        }
    }
    $UniqueNeuralFrames = @($NeuralFrames | Sort-Object -Unique)
    $NeuralFramesSynchronized = (
        $AllNeuralTitlesFrozen -and
        $UniqueNeuralFrames.Count -eq 1)
    if ($ExpectedSkinTitlePassed -and $NeuralFramesSynchronized) {
        break
    }
    Start-Sleep -Milliseconds $EvidencePollMilliseconds
} while (
    $EvidenceSettleStopwatch.ElapsedMilliseconds -lt
        $EvidenceSettleTimeoutMilliseconds)
$EvidenceSettleStopwatch.Stop()
$EvidenceSettleMilliseconds =
    [int]$EvidenceSettleStopwatch.ElapsedMilliseconds

if ($NeuralWindows.Count -ne 2) {
    throw ('Expected exactly two neural evidence windows; found ' +
        [string]$NeuralWindows.Count + '.')
}
if (-not $ExpectedSkinTitlePassed) {
    throw ('A neural window title does not match skin ' +
        $ExpectedSkinLabel + '; actual=' +
        ($NeuralWindowTitles -join ' | '))
}
if (-not $AllNeuralTitlesFrozen) {
    throw ('Neural evidence titles did not publish a frozen frame ' +
        'within ' + [string]$EvidenceSettleTimeoutMilliseconds +
        ' ms; actual=' + ($NeuralWindowTitles -join ' | '))
}
if (-not $NeuralFramesSynchronized) {
    throw ('Neural evidence windows are not frame-synchronized: ' +
        ($NeuralFrames -join ', '))
}
$NeuralCaptureFrame = [uint64]$UniqueNeuralFrames[0]

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

        $PetCaptures = @($Captures | Where-Object {
            [string]$_.source_window.class_name -eq
                'MechoFlyDesktopPetLayeredWindowV1'
        })
        if ($PetCaptures.Count -ne 1 -or
            [int]$PetCaptures[0].foreground_difference_pixels -lt 500) {
            throw 'The normal-scale pet foreground was not captured exactly once.'
        }

        $Result = [ordered]@{
            schema_version = 3
            status = 'PASS'
            case = $Name
            executable = $Executable
            executable_sha256 = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash
            arguments = @($Arguments)
            started_utc = $StartedUtc.ToString('o')
            observed_until_utc = [DateTime]::UtcNow.ToString('o')
            observation_seconds = $ObservationSeconds
            stimulus_hotkey_id = $StimulusHotkeyId
            stimulus_hotkey_posted = $StimulusPosted
            stimulus_settle_milliseconds = $StimulusSettleMilliseconds
            evidence_hold_message = '0x804D'
            evidence_hold_posted = $EvidenceHoldPosted
            evidence_settle_milliseconds = $EvidenceSettleMilliseconds
evidence_settle_timeout_milliseconds = $EvidenceSettleTimeoutMilliseconds
evidence_poll_milliseconds = $EvidencePollMilliseconds
evidence_poll_count = $EvidencePollCount
            expected_skin_label = $ExpectedSkinLabel
            expected_skin_title_passed = $ExpectedSkinTitlePassed
            neural_capture_frame = $NeuralCaptureFrame
            neural_capture_frame_synchronized = $true
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
            -Arguments @('--skin', 'drosophila', '--compute', 'cpu', '--brain-lab', '--reduced-motion') `
            -ExpectedSkinLabel 'Drosophila Natural' `
            -StimulusHotkeyId 0x4D05 `
            -StimulusSettleMilliseconds 500
        Invoke-GuiCase `
            -Name 'auto-brain-lab' `
            -Arguments @('--skin', 'firefly', '--compute', 'auto', '--brain-lab') `
            -ExpectedSkinLabel 'MechoFly Prism' `
            -StimulusHotkeyId 0x4D03 `
            -StimulusSettleMilliseconds 1000
    )
}
finally {
    $env:RUST_BACKTRACE = $PreviousBacktrace
    $env:RUST_LOG = $PreviousLog
}

$AllCaptures = @($Cases | ForEach-Object {
    @($_.screenshots)
})
$FullWindowCaptures = @($AllCaptures | Where-Object {
    -not [bool]$_.full_window
}).Count -eq 0
if (-not $FullWindowCaptures) {
    throw 'At least one GUI evidence image was clipped.'
}
$PetForegroundCaptured = @($AllCaptures | Where-Object {
    [string]$_.source_window.class_name -eq
        'MechoFlyDesktopPetLayeredWindowV1' -and
    [int]$_.foreground_difference_pixels -ge 500
}).Count -eq $Cases.Count
if (-not $PetForegroundCaptured) {
    throw 'At least one case lacks an isolated visible pet foreground.'
}
$NeuralFramesSynchronized = @($Cases | Where-Object {
    -not [bool]$_.neural_capture_frame_synchronized
}).Count -eq 0
$SkinTitlesMatched = @($Cases | Where-Object {
    -not [bool]$_.expected_skin_title_passed
}).Count -eq 0

Write-JsonFile `
    -LiteralPath (Join-Path $OutputDirectory 'summary.json') `
    -Value ([ordered]@{
        schema_version = 3
        status = 'PASS'
        cases = @($Cases)
        normal_scale_desktop_capture = $true
        pet_foreground_captured = $PetForegroundCaptured
        comprehensive_neural_window_capture = $true
        neural_frames_synchronized = $NeuralFramesSynchronized
        skin_titles_matched = $SkinTitlesMatched
        full_window_captures = $FullWindowCaptures
        source_mutation = $false
        live_hardware_authority = 'NONE'
    })

Write-Host ('MECHOFLY_GUI_STARTUP_SMOKE=PASS cases=' + [string]$Cases.Count)
