#requires -version 5.1
[CmdletBinding()]
param(
    [string] $ZipPath = (Join-Path $env:USERPROFILE `
        'Downloads\MechoFly-Rust-Windows-StartupFix-3409d35.zip'),

    [ValidateRange(8, 30)]
    [int] $ObservationSeconds = 12,

    [switch] $NoLaunch
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ExpectedZipSha256 = '0B8A699717B9A684747DB5CB0B486366F11BD1F30ED44238F86698A901E6D81F'
$ExpectedExecutableSha256 = 'A83B5E8369CD1C1AD1F496D8D75626B6737BF9AA4EF168DC530A359A1BA5B74C'
$ExpectedPdbSha256 = '1E763F2421F42A5E5FFE9F639D26F36669011F6487D3BF0A4BB054172735CD40'
$ReviewCommit = '3409d3517b090cc3d9fea1931edb7e8a7d89df5d'
$WorkflowRun = 'https://github.com/yaelkroy/MechoFly/actions/runs/32917278903'
$CanonicalRepository = 'https://github.com/yaelkroy/MechoFly.git'
$ReviewParent = 'D:\Projects\MechoFly-Reviews'
$ReviewTarget = Join-Path $ReviewParent 'StartupFix-3409d35'
$Downloads = Join-Path $env:USERPROFILE 'Downloads'
$CollectionStart = Get-Date
$Stamp = $CollectionStart.ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
$OutputZip = Join-Path $Downloads (
    'UPLOAD_MechoFly_StartupFix_3409d35_AI100_Evidence_' +
    $Stamp + '_PID' + [string]$PID + '.zip')
$LatestReceiptPath = Join-Path $Downloads `
    'UPLOAD_MechoFly_StartupFix_3409d35_LATEST.txt'
$WorkingRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
    'MechoFly-StartupFix-Evidence-' + [Guid]::NewGuid().ToString('N'))
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$StagingDirectory = $null
$TranscriptStarted = $false
$Failure = $null
$Cases = New-Object System.Collections.Generic.List[object]
$Screenshots = New-Object System.Collections.Generic.List[object]
$CreatedShortcuts = New-Object System.Collections.Generic.List[string]
$InteractiveProcess = $null
$InteractiveLaunch = $null
$InstalledExecutable = $null
$InstalledPdb = $null
$ArtifactHash = $null
$ExecutableHash = $null
$PdbHash = $null
$SelfTestReceiptPath = $null

function Write-Utf8Text {
    param(
        [Parameter(Mandatory = $true)]
        [string] $LiteralPath,

        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string] $Text
    )

    [System.IO.File]::WriteAllText($LiteralPath, $Text, $script:Utf8NoBom)
}

function Write-Utf8Lines {
    param(
        [Parameter(Mandatory = $true)]
        [string] $LiteralPath,

        [Parameter(Mandatory = $true)]
        [AllowEmptyCollection()]
        [string[]] $Lines
    )

    [System.IO.File]::WriteAllLines($LiteralPath, $Lines, $script:Utf8NoBom)
}

function Write-JsonFile {
    param(
        [Parameter(Mandatory = $true)]
        [string] $LiteralPath,

        [Parameter(Mandatory = $true)]
        [AllowNull()]
        [object] $Value,

        [int] $Depth = 10
    )

    $Json = $Value | ConvertTo-Json -Depth $Depth
    Write-Utf8Text -LiteralPath $LiteralPath -Text (
        $Json + [Environment]::NewLine)
}

function Assert-PowerShellSyntax {
    param(
        [Parameter(Mandatory = $true)]
        [string] $LiteralPath
    )

    $Tokens = $null
    $ParseErrors = $null
    [System.Management.Automation.Language.Parser]::ParseFile(
        $LiteralPath,
        [ref]$Tokens,
        [ref]$ParseErrors) | Out-Null
    if (@($ParseErrors).Count -gt 0) {
        $Details = @(
            $ParseErrors |
                ForEach-Object { $_.Message + ' at ' + $_.Extent.Text }
        ) -join '; '
        throw ('Generated PowerShell script did not parse: ' +
            $LiteralPath + '. ' + $Details)
    }
}

