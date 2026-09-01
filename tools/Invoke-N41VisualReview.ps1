#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $ExecutablePath,

    [Parameter(Mandatory = $true)]
    [string] $OutputDirectory,

    [Parameter(Mandatory = $true)]
    [string] $SourceBranch,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-f]{40}$')]
    [string] $SourceCommit,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-f]{40}$')]
    [string] $SourceTree,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-f]{64}$')]
    [string] $ExecutableSha256
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
$script:Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$script:BackdropColor = [System.Drawing.Color]::FromArgb(28, 24, 36)
$ExpectedParameterSha256 =
    'bec74a4ab771b61923ac81d71fd532d88001abd4bf00e90bf799e6e30703c138'
$ExpectedPetTitle = 'MechoFly N4.1-B visual review pet'
$EarlyBoundarySeconds = 30
$LateBoundarySeconds = 300
$LateMidpointSeconds = 450
$FinalBoundarySeconds = 600

function Assert-Condition {
    param(
        [Parameter(Mandatory = $true)]
        [bool] $Condition,

        [Parameter(Mandatory = $true)]
        [string] $Message
    )
    if (-not $Condition) {
        throw $Message
    }
}

function Write-JsonFile {
    param(
        [Parameter(Mandatory = $true)]
        [string] $LiteralPath,

        [Parameter(Mandatory = $true)]
        [object] $Value
    )
    $json = $Value | ConvertTo-Json -Depth 20
    [System.IO.File]::WriteAllText(
        $LiteralPath,
        $json + [Environment]::NewLine,
        $script:Utf8NoBom)
}

function Test-ProcessAlive {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process] $Process,

        [Parameter(Mandatory = $true)]
        [string] $StandardErrorPath
    )
    $Process.Refresh()
    if ($Process.HasExited) {
        $stderr = ''
        if (Test-Path -LiteralPath $StandardErrorPath -PathType Leaf) {
            $stderr = [System.IO.File]::ReadAllText($StandardErrorPath)
        }
        throw ('N4.1-B visual-review process exited unexpectedly. ExitCode=' +
            [string]$Process.ExitCode + [Environment]::NewLine + $stderr)
    }
}

function Wait-ReviewBoundary {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Stopwatch] $Stopwatch,

        [Parameter(Mandatory = $true)]
        [int] $Seconds,

        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process] $Process,

        [Parameter(Mandatory = $true)]
        [string] $StandardErrorPath,

        [Parameter(Mandatory = $true)]
        [string] $Label
    )
    $lastNotice = -1
    while ($Stopwatch.Elapsed.TotalSeconds -lt $Seconds) {
        Test-ProcessAlive -Process $Process -StandardErrorPath $StandardErrorPath
        $remaining = [Math]::Ceiling(
            $Seconds - $Stopwatch.Elapsed.TotalSeconds)
        if ($remaining -ne $lastNotice -and
            ($remaining -le 10 -or ($remaining % 30) -eq 0)) {
            Write-Host ($Label + ': ' + [string]$remaining + ' second(s) remaining.')
            $lastNotice = $remaining
        }
        Start-Sleep -Milliseconds 200
    }
}

function Get-ReviewPetWindow {
    param(
        [Parameter(Mandatory = $true)]
        [int] $ProcessId
    )
    $windows = @([MechoFly.N41VisualReview.WindowProbe]::ForProcess(
        [uint32]$ProcessId))
    $pets = @($windows | Where-Object {
        [string]$_.class_name -eq 'MechoFlyDesktopPetLayeredWindowV1'
    })
    Assert-Condition ($pets.Count -eq 1) (
        'Expected exactly one layered N4.1-B review pet window; found ' +
        [string]$pets.Count + '.')
    Assert-Condition ([string]$pets[0].title -ceq $ExpectedPetTitle) (
        'Review pet title mismatch: ' + [string]$pets[0].title)
    return $pets[0]
}

