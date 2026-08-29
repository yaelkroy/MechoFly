#requires -version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $ExecutablePath,

    [string] $ReceiptPath = (Join-Path $PSScriptRoot `
        '..\artifacts\pet-topmost.json'),

    [ValidateRange(3, 60)]
    [int] $ObservationSeconds = 12
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Executable = (Resolve-Path -LiteralPath $ExecutablePath).Path
$ReceiptDirectory = Split-Path -Parent $ReceiptPath
if (-not (Test-Path -LiteralPath $ReceiptDirectory -PathType Container)) {
    New-Item -ItemType Directory -Path $ReceiptDirectory -Force | Out-Null
}

if ($null -eq ('MechoFly.TopmostOracle.WindowProbe' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace MechoFly.TopmostOracle
{
    public sealed class WindowInfo
    {
        public long handle { get; set; }
        public int z_index { get; set; }
        public string title { get; set; }
        public string class_name { get; set; }
        public bool is_topmost { get; set; }
    }

    public static class WindowProbe
    {
        private delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr parameter);

        [DllImport("user32.dll")]
        private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);

        [DllImport("user32.dll")]
        private static extern bool IsWindowVisible(IntPtr hwnd);

        [DllImport("user32.dll")]
        private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int count);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetClassName(IntPtr hwnd, StringBuilder text, int count);

        [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
        private static extern IntPtr GetWindowLongPtr(IntPtr hwnd, int index);

        [DllImport("user32.dll")]
        private static extern bool SetForegroundWindow(IntPtr hwnd);

        private static bool IsTopmost(IntPtr hwnd)
        {
            const int ExtendedStyle = -20;
            const long TopmostStyle = 0x00000008L;
            return (GetWindowLongPtr(hwnd, ExtendedStyle).ToInt64()
                & TopmostStyle) != 0;
        }

        public static bool Activate(long handle)
        {
            return SetForegroundWindow(new IntPtr(handle));
        }

        public static WindowInfo[] ForProcess(uint expectedProcessId)
        {
            List<WindowInfo> result = new List<WindowInfo>();
            int zIndex = 0;
            EnumWindows(delegate(IntPtr hwnd, IntPtr parameter)
            {
                int currentZ = zIndex++;
                uint processId;
                GetWindowThreadProcessId(hwnd, out processId);
                if (processId != expectedProcessId || !IsWindowVisible(hwnd))
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
                    z_index = currentZ,
                    title = title.ToString(),
                    class_name = className.ToString(),
                    is_topmost = IsTopmost(hwnd)
                });
                return true;
            }, IntPtr.Zero);
            return result.ToArray();
        }
    }
}
'@
}

$Stdout = $ReceiptPath + '.stdout.txt'
$Stderr = $ReceiptPath + '.stderr.txt'
$Arguments = @('--skin', 'firefly', '--compute', 'auto', '--brain-lab')
$StartedUtc = [DateTime]::UtcNow
$Process = Start-Process `
    -FilePath $Executable `
    -WorkingDirectory (Split-Path -Parent $Executable) `
    -ArgumentList $Arguments `
    -RedirectStandardOutput $Stdout `
    -RedirectStandardError $Stderr `
    -PassThru

try {
    $Deadline = [DateTime]::UtcNow.AddSeconds($ObservationSeconds)
    $Windows = @()
    $Pet = $null
    $BrainLab = $null
    $LiveBrain = $null
    do {
        Start-Sleep -Milliseconds 200
        $Process.Refresh()
        if ($Process.HasExited) {
            $ErrorText = if (Test-Path -LiteralPath $Stderr -PathType Leaf) {
                [System.IO.File]::ReadAllText($Stderr)
            }
            else {
                ''
            }
            throw ('MechoFly exited before the topmost oracle completed. ' +
                'ExitCode=' + [string]$Process.ExitCode + [Environment]::NewLine +
                $ErrorText)
        }
        $Windows = @([MechoFly.TopmostOracle.WindowProbe]::ForProcess(
            [uint32]$Process.Id))
        $Pet = @($Windows | Where-Object {
            [string]$_.title -eq 'MechoFly desktop pet'
        }) | Select-Object -First 1
        $BrainLab = @($Windows | Where-Object {
            [string]$_.title -like '*Brain Lab*'
        }) | Select-Object -First 1
        $LiveBrain = @($Windows | Where-Object {
            [string]$_.title -like '*Live Brain*'
        }) | Select-Object -First 1
    } while (
        [DateTime]::UtcNow -lt $Deadline -and
        ($null -eq $Pet -or $null -eq $BrainLab -or $null -eq $LiveBrain)
    )

    if ($null -eq $Pet -or $null -eq $BrainLab -or $null -eq $LiveBrain) {
        throw 'The pet, Live Brain, and Brain Lab were not all visible.'
    }

    $BrainLabActivationRequested =
        [MechoFly.TopmostOracle.WindowProbe]::Activate(
            [int64]$BrainLab.handle)
    Start-Sleep -Milliseconds 500
    $Windows = @([MechoFly.TopmostOracle.WindowProbe]::ForProcess(
        [uint32]$Process.Id))
    $Pet = @($Windows | Where-Object {
        [string]$_.title -eq 'MechoFly desktop pet'
    }) | Select-Object -First 1
    $BrainLab = @($Windows | Where-Object {
        [string]$_.title -like '*Brain Lab*'
    }) | Select-Object -First 1

    if (-not [bool]$Pet.is_topmost) {
        throw 'The desktop pet lost WS_EX_TOPMOST while Brain Lab was open.'
    }
    if ([int]$Pet.z_index -ge [int]$BrainLab.z_index) {
        throw ('The desktop pet is below Brain Lab in Windows z-order. ' +
            'pet=' + [string]$Pet.z_index + '; brain_lab=' +
            [string]$BrainLab.z_index)
    }

    $Receipt = [ordered]@{
        schema_version = 1
        status = 'PASS'
        started_utc = $StartedUtc.ToString('o')
        completed_utc = [DateTime]::UtcNow.ToString('o')
        executable = $Executable
        executable_sha256 = (Get-FileHash `
            -LiteralPath $Executable `
            -Algorithm SHA256).Hash
        process_id = $Process.Id
        arguments = $Arguments
        brain_lab_activation_requested = $BrainLabActivationRequested
        desktop_pet_topmost = [bool]$Pet.is_topmost
        desktop_pet_z_index = [int]$Pet.z_index
        brain_lab_z_index = [int]$BrainLab.z_index
        desktop_pet_above_brain_lab = $true
        windows = @($Windows)
        live_hardware_authority = 'NONE'
    }
    [System.IO.File]::WriteAllText(
        $ReceiptPath,
        (($Receipt | ConvertTo-Json -Depth 8) + [Environment]::NewLine),
        (New-Object System.Text.UTF8Encoding($false)))
    Write-Host ('MECHOFLY_PET_TOPMOST=PASS pet_z=' +
        [string]$Pet.z_index + ' brain_lab_z=' +
        [string]$BrainLab.z_index)
}
finally {
    $Process.Refresh()
    if (-not $Process.HasExited) {
        Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
        $Process.WaitForExit()
    }
}