function Test-SafeZipEntries {
    param(
        [Parameter(Mandatory = $true)]
        [string] $ArchivePath,

        [Parameter(Mandatory = $true)]
        [string] $Destination
    )

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $DestinationFullPath = [System.IO.Path]::GetFullPath($Destination)
    $Separators = [char[]]@(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar)
    $DestinationPrefix = $DestinationFullPath.TrimEnd($Separators) +
        [System.IO.Path]::DirectorySeparatorChar
    $Archive = [System.IO.Compression.ZipFile]::OpenRead($ArchivePath)
    try {
        foreach ($Entry in $Archive.Entries) {
            if ([string]::IsNullOrWhiteSpace($Entry.FullName)) {
                throw 'The artifact contains an empty ZIP entry name.'
            }
            if ($Entry.FullName.StartsWith('/') -or
                $Entry.FullName.StartsWith('\') -or
                $Entry.FullName.Contains(':')) {
                throw ('The artifact contains an unsafe ZIP entry: ' +
                    $Entry.FullName)
            }

            $RelativePath = $Entry.FullName.Replace(
                [System.IO.Path]::AltDirectorySeparatorChar,
                [System.IO.Path]::DirectorySeparatorChar)
            $ExpandedPath = [System.IO.Path]::GetFullPath(
                (Join-Path $DestinationFullPath $RelativePath))
            if (-not $ExpandedPath.StartsWith(
                    $DestinationPrefix,
                    [StringComparison]::OrdinalIgnoreCase)) {
                throw ('The artifact contains a path-traversal entry: ' +
                    $Entry.FullName)
            }
        }
    }
    finally {
        $Archive.Dispose()
    }
}

function Stop-ExactMechoFlyProcesses {
    param(
        [Parameter(Mandatory = $true)]
        [string] $ExpectedExecutable
    )

    $Stopped = 0
    $Processes = @(
        Get-CimInstance Win32_Process `
            -Filter "Name='MechoFly.exe'" `
            -ErrorAction SilentlyContinue
    )
    foreach ($CimProcess in $Processes) {
        if (-not [string]::Equals(
                [string]$CimProcess.ExecutablePath,
                $ExpectedExecutable,
                [StringComparison]::OrdinalIgnoreCase)) {
            continue
        }

        $Process = Get-Process -Id $CimProcess.ProcessId -ErrorAction SilentlyContinue
        if ($null -eq $Process) {
            continue
        }
        try {
            $Process.Refresh()
            if ($Process.MainWindowHandle -ne [IntPtr]::Zero) {
                [void]$Process.CloseMainWindow()
                [void]$Process.WaitForExit(2500)
            }
        }
        catch {
            Write-Verbose $_.Exception.Message
        }

        $Process = Get-Process -Id $CimProcess.ProcessId -ErrorAction SilentlyContinue
        if ($null -ne $Process) {
            Stop-Process -Id $CimProcess.ProcessId -Force -ErrorAction Stop
            [void]$Process.WaitForExit(3000)
        }
        $Stopped++
    }
    return $Stopped
}

function Assert-SelfTestReceipt {
    param(
        [Parameter(Mandatory = $true)]
        [object] $Receipt
    )

    $Checks = [ordered]@{
        status = ($Receipt.status -eq 'PASS')
        implementation = ($Receipt.implementation -eq 'independent-rust-rebuild')
        live_state_unchanged = ([bool]$Receipt.live_state_unchanged)
        alternative_differs = ([bool]$Receipt.alternative_differs)
        default_skin = ($Receipt.default_skin -eq 'drosophila')
        drosophila_skin_available = ([bool]$Receipt.drosophila_skin_available)
        firefly_skin_available = ([bool]$Receipt.firefly_skin_available)
        startup_capacity_evaluation = ([bool]$Receipt.startup_capacity_evaluation)
        reevaluation_control = ([bool]$Receipt.reevaluation_control)
        compute_auto = (@($Receipt.compute_modes) -contains 'auto')
        compute_cpu = (@($Receipt.compute_modes) -contains 'cpu')
        compute_gpu = (@($Receipt.compute_modes) -contains 'gpu')
        vendor_neutral_gpu_policy =
            ($Receipt.gpu_policy -eq
                'wgpu-wgsl-capability-and-exactness-no-vendor-allowlist')
        cpu_without_gpu_supported = ([bool]$Receipt.cpu_without_gpu_supported)
        explicit_feedback_learning = ([bool]$Receipt.learning_requires_explicit_feedback)
        connectome_not_mutated = (-not [bool]$Receipt.connectome_mutated_by_learning)
        not_measured_activity = (-not [bool]$Receipt.measured_activity)
        no_live_hardware_authority = ($Receipt.live_hardware_authority -eq 'NONE')
    }

    $Failures = @(
        $Checks.GetEnumerator() |
            Where-Object { -not $_.Value } |
            ForEach-Object { $_.Key }
    )
    if ($Failures.Count -gt 0) {
        throw ('MechoFly safety self-test failed: ' + ($Failures -join ', '))
    }
}

function Initialize-WindowCapture {
    Add-Type -AssemblyName System.Drawing
    Add-Type -AssemblyName System.Windows.Forms

    if ($null -eq ('MechoFlyStartupFixCapture.NativeMethods' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace MechoFlyStartupFixCapture
{
    [StructLayout(LayoutKind.Sequential)]
    public struct NativeRect
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    public static class NativeMethods
    {
        public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);

        [DllImport("user32.dll")]
        public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool IsWindowVisible(IntPtr hWnd);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern int GetWindowTextLength(IntPtr hWnd);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int maximum);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern int GetClassName(IntPtr hWnd, StringBuilder text, int maximum);

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool GetWindowRect(IntPtr hWnd, out NativeRect rectangle);

        [DllImport("dwmapi.dll")]
        public static extern int DwmGetWindowAttribute(
            IntPtr hWnd,
            int attribute,
            out NativeRect value,
            int valueSize);

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool ShowWindow(IntPtr hWnd, int command);

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool SetForegroundWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool SetProcessDPIAware();
    }
}
'@
    }

    try {
        [void][MechoFlyStartupFixCapture.NativeMethods]::SetProcessDPIAware()
    }
    catch {
        Write-Verbose $_.Exception.Message
    }
}

function Get-MechoFlyDesignWindows {
    param(
        [Parameter(Mandatory = $true)]
        [int] $ProcessId
    )

    $Candidates = New-Object System.Collections.Generic.List[object]
    $Callback = [MechoFlyStartupFixCapture.NativeMethods+EnumWindowsProc]{
        param(
            [IntPtr] $WindowHandle,
            [IntPtr] $CallbackParameter
        )

        [uint32]$WindowProcessId = 0
        [void][MechoFlyStartupFixCapture.NativeMethods]::GetWindowThreadProcessId(
            $WindowHandle,
            [ref]$WindowProcessId)
        if ([int]$WindowProcessId -ne $ProcessId -or
            -not [MechoFlyStartupFixCapture.NativeMethods]::IsWindowVisible(
                $WindowHandle)) {
            return $true
        }

        $TitleLength = [MechoFlyStartupFixCapture.NativeMethods]::GetWindowTextLength(
            $WindowHandle)
        $TitleBuffer = New-Object System.Text.StringBuilder(
            ([Math]::Max(1, $TitleLength + 1)))
        [void][MechoFlyStartupFixCapture.NativeMethods]::GetWindowText(
            $WindowHandle,
            $TitleBuffer,
            $TitleBuffer.Capacity)
        $Title = $TitleBuffer.ToString()
        if ($Title -notlike 'MechoFly*') {
            return $true
        }

        $ClassBuffer = New-Object System.Text.StringBuilder(256)
        [void][MechoFlyStartupFixCapture.NativeMethods]::GetClassName(
            $WindowHandle,
            $ClassBuffer,
            $ClassBuffer.Capacity)

        $Rectangle = New-Object MechoFlyStartupFixCapture.NativeRect
        $BoundsSource = 'DwmExtendedFrameBounds'
        $DwmResult = [MechoFlyStartupFixCapture.NativeMethods]::DwmGetWindowAttribute(
            $WindowHandle,
            9,
            [ref]$Rectangle,
            [Runtime.InteropServices.Marshal]::SizeOf($Rectangle))
        if ($DwmResult -ne 0) {
            $BoundsSource = 'GetWindowRect'
            [void][MechoFlyStartupFixCapture.NativeMethods]::GetWindowRect(
                $WindowHandle,
                [ref]$Rectangle)
        }

        $Width = $Rectangle.Right - $Rectangle.Left
        $Height = $Rectangle.Bottom - $Rectangle.Top
        if ($Width -lt 640 -or $Height -lt 400) {
            return $true
        }

        [void]$Candidates.Add([PSCustomObject]@{
            process_id = [int]$WindowProcessId
            handle = $WindowHandle.ToInt64()
            title = $Title
            class_name = $ClassBuffer.ToString()
            left = $Rectangle.Left
            top = $Rectangle.Top
            width = $Width
            height = $Height
            bounds_source = $BoundsSource
            area = [int64]$Width * [int64]$Height
        })
        return $true
    }

    [void][MechoFlyStartupFixCapture.NativeMethods]::EnumWindows(
        $Callback,
        [IntPtr]::Zero)
    return @($Candidates.ToArray() | Sort-Object area -Descending)
}