function Get-NonBackdropPixelCount {
    param(
        [Parameter(Mandatory = $true)]
        [string] $LiteralPath
    )
    $bitmap = New-Object System.Drawing.Bitmap($LiteralPath)
    try {
        $count = 0
        for ($y = 0; $y -lt $bitmap.Height; $y++) {
            for ($x = 0; $x -lt $bitmap.Width; $x++) {
                $pixel = $bitmap.GetPixel($x, $y)
                $delta = [Math]::Abs(
                    [int]$pixel.R - [int]$script:BackdropColor.R) +
                    [Math]::Abs(
                        [int]$pixel.G - [int]$script:BackdropColor.G) +
                    [Math]::Abs(
                        [int]$pixel.B - [int]$script:BackdropColor.B)
                if ($delta -gt 18) {
                    $count++
                }
            }
        }
        return $count
    }
    finally {
        $bitmap.Dispose()
    }
}

function Save-PrivacySafePetCapture {
    param(
        [Parameter(Mandatory = $true)]
        [object] $Window,

        [Parameter(Mandatory = $true)]
        [string] $Name,

        [Parameter(Mandatory = $true)]
        [string] $Directory,

        [Parameter(Mandatory = $true)]
        [string] $CaptureDirectory,

        [Parameter(Mandatory = $true)]
        [double] $ElapsedSeconds,

        [switch] $KeepHeld
    )
    $current = [MechoFly.N41VisualReview.WindowProbe]::Current(
        [int64]$Window.handle)
    Assert-Condition ($null -ne $current) 'The review pet disappeared before capture.'
    $existing = @{}
    Get-ChildItem -LiteralPath $CaptureDirectory -Filter 'boundary-*.png' |
        ForEach-Object { $existing[$_.FullName] = $true }
    Assert-Condition (
        [MechoFly.N41VisualReview.WindowProbe]::SetEvidenceHold(
            [int64]$current.handle,
            $true)) 'Could not freeze the pet at the review boundary.'
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        $source = $null
        while ([DateTime]::UtcNow -lt $deadline) {
            $source = Get-ChildItem `
                -LiteralPath $CaptureDirectory `
                -Filter 'boundary-*.png' |
                Where-Object { -not $existing.ContainsKey($_.FullName) } |
                Sort-Object Name |
                Select-Object -First 1
            if ($null -ne $source) { break }
            Start-Sleep -Milliseconds 50
        }
        Assert-Condition ($null -ne $source) (
            'The candidate did not emit its direct pet-buffer boundary capture.')
        $path = Join-Path $Directory ($Name + '.png')
        Copy-Item -LiteralPath $source.FullName -Destination $path -Force
        $bitmap = New-Object System.Drawing.Bitmap($path)
        try {
            $width = $bitmap.Width
            $height = $bitmap.Height
        }
        finally {
            $bitmap.Dispose()
        }
        Assert-Condition ($width -eq 420 -and $height -eq 280) (
            'Direct pet-buffer capture dimensions were not 420 x 280.')
        $visiblePixels = Get-NonBackdropPixelCount -LiteralPath $path
        Assert-Condition ($visiblePixels -ge 500) (
            'Privacy-safe capture did not contain a visible pet. pixels=' +
            [string]$visiblePixels)
        return [pscustomobject][ordered]@{
            name = $Name
            relative_path = ($Name + '.png')
            sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
            width = $width
            height = $height
            elapsed_seconds = [Math]::Round($ElapsedSeconds, 3)
            non_backdrop_pixels = $visiblePixels
            capture_scope = 'direct pet BGRA buffer over constant backdrop; no screen API'
            full_desktop_captured = $false
            evidence_hold = $true
            source_window_title = [string]$current.title
            source_window_class = [string]$current.class_name
        }
    }
    finally {
        if (-not $KeepHeld) {
            [void][MechoFly.N41VisualReview.WindowProbe]::SetEvidenceHold(
                [int64]$current.handle,
                $false)
        }
    }
}