function Save-DesignScreenshot {
    param(
        [Parameter(Mandatory = $true)]
        [object] $Window,

        [Parameter(Mandatory = $true)]
        [string] $CaseName,

        [Parameter(Mandatory = $true)]
        [string] $SampleName,

        [Parameter(Mandatory = $true)]
        [string] $DestinationDirectory
    )

    $Handle = [IntPtr]$Window.handle
    [void][MechoFlyStartupFixCapture.NativeMethods]::ShowWindow($Handle, 9)
    [void][MechoFlyStartupFixCapture.NativeMethods]::SetForegroundWindow($Handle)
    Start-Sleep -Milliseconds 450

    $FileName = $CaseName + '_' + $SampleName + '.png'
    $Path = Join-Path $DestinationDirectory $FileName
    $Bitmap = New-Object System.Drawing.Bitmap($Window.width, $Window.height)
    $Graphics = [System.Drawing.Graphics]::FromImage($Bitmap)
    try {
        $Graphics.CopyFromScreen(
            $Window.left,
            $Window.top,
            0,
            0,
            (New-Object System.Drawing.Size($Window.width, $Window.height)),
            [System.Drawing.CopyPixelOperation]::SourceCopy)
        $Bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    }
    finally {
        $Graphics.Dispose()
        $Bitmap.Dispose()
    }

    $Record = [PSCustomObject]@{
        case = $CaseName
        sample = $SampleName
        captured_utc = [DateTime]::UtcNow.ToString('o')
        process_id = $Window.process_id
        handle = $Window.handle
        title = $Window.title
        class_name = $Window.class_name
        bounds = [ordered]@{
            left = $Window.left
            top = $Window.top
            width = $Window.width
            height = $Window.height
            source = $Window.bounds_source
        }
        relative_path = 'screenshots/' + $FileName
        privacy_scope = 'MechoFly Brain Lab window rectangle only; no desktop capture'
    }
    [void]$script:Screenshots.Add($Record)
    return $Record
}

function Invoke-ObservedGuiCase {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,

        [Parameter(Mandatory = $true)]
        [string] $Compute,

        [Parameter(Mandatory = $true)]
        [string] $Executable,

        [Parameter(Mandatory = $true)]
        [string] $WorkingDirectory,

        [Parameter(Mandatory = $true)]
        [string] $RuntimeDirectory,

        [Parameter(Mandatory = $true)]
        [string] $ScreenshotDirectory,

        [Parameter(Mandatory = $true)]
        [int] $Seconds
    )

    $CaseDirectory = Join-Path $RuntimeDirectory $Name
    New-Item -ItemType Directory -Path $CaseDirectory -Force | Out-Null
    $StandardOutput = Join-Path $CaseDirectory 'stdout.txt'
    $StandardError = Join-Path $CaseDirectory 'stderr.txt'
    $Arguments = @(
        '--skin', 'firefly',
        '--compute', $Compute,
        '--brain-lab')
    $StartedUtc = [DateTime]::UtcNow
    $Process = Start-Process `
        -FilePath $Executable `
        -WorkingDirectory $WorkingDirectory `
        -ArgumentList $Arguments `
        -RedirectStandardOutput $StandardOutput `
        -RedirectStandardError $StandardError `
        -PassThru
    Write-Host ('MECHOFLY_OBSERVED_CASE=' + $Name + '|PID=' +
        [string]$Process.Id)

    $Watch = [Diagnostics.Stopwatch]::StartNew()
    $FirstCapture = $null
    $FinalCapture = $null
    $ExitedEarly = $false
    $ExitCode = $null
    try {
        while ($Watch.Elapsed.TotalSeconds -lt $Seconds) {
            Start-Sleep -Milliseconds 250
            $Process.Refresh()
            if ($Process.HasExited) {
                $ExitedEarly = $true
                $ExitCode = $Process.ExitCode
                break
            }

            $Elapsed = $Watch.Elapsed.TotalSeconds
            if ($null -eq $FirstCapture -and $Elapsed -ge 2.0) {
                $Windows = @(Get-MechoFlyDesignWindows -ProcessId $Process.Id)
                if ($Windows.Count -gt 0) {
                    $FirstCapture = Save-DesignScreenshot `
                        -Window $Windows[0] `
                        -CaseName $Name `
                        -SampleName 'settled' `
                        -DestinationDirectory $ScreenshotDirectory
                }
            }
            if ($null -eq $FinalCapture -and
                $Elapsed -ge ([Math]::Max(4, $Seconds - 2))) {
                $Windows = @(Get-MechoFlyDesignWindows -ProcessId $Process.Id)
                if ($Windows.Count -gt 0) {
                    $FinalCapture = Save-DesignScreenshot `
                        -Window $Windows[0] `
                        -CaseName $Name `
                        -SampleName 'final' `
                        -DestinationDirectory $ScreenshotDirectory
                }
            }
        }
    }
    finally {
        $Watch.Stop()
        $Process.Refresh()
        if (-not $Process.HasExited) {
            try {
                if ($Process.MainWindowHandle -ne [IntPtr]::Zero) {
                    [void]$Process.CloseMainWindow()
                    [void]$Process.WaitForExit(2500)
                }
            }
            catch {
                Write-Verbose $_.Exception.Message
            }
        }
        $Process.Refresh()
        if (-not $Process.HasExited) {
            Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
            [void]$Process.WaitForExit(3000)
        }
        $Process.Refresh()
        if ($null -eq $ExitCode -and $ExitedEarly) {
            $ExitCode = $Process.ExitCode
        }
    }

    $ScreenshotCount = @($FirstCapture, $FinalCapture |
        Where-Object { $null -ne $_ }).Count
    $Status = if (-not $ExitedEarly -and $ScreenshotCount -gt 0) {
        'PASS'
    }
    elseif ($ExitedEarly) {
        'FAIL_PROCESS_EXITED'
    }
    else {
        'FAIL_NO_DESIGN_WINDOW'
    }

    $Result = [PSCustomObject]@{
        schema_version = 1
        status = $Status
        case = $Name
        executable = $Executable
        executable_sha256 = $script:ExpectedExecutableSha256
        arguments = @($Arguments)
        compute = $Compute
        skin = 'firefly'
        brain_lab_requested = $true
        started_utc = $StartedUtc.ToString('o')
        finished_utc = [DateTime]::UtcNow.ToString('o')
        observation_seconds = $Seconds
        observed_seconds = [Math]::Round($Watch.Elapsed.TotalSeconds, 3)
        process_id = $Process.Id
        exited_before_collector_stop = $ExitedEarly
        exit_code = $ExitCode
        screenshot_count = $ScreenshotCount
        survived_startup_boundary = (-not $ExitedEarly)
        collector_stopped_process = (-not $ExitedEarly)
        live_hardware_authority = 'NONE'
    }
    Write-JsonFile `
        -LiteralPath (Join-Path $CaseDirectory 'receipt.json') `
        -Value $Result
    return $Result
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
        [string] $Arguments,

        [Parameter(Mandatory = $true)]
        [string] $Description,

        [Parameter(Mandatory = $true)]
        [string] $WorkingDirectory,

        [Parameter(Mandatory = $true)]
        [string] $IconLocation
    )

    $Shortcut = $Shell.CreateShortcut($ShortcutPath)
    $Shortcut.TargetPath = $TargetPath
    $Shortcut.Arguments = $Arguments
    $Shortcut.Description = $Description
    $Shortcut.WorkingDirectory = $WorkingDirectory
    $Shortcut.IconLocation = $IconLocation
    $Shortcut.WindowStyle = 1
    $Shortcut.Save()
}

function Copy-NewStartupLogs {
    param(
        [Parameter(Mandatory = $true)]
        [DateTime] $Since,

        [Parameter(Mandatory = $true)]
        [string] $Destination
    )

    $LogDirectory = Join-Path $env:LOCALAPPDATA 'MechoFly\logs'
    if (-not (Test-Path -LiteralPath $LogDirectory -PathType Container)) {
        return
    }
    Get-ChildItem -LiteralPath $LogDirectory -Filter 'startup-*.log' -File |
        Where-Object { $_.LastWriteTimeUtc -ge $Since.ToUniversalTime().AddMinutes(-1) } |
        ForEach-Object {
            Copy-Item -LiteralPath $_.FullName -Destination (
                Join-Path $Destination $_.Name) -Force
        }
}