function Ask-Criterion {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Phase,

        [Parameter(Mandatory = $true)]
        [string] $Criterion,

        [Parameter(Mandatory = $true)]
        [string] $Question
    )
    $answer = [System.Windows.Forms.MessageBox]::Show(
        $Question + [Environment]::NewLine + [Environment]::NewLine +
            'Yes = pass, No = fail, Cancel = abort the review.',
        ('MechoFly N4.1-B ' + $Phase + ' review — ' + $Criterion),
        [System.Windows.Forms.MessageBoxButtons]::YesNoCancel,
        [System.Windows.Forms.MessageBoxIcon]::Question,
        [System.Windows.Forms.MessageBoxDefaultButton]::Button1)
    $value = switch ($answer) {
        ([System.Windows.Forms.DialogResult]::Yes) { 'PASS' }
        ([System.Windows.Forms.DialogResult]::No) { 'FAIL' }
        default { 'ABORT' }
    }
    return [pscustomobject][ordered]@{
        phase = $Phase.ToLowerInvariant()
        criterion = $Criterion.ToLowerInvariant()
        result = $value
        recorded_utc = [DateTime]::UtcNow.ToString('o')
    }
}

function Get-PhaseRatings {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Phase
    )
    $naturalnessQuestion = if ($Phase -ceq 'Late') {
        'Did walking create real screen displacement with stops, curves, and turns, and did you see a recognizable multi-step grooming sequence (head/forelegs plus abdomen or wing) rather than pivoting or walking in place?'
    }
    else {
        'Did walking create real screen displacement with believable straight/curved bouts and stops, rather than rotating or walking in place?'
    }
    $ratings = New-Object System.Collections.Generic.List[object]
    foreach ($item in @(
        [pscustomobject]@{
            criterion = 'Responsiveness'
            question = 'Did the fly remain visibly responsive when you hovered, clicked, or used its bounded interaction controls?'
        },
        [pscustomobject]@{
            criterion = 'Naturalness'
            question = $naturalnessQuestion
        },
        [pscustomobject]@{
            criterion = 'Non-disruption'
            question = 'Was the fly acceptable to keep on the desktop without becoming obstructive or distracting?'
        }
    )) {
        $rating = Ask-Criterion `
            -Phase $Phase `
            -Criterion $item.criterion `
            -Question $item.question
        $ratings.Add($rating)
        if ($rating.result -eq 'ABORT') {
            break
        }
    }
    return [object[]]$ratings.ToArray()
}

function Get-NaturalMotionMetrics {
    param(
        [Parameter(Mandatory = $true)]
        [string] $TracePath,

        [Parameter(Mandatory = $true)]
        [string] $CaptureDirectory
    )
    Assert-Condition (Test-Path -LiteralPath $TracePath -PathType Leaf) (
        'Candidate motion trace is missing: ' + $TracePath)
    $previous = $null
    $previousBehavior = ''
    $walkingPairs = 0
    $translatedPairs = 0
    $stationaryRotationPairs = 0
    $walkingPath = 0.0
    $groomingBouts = 0
    $groomingFrames = New-Object 'System.Collections.Generic.HashSet[long]'
    $groomingSubstates = New-Object 'System.Collections.Generic.HashSet[string]'
    foreach ($line in [System.IO.File]::ReadLines($TracePath)) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        $sample = $line | ConvertFrom-Json
        $behavior = [string]$sample.behavior
        if ($behavior -ceq 'groom') {
            [void]$groomingFrames.Add([long]$sample.model_frame)
            if ($null -ne $sample.grooming_substate) {
                [void]$groomingSubstates.Add(
                    [string]$sample.grooming_substate)
            }
            if ($previousBehavior -cne 'groom') { $groomingBouts++ }
        }
        if ($null -ne $previous -and
            $behavior -ceq 'walk' -and
            [string]$previous.behavior -ceq 'walk' -and
            -not [bool]$sample.dragging -and
            -not [bool]$sample.cursor_hovered -and
            -not [bool]$sample.evidence_hold) {
            $dx = [double]$sample.screen_x - [double]$previous.screen_x
            $dy = [double]$sample.screen_y - [double]$previous.screen_y
            $distance = [Math]::Sqrt($dx * $dx + $dy * $dy)
            $headingDelta = [Math]::Abs(
                [double]$sample.heading_radians -
                [double]$previous.heading_radians)
            while ($headingDelta -gt [Math]::PI) {
                $headingDelta = [Math]::Abs(
                    $headingDelta - 2.0 * [Math]::PI)
            }
            $walkingPairs++
            $walkingPath += $distance
            if ($distance -ge 0.05) { $translatedPairs++ }
            if ($distance -lt 0.05 -and $headingDelta -gt 0.003) {
                $stationaryRotationPairs++
            }
        }
        $previous = $sample
        $previousBehavior = $behavior
    }
    $requiredCaptures = @(
        'groom-head-sweep.png',
        'groom-foreleg-rub.png',
        'groom-abdomen-brush.png',
        'groom-wing-clean.png'
    )
    $observedCaptures = @($requiredCaptures | Where-Object {
        Test-Path -LiteralPath (Join-Path $CaptureDirectory $_) -PathType Leaf
    })
    $stationaryRatio = if ($walkingPairs -gt 0) {
        [double]$stationaryRotationPairs / [double]$walkingPairs
    }
    else { 1.0 }
    $groomingSeconds = [double]$groomingFrames.Count * 0.033
    $passed = $walkingPath -ge 100.0 -and
        $translatedPairs -ge 50 -and
        $stationaryRatio -le 0.05 -and
        $groomingBouts -ge 1 -and
        $groomingSeconds -ge 1.5 -and
        $observedCaptures.Count -eq $requiredCaptures.Count -and
        $groomingSubstates.Count -ge 4
    return [pscustomobject][ordered]@{
        status = if ($passed) { 'PASS' } else { 'FAIL' }
        walking_sample_pairs = $walkingPairs
        translated_walking_pairs = $translatedPairs
        walking_path_pixels = [Math]::Round($walkingPath, 3)
        stationary_rotation_pairs = $stationaryRotationPairs
        stationary_rotation_ratio = [Math]::Round($stationaryRatio, 6)
        grooming_bouts = $groomingBouts
        grooming_frames = $groomingFrames.Count
        grooming_seconds = [Math]::Round($groomingSeconds, 3)
        grooming_substates = @($groomingSubstates | Sort-Object)
        required_grooming_captures = $requiredCaptures
        observed_grooming_captures = $observedCaptures
        gate = 'translation >= 100 px; >= 50 translated pairs; stationary-rotation ratio <= 0.05; >= 1 autonomous groom bout lasting >= 1.5 s; all four action captures'
    }
}

$executable = (Resolve-Path -LiteralPath $ExecutablePath).Path
$observedExecutableSha256 = (Get-FileHash `
    -LiteralPath $executable `
    -Algorithm SHA256).Hash.ToLowerInvariant()
Assert-Condition ($observedExecutableSha256 -ceq $ExecutableSha256) (
    'Candidate executable SHA-256 mismatch: ' + $observedExecutableSha256)

$downloads = [System.IO.Path]::GetFullPath(
    (Join-Path ([Environment]::GetFolderPath('UserProfile')) 'Downloads'))
if (-not (Test-Path -LiteralPath $downloads -PathType Container)) {
    New-Item -ItemType Directory -Path $downloads -Force | Out-Null
}
if (-not (Test-Path -LiteralPath $OutputDirectory -PathType Container)) {
    New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
}
$output = (Resolve-Path -LiteralPath $OutputDirectory).Path
$downloadsPrefix = $downloads.TrimEnd(
    [System.IO.Path]::DirectorySeparatorChar,
    [System.IO.Path]::AltDirectorySeparatorChar) +
    [System.IO.Path]::DirectorySeparatorChar
Assert-Condition ($output.StartsWith(
    $downloadsPrefix,
    [StringComparison]::OrdinalIgnoreCase)) (
    'Visual-review output must be inside Downloads: ' + $output)

$existing = @(Get-CimInstance Win32_Process `
    -Filter "Name='MechoFly.exe'" `
    -ErrorAction SilentlyContinue)
Assert-Condition ($existing.Count -eq 0) (
    'Quit the existing MechoFly pet before review. The collector will not stop a process it did not start.')

if ($null -eq ('MechoFly.N41VisualReview.WindowProbe' -as [type])) {
    $references = @(
        [System.Drawing.Bitmap].Assembly.Location
        [System.Windows.Forms.Application].Assembly.Location
    )
    Add-Type -ReferencedAssemblies $references -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows.Forms;

namespace MechoFly.N41VisualReview
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

    public sealed class CaptureBackdropForm : Form
    {
        public CaptureBackdropForm()
        {
            FormBorderStyle = FormBorderStyle.None;
            ShowInTaskbar = false;
            TopMost = true;
        }

        protected override bool ShowWithoutActivation { get { return true; } }

        protected override CreateParams CreateParams
        {
            get
            {
                const int WsExNoActivate = 0x08000000;
                CreateParams parameters = base.CreateParams;
                parameters.ExStyle |= WsExNoActivate;
                return parameters;
            }
        }
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

        public static bool SetEvidenceHold(long handle, bool held)
        {
            const uint EvidenceHoldMessage = 0x804D;
            return PostMessage(
                new IntPtr(handle),
                EvidenceHoldMessage,
                new UIntPtr(held ? 1U : 0U),
                IntPtr.Zero);
        }

        public static WindowInfo Current(long handle)
        {
            IntPtr hwnd = new IntPtr(handle);
            if (!IsWindowVisible(hwnd)) return null;
            Rect rect;
            if (!GetWindowRect(hwnd, out rect)) return null;
            return Describe(hwnd, rect);
        }

        private static WindowInfo Describe(IntPtr hwnd, Rect rect)
        {
            StringBuilder title = new StringBuilder(512);
            StringBuilder className = new StringBuilder(256);
            GetWindowText(hwnd, title, title.Capacity);
            GetClassName(hwnd, className, className.Capacity);
            return new WindowInfo
            {
                handle = hwnd.ToInt64(),
                x = rect.Left,
                y = rect.Top,
                width = rect.Right - rect.Left,
                height = rect.Bottom - rect.Top,
                title = title.ToString(),
                class_name = className.ToString()
            };
        }

        public static WindowInfo[] ForProcess(uint expectedProcessId)
        {
            List<WindowInfo> result = new List<WindowInfo>();
            EnumWindows(delegate(IntPtr hwnd, IntPtr parameter)
            {
                uint processId;
                GetWindowThreadProcessId(hwnd, out processId);
                if (processId != expectedProcessId || !IsWindowVisible(hwnd)) return true;
                Rect rect;
                if (!GetWindowRect(hwnd, out rect)) return true;
                if (rect.Right - rect.Left < 32 || rect.Bottom - rect.Top < 32) return true;
                result.Add(Describe(hwnd, rect));
                return true;
            }, IntPtr.Zero);
            return result.ToArray();
        }
    }
}
'@
}

$intro = [System.Windows.Forms.MessageBox]::Show(
    'This launches the isolated N4.1-B candidate for ten minutes.' +
        [Environment]::NewLine + [Environment]::NewLine +
        'Observe the first 30 seconds, then the final five-minute window.' +
        ' Hover and click the pet occasionally so responsiveness is visible.' +
        [Environment]::NewLine + [Environment]::NewLine +
        'The review does not deploy anything, change shortcuts, or write to AppData.' +
        ' Press Cancel if you cannot observe the full session.',
    'MechoFly N4.1-B early/late visual acceptance',
    [System.Windows.Forms.MessageBoxButtons]::OKCancel,
    [System.Windows.Forms.MessageBoxIcon]::Information,
    [System.Windows.Forms.MessageBoxDefaultButton]::Button1)
Assert-Condition ($intro -eq [System.Windows.Forms.DialogResult]::OK) (
    'The reviewer cancelled before the candidate was launched.')

$dataDirectory = Join-Path $output 'app-data'
New-Item -ItemType Directory -Path $dataDirectory -Force | Out-Null
$runtimeProfilePath = Join-Path $dataDirectory 'runtime-profile.json'
Write-JsonFile -LiteralPath $runtimeProfilePath -Value ([ordered]@{
    skin = 'firefly'
    compute = 'auto'
    reduced_motion = $false
    source_branch = $SourceBranch
    source_commit = $SourceCommit
    source_tree = $SourceTree
    executable_sha256 = $ExecutableSha256
})

$launchReceiptPath = Join-Path $output 'candidate-launch-receipt.json'
$stdoutPath = Join-Path $output 'candidate-stdout.txt'
$stderrPath = Join-Path $output 'candidate-stderr.txt'
$arguments = @(
    '--skin', 'firefly',
    '--compute', 'auto',
    '--n41-b-visual-review',
    '--n41-visual-review-receipt', ('"' + $launchReceiptPath + '"')
)
$previousDataDirectory = $env:MECHOFLY_DATA_DIR
$process = $null
$reviewStopwatch = $null
$captures = New-Object System.Collections.Generic.List[object]
$earlyRatings = @()
$lateRatings = @()
$visualDecision = 'ABORTED'
$finalConfirmation = 'NOT_ASKED'
$objectiveMetrics = $null
$startedUtc = [DateTime]::UtcNow
try {
    $env:MECHOFLY_DATA_DIR = $dataDirectory
    $process = Start-Process `
        -FilePath $executable `
        -WorkingDirectory (Split-Path -Parent $executable) `
        -ArgumentList $arguments `
        -RedirectStandardOutput $stdoutPath `
        -RedirectStandardError $stderrPath `
        -PassThru
}
finally {
    $env:MECHOFLY_DATA_DIR = $previousDataDirectory
}

try {
    $deadline = [DateTime]::UtcNow.AddSeconds(45)
    $petWindow = $null
    while ([DateTime]::UtcNow -lt $deadline) {
        Test-ProcessAlive -Process $process -StandardErrorPath $stderrPath
        if (Test-Path -LiteralPath $launchReceiptPath -PathType Leaf) {
            try {
                $petWindow = Get-ReviewPetWindow -ProcessId $process.Id
                break
            }
            catch {
                $petWindow = $null
            }
        }
        Start-Sleep -Milliseconds 250
    }
    Assert-Condition ($null -ne $petWindow) (
        'The feature-gated candidate did not publish its labeled pet and launch receipt.')

    $launchReceipt = Get-Content -LiteralPath $launchReceiptPath -Raw |
        ConvertFrom-Json
    Assert-Condition ([string]$launchReceipt.status -ceq 'PASS') 'Candidate launch receipt failed.'
    Assert-Condition ([string]$launchReceipt.active_profile -ceq 'n41-b') 'Candidate did not activate n41-b.'
    Assert-Condition ([string]$launchReceipt.canonical_default_profile -ceq 'n4') 'Canonical default is not N4.'
    Assert-Condition ([string]$launchReceipt.parameter_sha256 -ceq $ExpectedParameterSha256) 'N4.1-B parameter identity mismatch.'
    Assert-Condition ([string]$launchReceipt.executable_sha256 -ceq $ExecutableSha256) 'Candidate launch binary identity mismatch.'
    Assert-Condition ([bool]$launchReceipt.storage_override_active) 'Downloads storage override was not active.'
    Assert-Condition ([string]$launchReceipt.capture_source -ceq
        'direct pet BGRA buffer composited over a constant backdrop; no screen capture') (
        'Candidate did not declare direct-buffer privacy-safe capture.')
    $captureDirectory = [string]$launchReceipt.capture_directory
    $tracePath = [string]$launchReceipt.trace_path
    Assert-Condition ($captureDirectory.StartsWith(
        $dataDirectory,
        [StringComparison]::OrdinalIgnoreCase)) (
        'Candidate capture directory escaped the Downloads-only review directory.')
    Assert-Condition ($tracePath.StartsWith(
        $dataDirectory,
        [StringComparison]::OrdinalIgnoreCase)) (
        'Candidate trace path escaped the Downloads-only review directory.')
    Assert-Condition (-not [bool]$launchReceipt.promotion_authorized) 'Candidate incorrectly authorized promotion.'
    Assert-Condition (-not [bool]$launchReceipt.deployment_authorized) 'Candidate incorrectly authorized deployment.'

    $reviewStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    Write-Host 'EARLY_WINDOW=Observe the candidate through 30 seconds.' -ForegroundColor Cyan
    Wait-ReviewBoundary `
        -Stopwatch $reviewStopwatch `
        -Seconds $EarlyBoundarySeconds `
        -Process $process `
        -StandardErrorPath $stderrPath `
        -Label 'Early boundary'
    $petWindow = Get-ReviewPetWindow -ProcessId $process.Id
    $captures.Add((Save-PrivacySafePetCapture `
        -Window $petWindow `
        -Name 'early-30s' `
        -Directory $output `
        -CaptureDirectory $captureDirectory `
        -ElapsedSeconds $reviewStopwatch.Elapsed.TotalSeconds))
    $earlyRatings = Get-PhaseRatings -Phase 'Early'
    if (@($earlyRatings | Where-Object { $_.result -eq 'ABORT' }).Count -gt 0) {
        $visualDecision = 'ABORTED'
    }
    else {
        Write-Host 'WARMUP_WINDOW=The candidate continues to five modeled minutes.' -ForegroundColor Cyan
        Wait-ReviewBoundary `
            -Stopwatch $reviewStopwatch `
            -Seconds $LateBoundarySeconds `
            -Process $process `
            -StandardErrorPath $stderrPath `
            -Label 'Late-window start'
        [Console]::Beep(880, 300)
        Write-Host 'LATE_WINDOW=Observe from five through ten minutes.' -ForegroundColor Cyan
        $petWindow = Get-ReviewPetWindow -ProcessId $process.Id
        $captures.Add((Save-PrivacySafePetCapture `
            -Window $petWindow `
            -Name 'late-start-5m' `
            -Directory $output `
            -CaptureDirectory $captureDirectory `
            -ElapsedSeconds $reviewStopwatch.Elapsed.TotalSeconds))

        Wait-ReviewBoundary `
            -Stopwatch $reviewStopwatch `
            -Seconds $LateMidpointSeconds `
            -Process $process `
            -StandardErrorPath $stderrPath `
            -Label 'Late midpoint'
        $petWindow = Get-ReviewPetWindow -ProcessId $process.Id
        $captures.Add((Save-PrivacySafePetCapture `
            -Window $petWindow `
            -Name 'late-mid-7m30s' `
            -Directory $output `
            -CaptureDirectory $captureDirectory `
            -ElapsedSeconds $reviewStopwatch.Elapsed.TotalSeconds))

        Wait-ReviewBoundary `
            -Stopwatch $reviewStopwatch `
            -Seconds $FinalBoundarySeconds `
            -Process $process `
            -StandardErrorPath $stderrPath `
            -Label 'Final boundary'
        $petWindow = Get-ReviewPetWindow -ProcessId $process.Id
        $captures.Add((Save-PrivacySafePetCapture `
            -Window $petWindow `
            -Name 'late-end-10m' `
            -Directory $output `
            -CaptureDirectory $captureDirectory `
            -ElapsedSeconds $reviewStopwatch.Elapsed.TotalSeconds `
            -KeepHeld))
        $objectiveMetrics = Get-NaturalMotionMetrics `
            -TracePath $tracePath `
            -CaptureDirectory $captureDirectory
        Write-Host ('NATURAL_MOTION_GATE=' + $objectiveMetrics.status +
            ' walking_path_pixels=' + [string]$objectiveMetrics.walking_path_pixels +
            ' grooming_bouts=' + [string]$objectiveMetrics.grooming_bouts)
        [Console]::Beep(1047, 400)
        $lateRatings = Get-PhaseRatings -Phase 'Late'
        $lateAborted = @($lateRatings | Where-Object {
            $_.result -eq 'ABORT'
        }).Count -gt 0
        $allRatings = @($earlyRatings) + @($lateRatings)
        if ($lateAborted) {
            $visualDecision = 'ABORTED'
        }
        elseif ([string]$objectiveMetrics.status -cne 'PASS' -or
            @($allRatings | Where-Object {
            $_.result -ne 'PASS'
        }).Count -gt 0) {
            $visualDecision = 'REJECTED'
        }
        else {
            $confirmation = [System.Windows.Forms.MessageBox]::Show(
                'All six early/late criteria passed.' +
                    [Environment]::NewLine + [Environment]::NewLine +
                    'Accept this exact N4.1-B candidate binary only for the next guarded step?' +
                    ' This still does not authorize deployment or shortcut changes.',
                'MechoFly N4.1-B explicit visual-acceptance decision',
                [System.Windows.Forms.MessageBoxButtons]::YesNo,
                [System.Windows.Forms.MessageBoxIcon]::Question,
                [System.Windows.Forms.MessageBoxDefaultButton]::Button2)
            if ($confirmation -eq [System.Windows.Forms.DialogResult]::Yes) {
                $finalConfirmation = 'YES'
                $visualDecision = 'ACCEPTED_FOR_GUARDED_NEXT_STEP'
            }
            else {
                $finalConfirmation = 'NO'
                $visualDecision = 'REJECTED'
            }
        }
    }

    $completedSeconds = if ($null -ne $reviewStopwatch) {
        [Math]::Round($reviewStopwatch.Elapsed.TotalSeconds, 3)
    }
    else {
        0
    }
    $receipt = [ordered]@{
        schema_version = 2
        status = 'PASS'
        classification = 'single_owner_formative_early_late_visual_review'
        candidate_profile = 'n41-b'
        parameter_sha256 = $ExpectedParameterSha256
        source_branch = $SourceBranch
        source_commit = $SourceCommit
        source_tree = $SourceTree
        executable_sha256 = $ExecutableSha256
        launch_receipt = 'candidate-launch-receipt.json'
        protocol_seconds = $FinalBoundarySeconds
        early_boundary_seconds = $EarlyBoundarySeconds
        late_window_start_seconds = $LateBoundarySeconds
        late_window_end_seconds = $FinalBoundarySeconds
        observed_elapsed_seconds = $completedSeconds
        early_ratings = @($earlyRatings)
        late_ratings = @($lateRatings)
        final_confirmation = $finalConfirmation
        visual_acceptance = $visualDecision
        objective_natural_motion = $objectiveMetrics
        captures = [object[]]$captures.ToArray()
        screenshot_scope = 'direct pet BGRA buffer over constant backdrop; no screen API'
        full_desktop_captured = $false
        appdata_write_authorized = $false
        storage_directory = $dataDirectory
        collector_launched_candidate = $true
        collector_stopped_only_started_process = $true
        blinded_review = $false
        promotion_authorized = $false
        deployment_authorized = $false
        shortcut_changes = $false
        started_utc = $startedUtc.ToString('o')
        completed_utc = [DateTime]::UtcNow.ToString('o')
    }
    Write-JsonFile `
        -LiteralPath (Join-Path $output 'visual-review-receipt.json') `
        -Value $receipt
    Write-Host ('N41_VISUAL_REVIEW=PASS decision=' + $visualDecision)
}
finally {
    if ($null -ne $process) {
        $process.Refresh()
        if (-not $process.HasExited) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            $process.WaitForExit()
        }
    }
}