try {
    New-Item -ItemType Directory -Path $WorkingRoot -Force | Out-Null
    foreach ($RelativeDirectory in @(
        'configuration',
        'runtime',
        'screenshots',
        'system',
        'startup-logs')) {
        New-Item -ItemType Directory -Path (
            Join-Path $WorkingRoot $RelativeDirectory) -Force | Out-Null
    }
    Start-Transcript -LiteralPath (
        Join-Path $WorkingRoot 'installer-and-collector-transcript.txt') -Force |
        Out-Null
    $TranscriptStarted = $true

    Write-Host 'MECHOFLY_STARTUP_FIX_INSTALL_TEST_CAPTURE=START'
    Write-Host 'SOURCE_MUTATION=NO'
    Write-Host 'LIVE_HARDWARE_AUTHORITY=NONE'
    Write-Host ('OUTPUT_ZIP=' + $OutputZip)

    if (-not (Test-Path -LiteralPath 'D:\' -PathType Container)) {
        throw 'AI100 review installation requires the D: drive.'
    }
    if (-not (Test-Path -LiteralPath $Downloads -PathType Container)) {
        throw ('Downloads directory was not found: ' + $Downloads)
    }

    $ResolvedZipPath = [System.IO.Path]::GetFullPath($ZipPath)
    if (-not (Test-Path -LiteralPath $ResolvedZipPath -PathType Leaf)) {
        throw ('The required artifact was not found: ' + $ResolvedZipPath)
    }
    $ArtifactHash = (
        Get-FileHash -LiteralPath $ResolvedZipPath -Algorithm SHA256).Hash
    if ($ArtifactHash -ne $ExpectedZipSha256) {
        throw ('Artifact hash mismatch. Expected ' + $ExpectedZipSha256 +
            '; received ' + $ArtifactHash + '.')
    }
    Write-Host ('MECHOFLY_ARTIFACT_HASH=PASS|' + $ArtifactHash)

    if (-not (Test-Path -LiteralPath $ReviewParent -PathType Container)) {
        New-Item -ItemType Directory -Path $ReviewParent -Force | Out-Null
    }
    $StagingDirectory = Join-Path $ReviewParent (
        '.staging-startup-fix-3409d35-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $StagingDirectory -Force | Out-Null
    Test-SafeZipEntries `
        -ArchivePath $ResolvedZipPath `
        -Destination $StagingDirectory
    [System.IO.Compression.ZipFile]::ExtractToDirectory(
        $ResolvedZipPath,
        $StagingDirectory)

    $StagedExecutable = Join-Path $StagingDirectory `
        'host-windows\bin\MechoFly.exe'
    $StagedPdb = Join-Path $StagingDirectory 'host-windows\bin\MechoFly.pdb'
    $StagedCiSelfTest = Join-Path $StagingDirectory 'artifacts\self-test.json'
    $StagedCiRuntime = Join-Path $StagingDirectory `
        'artifacts\runtime-smoke\summary.json'
    foreach ($RequiredFile in @(
        $StagedExecutable,
        $StagedPdb,
        $StagedCiSelfTest,
        $StagedCiRuntime)) {
        if (-not (Test-Path -LiteralPath $RequiredFile -PathType Leaf)) {
            throw ('Verified artifact is incomplete: ' + $RequiredFile)
        }
    }

    $StagedExecutableHash = (
        Get-FileHash -LiteralPath $StagedExecutable -Algorithm SHA256).Hash
    $StagedPdbHash = (
        Get-FileHash -LiteralPath $StagedPdb -Algorithm SHA256).Hash
    if ($StagedExecutableHash -ne $ExpectedExecutableSha256) {
        throw 'Executable hash did not match the verified CI build.'
    }
    if ($StagedPdbHash -ne $ExpectedPdbSha256) {
        throw 'PDB hash did not match the verified CI build.'
    }

    $InstalledExecutable = Join-Path $ReviewTarget `
        'host-windows\bin\MechoFly.exe'
    $InstalledPdb = Join-Path $ReviewTarget 'host-windows\bin\MechoFly.pdb'
    if (Test-Path -LiteralPath $ReviewTarget -PathType Container) {
        [void](Stop-ExactMechoFlyProcesses `
            -ExpectedExecutable $InstalledExecutable)
    }

    $ReuseExisting = $false
    if ((Test-Path -LiteralPath $InstalledExecutable -PathType Leaf) -and
        (Test-Path -LiteralPath $InstalledPdb -PathType Leaf)) {
        $ExistingExecutableHash = (
            Get-FileHash -LiteralPath $InstalledExecutable -Algorithm SHA256).Hash
        $ExistingPdbHash = (
            Get-FileHash -LiteralPath $InstalledPdb -Algorithm SHA256).Hash
        $ReuseExisting =
            ($ExistingExecutableHash -eq $ExpectedExecutableSha256) -and
            ($ExistingPdbHash -eq $ExpectedPdbSha256)
    }

    if (-not $ReuseExisting -and
        (Test-Path -LiteralPath $ReviewTarget -PathType Container)) {
        $BackupTarget = $ReviewTarget + '.previous-' + $Stamp
        if (Test-Path -LiteralPath $BackupTarget) {
            $BackupTarget += '-' +
                [Guid]::NewGuid().ToString('N').Substring(0, 8)
        }
        Move-Item -LiteralPath $ReviewTarget -Destination $BackupTarget
        Write-Host ('MECHOFLY_PREVIOUS_REVIEW_BACKUP=' + $BackupTarget)
    }
    if (-not $ReuseExisting) {
        Move-Item -LiteralPath $StagingDirectory -Destination $ReviewTarget
        $StagingDirectory = $null
    }

    $ExecutableHash = (
        Get-FileHash -LiteralPath $InstalledExecutable -Algorithm SHA256).Hash
    $PdbHash = (
        Get-FileHash -LiteralPath $InstalledPdb -Algorithm SHA256).Hash
    if ($ExecutableHash -ne $ExpectedExecutableSha256 -or
        $PdbHash -ne $ExpectedPdbSha256) {
        throw 'Installed binary or symbols failed final verification.'
    }
    Write-Host ('MECHOFLY_INSTALL=PASS|' + $ReviewTarget)
    Write-Host ('MECHOFLY_EXECUTABLE_SHA256=' + $ExecutableHash)
    Write-Host ('MECHOFLY_PDB_SHA256=' + $PdbHash)

    Copy-Item -LiteralPath (
        Join-Path $ReviewTarget 'artifacts\self-test.json') `
        -Destination (Join-Path $WorkingRoot `
            'configuration\ci-self-test.json') -Force
    Copy-Item -LiteralPath (
        Join-Path $ReviewTarget 'artifacts\runtime-smoke\summary.json') `
        -Destination (Join-Path $WorkingRoot `
            'configuration\ci-runtime-smoke.json') -Force

    $ReviewProfile = [ordered]@{
        schema_version = 1
        machine_role = 'ai100-review'
        skin = 'firefly'
        compute = 'auto'
        review_commit = $ReviewCommit
        workflow_run = $WorkflowRun
        canonical_repository = $CanonicalRepository
        workspace = $ReviewTarget
        artifact_sha256 = $ExpectedZipSha256
        executable_sha256 = $ExpectedExecutableSha256
        pdb_sha256 = $ExpectedPdbSha256
        source_mutation = $false
        live_hardware_authority = 'NONE'
        generated_utc = [DateTime]::UtcNow.ToString('o')
    }
    Write-JsonFile `
        -LiteralPath (Join-Path $ReviewTarget 'review-profile.json') `
        -Value $ReviewProfile
    Write-JsonFile `
        -LiteralPath (Join-Path $WorkingRoot `
            'configuration\review-profile.json') `
        -Value $ReviewProfile

    $SelfTestReceiptPath = Join-Path $WorkingRoot `
        'runtime\ai100-self-test.json'
    $SelfTestArguments = '--self-test "{0}"' -f $SelfTestReceiptPath
    $SelfTestProcess = Start-Process `
        -FilePath $InstalledExecutable `
        -WorkingDirectory (Split-Path -Parent $InstalledExecutable) `
        -ArgumentList $SelfTestArguments `
        -Wait `
        -PassThru
    if ($SelfTestProcess.ExitCode -ne 0) {
        throw ('AI100 self-test exited with code ' +
            [string]$SelfTestProcess.ExitCode + '.')
    }
    if (-not (Test-Path -LiteralPath $SelfTestReceiptPath -PathType Leaf)) {
        throw 'AI100 self-test did not produce a receipt.'
    }
    $SelfTestReceipt = Get-Content -LiteralPath $SelfTestReceiptPath -Raw |
        ConvertFrom-Json
    Assert-SelfTestReceipt -Receipt $SelfTestReceipt
    Write-Host 'MECHOFLY_AI100_SELF_TEST=PASS'

    $LauncherDirectory = Join-Path $ReviewTarget 'host-windows'
    $EscapedExecutable = $InstalledExecutable.Replace("'", "''")
    $EscapedWorkingDirectory = (
        Split-Path -Parent $InstalledExecutable).Replace("'", "''")
    $StartScript = Join-Path $LauncherDirectory `
        'Start-MechoFly-StartupFix.ps1'
    $BrainLabScript = Join-Path $LauncherDirectory `
        'Open-MechoFly-Brain-Lab-StartupFix.ps1'
    $StopScript = Join-Path $LauncherDirectory `
        'Stop-MechoFly-StartupFix.ps1'
    Write-Utf8Lines -LiteralPath $StartScript -Lines @(
        '#requires -version 5.1',
        'Set-StrictMode -Version Latest',
        '$ErrorActionPreference = ''Stop''',
        ('$Executable = ''' + $EscapedExecutable + ''''),
        ('$WorkingDirectory = ''' + $EscapedWorkingDirectory + ''''),
        '$Process = Start-Process -FilePath $Executable -WorkingDirectory $WorkingDirectory -ArgumentList @(''--skin'', ''firefly'', ''--compute'', ''auto'') -PassThru',
        'Start-Sleep -Seconds 5',
        '$Process.Refresh()',
        'if ($Process.HasExited) { throw (''MechoFly exited during startup with code '' + [string]$Process.ExitCode + ''. Inspect %LOCALAPPDATA%\MechoFly\logs.'') }',
        'Write-Host (''MECHOFLY_STARTED_PID='' + [string]$Process.Id)'
    )
    Write-Utf8Lines -LiteralPath $BrainLabScript -Lines @(
        '#requires -version 5.1',
        'Set-StrictMode -Version Latest',
        '$ErrorActionPreference = ''Stop''',
        ('$Executable = ''' + $EscapedExecutable + ''''),
        ('$WorkingDirectory = ''' + $EscapedWorkingDirectory + ''''),
        '$Process = Start-Process -FilePath $Executable -WorkingDirectory $WorkingDirectory -ArgumentList @(''--skin'', ''firefly'', ''--compute'', ''auto'', ''--brain-lab'') -PassThru',
        'Start-Sleep -Seconds 5',
        '$Process.Refresh()',
        'if ($Process.HasExited) { throw (''MechoFly Brain Lab exited during startup with code '' + [string]$Process.ExitCode + ''. Inspect %LOCALAPPDATA%\MechoFly\logs.'') }',
        'Write-Host (''MECHOFLY_BRAIN_LAB_STARTED_PID='' + [string]$Process.Id)'
    )
    Write-Utf8Lines -LiteralPath $StopScript -Lines @(
        '#requires -version 5.1',
        'Set-StrictMode -Version Latest',
        '$ErrorActionPreference = ''Stop''',
        ('$ExpectedExecutable = ''' + $EscapedExecutable + ''''),
        '$Stopped = 0',
        '$Processes = @(Get-CimInstance Win32_Process -Filter "Name=''MechoFly.exe''" -ErrorAction SilentlyContinue)',
        'foreach ($Process in $Processes) {',
        '    if ([string]::Equals([string]$Process.ExecutablePath, $ExpectedExecutable, [StringComparison]::OrdinalIgnoreCase)) {',
        '        Stop-Process -Id $Process.ProcessId -Force -ErrorAction Stop',
        '        $Stopped++',
        '    }',
        '}',
        'Write-Host (''MECHOFLY_STARTUP_FIX_PROCESSES_STOPPED='' + [string]$Stopped)'
    )
    foreach ($GeneratedScript in @(
        $StartScript,
        $BrainLabScript,
        $StopScript)) {
        Assert-PowerShellSyntax -LiteralPath $GeneratedScript
    }

    $Desktop = [Environment]::GetFolderPath('Desktop')
    if ([string]::IsNullOrWhiteSpace($Desktop)) {
        throw 'The current user Desktop directory could not be resolved.'
    }
    $WindowsPowerShell = Join-Path $env:SystemRoot `
        'System32\WindowsPowerShell\v1.0\powershell.exe'
    $PowerShellPrefix = '-NoLogo -NoProfile -ExecutionPolicy Bypass -File "'
    $Shell = New-Object -ComObject WScript.Shell
    try {
        foreach ($ShortcutDefinition in @(
            [PSCustomObject]@{
                Name = 'Start MechoFly Startup Fix.lnk'
                Script = $StartScript
                Description = 'Start the verified MechoFly startup-fix review.'
            },
            [PSCustomObject]@{
                Name = 'MechoFly Brain Lab Startup Fix.lnk'
                Script = $BrainLabScript
                Description = 'Open the verified Firefly Brain Lab startup-fix review.'
            },
            [PSCustomObject]@{
                Name = 'Stop MechoFly Startup Fix.lnk'
                Script = $StopScript
                Description = 'Stop only this MechoFly startup-fix review.'
            }
        )) {
            $ShortcutPath = Join-Path $Desktop $ShortcutDefinition.Name
            New-MechoFlyShortcut `
                -Shell $Shell `
                -ShortcutPath $ShortcutPath `
                -TargetPath $WindowsPowerShell `
                -Arguments ($PowerShellPrefix +
                    $ShortcutDefinition.Script + '"') `
                -Description $ShortcutDefinition.Description `
                -WorkingDirectory $ReviewTarget `
                -IconLocation ($InstalledExecutable + ',0')
            [void]$CreatedShortcuts.Add($ShortcutPath)
        }
    }
    finally {
        if ($null -ne $Shell) {
            [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($Shell)
        }
    }

    Initialize-WindowCapture
    $RuntimeDirectory = Join-Path $WorkingRoot 'runtime'
    $ScreenshotDirectory = Join-Path $WorkingRoot 'screenshots'
    $PreviousBacktrace = $env:RUST_BACKTRACE
    $PreviousRustLog = $env:RUST_LOG
    try {
        $env:RUST_BACKTRACE = '1'
        $env:RUST_LOG = 'mechofly_app=debug,wgpu=warn,eframe=info'
        foreach ($Definition in @(
            [PSCustomObject]@{ Name = 'firefly-brain-lab-cpu'; Compute = 'cpu' },
            [PSCustomObject]@{ Name = 'firefly-brain-lab-auto'; Compute = 'auto' }
        )) {
            $Result = Invoke-ObservedGuiCase `
                -Name $Definition.Name `
                -Compute $Definition.Compute `
                -Executable $InstalledExecutable `
                -WorkingDirectory (Split-Path -Parent $InstalledExecutable) `
                -RuntimeDirectory $RuntimeDirectory `
                -ScreenshotDirectory $ScreenshotDirectory `
                -Seconds $ObservationSeconds
            [void]$Cases.Add($Result)
        }
    }
    finally {
        if ($null -eq $PreviousBacktrace) {
            Remove-Item Env:RUST_BACKTRACE -ErrorAction SilentlyContinue
        }
        else {
            $env:RUST_BACKTRACE = $PreviousBacktrace
        }
        if ($null -eq $PreviousRustLog) {
            Remove-Item Env:RUST_LOG -ErrorAction SilentlyContinue
        }
        else {
            $env:RUST_LOG = $PreviousRustLog
        }
    }

    $FailedCases = @($Cases.ToArray() |
        Where-Object { $_.status -ne 'PASS' })
    if ($FailedCases.Count -gt 0) {
        throw ('AI100 GUI validation failed: ' +
            (($FailedCases | ForEach-Object {
                $_.case + '=' + $_.status
            }) -join ', '))
    }
    Write-Host ('MECHOFLY_AI100_GUI_VALIDATION=PASS|cases=' +
        [string]$Cases.Count + '|screenshots=' +
        [string]$Screenshots.Count)

    if (-not $NoLaunch) {
        $InteractiveProcess = Start-Process `
            -FilePath $InstalledExecutable `
            -WorkingDirectory (Split-Path -Parent $InstalledExecutable) `
            -ArgumentList @(
                '--skin', 'firefly',
                '--compute', 'auto',
                '--brain-lab') `
            -PassThru
        Start-Sleep -Seconds 5
        $InteractiveProcess.Refresh()
        $InteractiveLaunch = [ordered]@{
            requested = $true
            process_id = $InteractiveProcess.Id
            survived_five_seconds = (-not $InteractiveProcess.HasExited)
            exit_code = if ($InteractiveProcess.HasExited) {
                $InteractiveProcess.ExitCode
            }
            else {
                $null
            }
            arguments = @(
                '--skin', 'firefly',
                '--compute', 'auto',
                '--brain-lab')
        }
        if ($InteractiveProcess.HasExited) {
            throw ('Interactive MechoFly launch exited with code ' +
                [string]$InteractiveProcess.ExitCode + '.')
        }
    }
    else {
        $InteractiveLaunch = [ordered]@{
            requested = $false
            process_id = $null
            survived_five_seconds = $null
            exit_code = $null
            arguments = @()
        }
    }
}
catch {
    $Failure = $_
    if (Test-Path -LiteralPath $WorkingRoot -PathType Container) {
        Write-Utf8Text `
            -LiteralPath (Join-Path $WorkingRoot 'failure.txt') `
            -Text ($_ | Format-List * -Force | Out-String -Width 240)
    }
    Write-Error -ErrorRecord $_ -ErrorAction Continue
}
finally {
    if ($null -ne $StagingDirectory -and
        (Test-Path -LiteralPath $StagingDirectory -PathType Container)) {
        Remove-Item -LiteralPath $StagingDirectory -Recurse -Force `
            -ErrorAction SilentlyContinue
    }

    if (Test-Path -LiteralPath $WorkingRoot -PathType Container) {
        try {
            $SystemEvidence = [ordered]@{
                collected_utc = [DateTime]::UtcNow.ToString('o')
                computer = $env:COMPUTERNAME
                user = $env:USERNAME
                powershell = $PSVersionTable.PSVersion.ToString()
                os = @(Get-CimInstance Win32_OperatingSystem `
                    -ErrorAction SilentlyContinue |
                    Select-Object Caption, Version, BuildNumber, OSArchitecture,
                        TotalVisibleMemorySize, FreePhysicalMemory)
                processors = @(Get-CimInstance Win32_Processor `
                    -ErrorAction SilentlyContinue |
                    Select-Object Name, NumberOfCores,
                        NumberOfLogicalProcessors, MaxClockSpeed)
                video_controllers = @(Get-CimInstance Win32_VideoController `
                    -ErrorAction SilentlyContinue |
                    Select-Object Name, AdapterRAM, DriverVersion,
                        VideoProcessor, CurrentHorizontalResolution,
                        CurrentVerticalResolution)
            }
            Write-JsonFile `
                -LiteralPath (Join-Path $WorkingRoot `
                    'system\system-evidence.json') `
                -Value $SystemEvidence `
                -Depth 8
        }
        catch {
            Write-Utf8Text `
                -LiteralPath (Join-Path $WorkingRoot `
                    'system\system-evidence-error.txt') `
                -Text ($_ | Out-String)
        }

        try {
            Copy-NewStartupLogs `
                -Since $CollectionStart `
                -Destination (Join-Path $WorkingRoot 'startup-logs')
        }
        catch {
            Write-Utf8Text `
                -LiteralPath (Join-Path $WorkingRoot `
                    'startup-logs\copy-error.txt') `
                -Text ($_ | Out-String)
        }

        try {
            $ApplicationEvents = @(
                Get-WinEvent `
                    -FilterHashtable @{
                        LogName = 'Application'
                        StartTime = $CollectionStart.AddMinutes(-15)
                    } `
                    -MaxEvents 400 `
                    -ErrorAction Stop |
                    Where-Object {
                        $_.Message -match 'MechoFly|MechoFly\.exe'
                    } |
                    ForEach-Object {
                        [PSCustomObject]@{
                            time_created = $_.TimeCreated.ToUniversalTime().ToString('o')
                            provider = $_.ProviderName
                            event_id = $_.Id
                            level = $_.LevelDisplayName
                            message = $_.Message
                        }
                    }
            )
            Write-JsonFile `
                -LiteralPath (Join-Path $WorkingRoot `
                    'runtime\application-events.json') `
                -Value $ApplicationEvents `
                -Depth 6
        }
        catch {
            Write-Utf8Text `
                -LiteralPath (Join-Path $WorkingRoot `
                    'runtime\application-events-error.txt') `
                -Text ($_ | Out-String)
        }

        Write-JsonFile `
            -LiteralPath (Join-Path $WorkingRoot `
                'screenshots\screenshot-index.json') `
            -Value @($Screenshots.ToArray()) `
            -Depth 10
        $Manifest = [ordered]@{
            schema_version = 1
            collector = 'MechoFly Startup Fix 3409d35 installer/test/design capture'
            status = if ($null -eq $Failure) { 'PASS' } else { 'FAIL' }
            collection_started_utc = $CollectionStart.ToUniversalTime().ToString('o')
            collection_finished_utc = [DateTime]::UtcNow.ToString('o')
            source_mutation = $false
            live_hardware_authority = 'NONE'
            canonical_repository = $CanonicalRepository
            review_commit = $ReviewCommit
            workflow_run = $WorkflowRun
            review_workspace = $ReviewTarget
            artifact_path = $ZipPath
            artifact_sha256 = $ArtifactHash
            executable_path = $InstalledExecutable
            executable_sha256 = $ExecutableHash
            pdb_sha256 = $PdbHash
            observation_seconds_per_case = $ObservationSeconds
            cases = @($Cases.ToArray())
            screenshots = @($Screenshots.ToArray())
            screenshot_count = $Screenshots.Count
            shortcuts = @($CreatedShortcuts.ToArray())
            self_test_receipt = $SelfTestReceiptPath
            interactive_launch = $InteractiveLaunch
            failure = if ($null -eq $Failure) {
                $null
            }
            else {
                $Failure.Exception.Message
            }
            requested_output_zip = $OutputZip
        }
        Write-JsonFile `
            -LiteralPath (Join-Path $WorkingRoot 'manifest.json') `
            -Value $Manifest `
            -Depth 12

        if ($TranscriptStarted) {
            try {
                Stop-Transcript | Out-Null
            }
            catch {
                Write-Warning $_.Exception.Message
            }
            $TranscriptStarted = $false
        }

        try {
            if (Test-Path -LiteralPath $OutputZip -PathType Leaf) {
                throw ('Refusing to overwrite existing evidence ZIP: ' +
                    $OutputZip)
            }
            Compress-Archive `
                -Path (Join-Path $WorkingRoot '*') `
                -DestinationPath $OutputZip `
                -CompressionLevel Optimal
            $OutputZipHash = (
                Get-FileHash -LiteralPath $OutputZip -Algorithm SHA256).Hash
            $LatestReceipt = @(
                'schema_version=1',
                ('status=' + $Manifest.status),
                ('UPLOAD_ZIP=' + $OutputZip),
                ('UPLOAD_ZIP_SHA256=' + $OutputZipHash),
                ('review_commit=' + $ReviewCommit),
                ('generated_utc=' + [DateTime]::UtcNow.ToString('o'))
            )
            Write-Utf8Lines `
                -LiteralPath $LatestReceiptPath `
                -Lines $LatestReceipt
            Write-Host ('MECHOFLY_EVIDENCE_STATUS=' + $Manifest.status)
            Write-Host ('MECHOFLY_SCREENSHOT_COUNT=' +
                [string]$Screenshots.Count)
            Write-Host ('UPLOAD_THIS_ZIP=' + $OutputZip)
            Write-Host ('UPLOAD_ZIP_SHA256=' + $OutputZipHash)
            Write-Host ('LATEST_RECEIPT=' + $LatestReceiptPath)
        }
        catch {
            if ($null -eq $Failure) {
                $Failure = $_
            }
            Write-Error ('Evidence ZIP creation failed: ' +
                $_.Exception.Message) -ErrorAction Continue
        }
        finally {
            Remove-Item -LiteralPath $WorkingRoot -Recurse -Force `
                -ErrorAction SilentlyContinue
        }
    }
}

if ($null -ne $Failure) {
    if ($null -ne $InteractiveProcess) {
        $InteractiveProcess.Refresh()
        if (-not $InteractiveProcess.HasExited) {
            Stop-Process -Id $InteractiveProcess.Id -Force `
                -ErrorAction SilentlyContinue
        }
    }
    exit 1
}

Write-Host 'MECHOFLY_STARTUP_FIX_INSTALL_TEST_CAPTURE=PASS'
Write-Host ('MECHOFLY_REVIEW_WORKSPACE=' + $ReviewTarget)
Write-Host ('MECHOFLY_EXECUTABLE=' + $InstalledExecutable)
Write-Host 'MECHOFLY_SKIN=firefly'
Write-Host 'MECHOFLY_COMPUTE=auto'
Write-Host 'MECHOFLY_CANONICAL_WORKSPACE_UNCHANGED=D:\Projects\MechoFly'
if ($null -ne $InteractiveProcess) {
    Write-Host ('MECHOFLY_BRAIN_LAB_PROCESS_ID=' +
        [string]$InteractiveProcess.Id)
}
exit 0
