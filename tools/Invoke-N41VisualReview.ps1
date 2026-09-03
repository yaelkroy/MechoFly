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
    'cb3cd2654dcd4fa9def34fb0145645f5d61b59c96c407669cf1e9dd4f12628ef'
$ExpectedPetTitle = 'MechoFly N4.1-D exploratory flight review pet'
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
        throw ('N4.1-D visual-review process exited unexpectedly. ExitCode=' +
            [string]$Process.ExitCode + [Environment]::NewLine + $stderr)
    }
}

function Stop-ReviewCandidate {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process] $Process
    )
    $Process.Refresh()
    if ($Process.HasExited) { return }

    # This function is called only for the exact process object created below.
    # Give the GUI a bounded opportunity to close its trace writer normally.
    [void]$Process.CloseMainWindow()
    if (-not $Process.WaitForExit(3000)) {
        Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
        $Process.WaitForExit()
    }
    $Process.Refresh()
    Assert-Condition $Process.HasExited (
        'The owned visual-review candidate did not exit before trace analysis.')
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
        'Expected exactly one layered N4.1-D review pet window; found ' +
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
        ('MechoFly N4.1-D ' + $Phase + ' review — ' + $Criterion),
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
        'Across both walking and the flights you triggered, did bout length, distance, speed, and path vary visibly—with straight flight segments interrupted by quick turns, smooth takeoff-to-cruise and cruise-to-landing motion, natural stops, and recognizable grooming—without a repeated clockwork rhythm or motion in place?'
    }
    else {
        'Did walking create real displacement, and did the flights you triggered show visibly different lengths and speeds with a quick maneuver and a continuous landing, rather than repeating one fixed loop or rotating in place?'
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

function Get-MeanAndCv {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyCollection()]
        [double[]] $Values
    )
    if ($Values.Count -eq 0) {
        return [pscustomobject]@{ mean = 0.0; cv = 0.0 }
    }
    $sum = 0.0
    foreach ($value in $Values) { $sum += $value }
    $mean = $sum / [double]$Values.Count
    $variance = 0.0
    foreach ($value in $Values) {
        $variance += ($value - $mean) * ($value - $mean)
    }
    $variance /= [double]$Values.Count
    $cv = if ($mean -gt 0.0) { [Math]::Sqrt($variance) / $mean } else { 0.0 }
    return [pscustomobject]@{ mean = $mean; cv = $cv }
}

function Get-NearestRank {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyCollection()]
        [double[]] $SortedValues,

        [Parameter(Mandatory = $true)]
        [ValidateRange(0.0, 1.0)]
        [double] $Quantile
    )
    if ($SortedValues.Count -eq 0) { return 0.0 }
    $index = [Math]::Ceiling($Quantile * $SortedValues.Count) - 1
    $index = [Math]::Max(0, [Math]::Min($SortedValues.Count - 1, $index))
    return [double]$SortedValues[$index]
}

function Get-SquaredCorrelation {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyCollection()]
        [double[]] $X,

        [Parameter(Mandatory = $true)]
        [AllowEmptyCollection()]
        [double[]] $Y
    )
    if ($X.Count -lt 3 -or $X.Count -ne $Y.Count) { return 1.0 }
    $xStats = Get-MeanAndCv -Values $X
    $yStats = Get-MeanAndCv -Values $Y
    $covariance = 0.0
    $xVariance = 0.0
    $yVariance = 0.0
    for ($index = 0; $index -lt $X.Count; $index++) {
        $dx = $X[$index] - $xStats.mean
        $dy = $Y[$index] - $yStats.mean
        $covariance += $dx * $dy
        $xVariance += $dx * $dx
        $yVariance += $dy * $dy
    }
    if ($xVariance -le 0.0 -or $yVariance -le 0.0) { return 1.0 }
    $correlation = $covariance / [Math]::Sqrt($xVariance * $yVariance)
    return $correlation * $correlation
}

function Add-CompletedWalkBout {
    param(
        [Parameter(Mandatory = $true)]
        [hashtable] $Bout,

        [Parameter(Mandatory = $true)]
        [AllowEmptyCollection()]
        [System.Collections.Generic.List[object]] $Destination
    )
    if ($Bout.interactive -or $Bout.pairs -lt 1 -or $Bout.speed_count -lt 1) { return }
    $Destination.Add([pscustomobject][ordered]@{
        duration_seconds = ([double]($Bout.max_age_frames + 1) * 0.033)
        path_pixels = [double]$Bout.path_pixels
        mean_speed_pixels_per_second = [double]$Bout.speed_sum / [double]$Bout.speed_count
    })
}

function Add-CompletedFlightBout {
    param(
        [Parameter(Mandatory = $true)]
        [hashtable] $Bout,

        [Parameter(Mandatory = $true)]
        [AllowEmptyCollection()]
        [System.Collections.Generic.List[object]] $Destination
    )
    if (-not $Bout.completed_to_landing -or
        $Bout.pairs -lt 1 -or
        $Bout.speed_count -lt 1) { return }
    $Destination.Add([pscustomobject][ordered]@{
        duration_seconds = ([double]($Bout.max_age_frames + 1) * 0.033)
        path_pixels = [double]$Bout.path_pixels
        mean_speed_pixels_per_second =
            [double]$Bout.speed_sum / [double]$Bout.speed_count
        saccades = [int]$Bout.saccades
        natural_motion = [bool]$Bout.natural_motion
        normalized_x_start = [double]$Bout.normalized_x_start
        normalized_x_end = [double]$Bout.normalized_x_end
        normalized_x_min = [double]$Bout.normalized_x_min
        normalized_x_max = [double]$Bout.normalized_x_max
        normalized_x_span =
            [double]$Bout.normalized_x_max - [double]$Bout.normalized_x_min
        normalized_x_displacement =
            [double]$Bout.normalized_x_end -
            [double]$Bout.normalized_x_start
        visited_left_tertile = [bool]$Bout.visited_left_tertile
        visited_middle_tertile = [bool]$Bout.visited_middle_tertile
        visited_right_tertile = [bool]$Bout.visited_right_tertile
    })
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

    # The UI may emit several records for one model frame. Keep the last one so
    # metrics describe modeled states rather than rendering-loop frequency.
    $byFrame = @{}
    foreach ($line in [System.IO.File]::ReadLines($TracePath)) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        $sample = $line | ConvertFrom-Json
        $byFrame[[long]$sample.model_frame] = $sample
    }
    $samples = @($byFrame.Values | Sort-Object { [long]$_.model_frame })
    Assert-Condition ($samples.Count -ge 1000) (
        'Motion trace did not contain enough unique modeled frames: ' +
        [string]$samples.Count)

    $previous = $null
    $previousBehavior = ''
    $currentBout = $null
    $walkBouts = New-Object 'System.Collections.Generic.List[object]'
    $currentFlightBout = $null
    $flightBouts = New-Object 'System.Collections.Generic.List[object]'
    $walkingPairs = 0
    $translatedPairs = 0
    $stationaryRotationPairs = 0
    $walkingPath = 0.0
    $groomingBouts = 0
    $groomingFrames = New-Object 'System.Collections.Generic.HashSet[long]'
    $groomingSubstates = New-Object 'System.Collections.Generic.HashSet[string]'

    foreach ($sample in $samples) {
        $behavior = [string]$sample.behavior
        $interactive = [bool]$sample.dragging -or
            [bool]$sample.cursor_hovered -or
            [bool]$sample.evidence_hold
        $normalizedFlightX = $null
        if ($behavior -ceq 'flight') {
            foreach ($field in @(
                'movement_left',
                'movement_top',
                'movement_right',
                'movement_bottom')) {
                Assert-Condition (
                    $sample.PSObject.Properties.Name -ccontains $field) (
                    'Flight trace is missing spatial-bound field: ' + $field)
            }
            $movementLeft = [double]$sample.movement_left
            $movementTop = [double]$sample.movement_top
            $movementRight = [double]$sample.movement_right
            $movementBottom = [double]$sample.movement_bottom
            Assert-Condition (
                $movementRight -gt $movementLeft -and
                $movementBottom -gt $movementTop) (
                'Flight trace contains invalid movement bounds at modeled frame ' +
                [string]$sample.model_frame)
            $normalizedFlightX = [Math]::Max(
                0.0,
                [Math]::Min(
                    1.0,
                    (([double]$sample.screen_x - $movementLeft) /
                        ($movementRight - $movementLeft))))
        }
        if ($behavior -ceq 'groom') {
            [void]$groomingFrames.Add([long]$sample.model_frame)
            if ($null -ne $sample.grooming_substate) {
                [void]$groomingSubstates.Add([string]$sample.grooming_substate)
            }
            if ($previousBehavior -cne 'groom') { $groomingBouts++ }
        }

        if ($behavior -ceq 'walk' -and $previousBehavior -cne 'walk') {
            $currentBout = @{
                max_age_frames = [long]$sample.behavior_age_frames
                path_pixels = 0.0
                pairs = 0
                speed_sum = 0.0
                speed_count = 0
                interactive = $interactive
            }
        }
        elseif ($behavior -cne 'walk' -and $previousBehavior -ceq 'walk' -and
            $null -ne $currentBout) {
            Add-CompletedWalkBout -Bout $currentBout -Destination $walkBouts
            $currentBout = $null
        }

        if ($behavior -ceq 'flight' -and $previousBehavior -cne 'flight') {
            $currentFlightBout = @{
                max_age_frames = [long]$sample.behavior_age_frames
                path_pixels = 0.0
                pairs = 0
                speed_sum = 0.0
                speed_count = 0
                saccades = 0
                in_saccade = $false
                completed_to_landing = $false
                natural_motion = [bool]$sample.natural_flight_motion
                normalized_x_start = [double]$normalizedFlightX
                normalized_x_end = [double]$normalizedFlightX
                normalized_x_min = [double]$normalizedFlightX
                normalized_x_max = [double]$normalizedFlightX
                visited_left_tertile = [double]$normalizedFlightX -lt (1.0 / 3.0)
                visited_middle_tertile =
                    [double]$normalizedFlightX -ge (1.0 / 3.0) -and
                    [double]$normalizedFlightX -lt (2.0 / 3.0)
                visited_right_tertile = [double]$normalizedFlightX -ge (2.0 / 3.0)
            }
        }
        elseif ($behavior -cne 'flight' -and
            $previousBehavior -ceq 'flight' -and
            $null -ne $currentFlightBout) {
            $currentFlightBout.completed_to_landing = $behavior -ceq 'landing'
            Add-CompletedFlightBout `
                -Bout $currentFlightBout `
                -Destination $flightBouts
            $currentFlightBout = $null
        }

        if ($behavior -ceq 'walk' -and $null -ne $currentBout) {
            $currentBout.max_age_frames = [Math]::Max(
                [long]$currentBout.max_age_frames,
                [long]$sample.behavior_age_frames)
            if ($interactive) { $currentBout.interactive = $true }
            if (-not $interactive -and [long]$sample.behavior_age_frames -ge 1) {
                $currentBout.speed_sum += [double]$sample.speed_pixels_per_second
                $currentBout.speed_count++
            }
        }

        if ($behavior -ceq 'flight' -and $null -ne $currentFlightBout) {
            $currentFlightBout.max_age_frames = [Math]::Max(
                [long]$currentFlightBout.max_age_frames,
                [long]$sample.behavior_age_frames)
            $currentFlightBout.speed_sum +=
                [double]$sample.speed_pixels_per_second
            $currentFlightBout.speed_count++
            $currentFlightBout.natural_motion =
                [bool]$currentFlightBout.natural_motion -and
                [bool]$sample.natural_flight_motion
            $currentFlightBout.normalized_x_end = [double]$normalizedFlightX
            $currentFlightBout.normalized_x_min = [Math]::Min(
                [double]$currentFlightBout.normalized_x_min,
                [double]$normalizedFlightX)
            $currentFlightBout.normalized_x_max = [Math]::Max(
                [double]$currentFlightBout.normalized_x_max,
                [double]$normalizedFlightX)
            if ([double]$normalizedFlightX -lt (1.0 / 3.0)) {
                $currentFlightBout.visited_left_tertile = $true
            }
            elseif ([double]$normalizedFlightX -lt (2.0 / 3.0)) {
                $currentFlightBout.visited_middle_tertile = $true
            }
            else {
                $currentFlightBout.visited_right_tertile = $true
            }
        }

        if ($null -ne $previous -and
            $behavior -ceq 'walk' -and
            [string]$previous.behavior -ceq 'walk') {
            $pairInteractive = $interactive -or
                [bool]$previous.dragging -or
                [bool]$previous.cursor_hovered -or
                [bool]$previous.evidence_hold
            if (-not $pairInteractive) {
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
                if ($null -ne $currentBout) {
                    $currentBout.path_pixels += $distance
                    $currentBout.pairs++
                }
            }
        }
        if ($null -ne $previous -and
            $behavior -ceq 'flight' -and
            [string]$previous.behavior -ceq 'flight' -and
            $null -ne $currentFlightBout) {
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
            $currentFlightBout.path_pixels += $distance
            $currentFlightBout.pairs++
            $isSaccade = $headingDelta -ge 0.10
            if ($isSaccade -and -not [bool]$currentFlightBout.in_saccade) {
                $currentFlightBout.saccades++
            }
            $currentFlightBout.in_saccade = $isSaccade
        }
        $previous = $sample
        $previousBehavior = $behavior
    }
    # A walk still active at trace end is right-censored and intentionally omitted.

    $durationValues = [double[]]@($walkBouts | ForEach-Object {
        [double]$_.duration_seconds
    })
    $durations = [double[]]@($durationValues | Sort-Object)
    $distances = [double[]]@($walkBouts | ForEach-Object {
        [double]$_.path_pixels
    })
    $meanSpeeds = [double[]]@($walkBouts | ForEach-Object {
        [double]$_.mean_speed_pixels_per_second
    })
    $durationStats = Get-MeanAndCv -Values $durations
    $speedStats = Get-MeanAndCv -Values $meanSpeeds
    $shortCount = @($durations | Where-Object { $_ -le 1.0 }).Count
    $longCount = @($durations | Where-Object { $_ -ge 4.5 }).Count
    $shortFraction = if ($durations.Count -gt 0) {
        [double]$shortCount / [double]$durations.Count
    }
    else { 0.0 }
    $speedMinimum = if ($meanSpeeds.Count -gt 0) {
        [double]($meanSpeeds | Measure-Object -Minimum).Minimum
    }
    else { 0.0 }
    $speedMaximum = if ($meanSpeeds.Count -gt 0) {
        [double]($meanSpeeds | Measure-Object -Maximum).Maximum
    }
    else { 0.0 }
    $durationDistanceR2 = Get-SquaredCorrelation -X $durationValues -Y $distances

    $flightDurationValues = [double[]]@($flightBouts | ForEach-Object {
        [double]$_.duration_seconds
    })
    $flightDurations = [double[]]@($flightDurationValues | Sort-Object)
    $flightSpeeds = [double[]]@($flightBouts | ForEach-Object {
        [double]$_.mean_speed_pixels_per_second
    })
    $flightDurationStats = Get-MeanAndCv -Values $flightDurations
    $flightSpeedStats = Get-MeanAndCv -Values $flightSpeeds
    $flightSpeedMinimum = if ($flightSpeeds.Count -gt 0) {
        [double]($flightSpeeds | Measure-Object -Minimum).Minimum
    }
    else { 0.0 }
    $flightSpeedMaximum = if ($flightSpeeds.Count -gt 0) {
        [double]($flightSpeeds | Measure-Object -Maximum).Maximum
    }
    else { 0.0 }
    # Windows PowerShell 5.1 can return a measure object without a Sum
    # property when the input collection is empty under StrictMode. Accumulate
    # explicitly so the zero-bout regression has a portable numeric identity.
    $flightPath = 0.0
    $flightSaccades = 0
    foreach ($flightBout in $flightBouts) {
        $flightPath += [double]$flightBout.path_pixels
        $flightSaccades += [int]$flightBout.saccades
    }
    $flightDistinctDurations = @($flightDurations | Select-Object -Unique).Count
    $allFlightMotionNatural = $flightBouts.Count -gt 0 -and
        @($flightBouts | Where-Object { -not [bool]$_.natural_motion }).Count -eq 0
    $flightNormalizedXMinimum = 0.0
    $flightNormalizedXMaximum = 0.0
    $flightObservedHorizontalSpan = 0.0
    $flightLeftwardBouts = 0
    $flightRightwardBouts = 0
    $visitedLeftTertile = $false
    $visitedMiddleTertile = $false
    $visitedRightTertile = $false
    if ($flightBouts.Count -gt 0) {
        $minimumXMeasure = $flightBouts |
            Measure-Object -Property normalized_x_min -Minimum
        $maximumXMeasure = $flightBouts |
            Measure-Object -Property normalized_x_max -Maximum
        $flightNormalizedXMinimum = [double]$minimumXMeasure.Minimum
        $flightNormalizedXMaximum = [double]$maximumXMeasure.Maximum
        $flightObservedHorizontalSpan =
            $flightNormalizedXMaximum - $flightNormalizedXMinimum
        $flightLeftwardBouts = @($flightBouts | Where-Object {
            [double]$_.normalized_x_displacement -le -0.05
        }).Count
        $flightRightwardBouts = @($flightBouts | Where-Object {
            [double]$_.normalized_x_displacement -ge 0.05
        }).Count
        $visitedLeftTertile = @($flightBouts | Where-Object {
            [bool]$_.visited_left_tertile
        }).Count -gt 0
        $visitedMiddleTertile = @($flightBouts | Where-Object {
            [bool]$_.visited_middle_tertile
        }).Count -gt 0
        $visitedRightTertile = @($flightBouts | Where-Object {
            [bool]$_.visited_right_tertile
        }).Count -gt 0
    }
    $flightHorizontalTertilesVisited = @(@(
        $visitedLeftTertile,
        $visitedMiddleTertile,
        $visitedRightTertile
    ) | Where-Object { $_ }).Count

    $requiredCaptures = @(
        'groom-head-sweep.png',
        'groom-foreleg-rub.png',
        'groom-abdomen-brush.png',
        'groom-wing-clean.png'
    )
    $observedCaptures = @($requiredCaptures | Where-Object {
        Test-Path -LiteralPath (Join-Path $CaptureDirectory $_) -PathType Leaf
    })
    $requiredFlightCaptures = @(
        'flight-takeoff.png',
        'flight-early.png',
        'flight-maneuver.png',
        'flight-landing.png',
        'flight-touchdown.png'
    )
    $observedFlightCaptures = @($requiredFlightCaptures | Where-Object {
        Test-Path -LiteralPath (Join-Path $CaptureDirectory $_) -PathType Leaf
    })
    $stationaryRatio = if ($walkingPairs -gt 0) {
        [double]$stationaryRotationPairs / [double]$walkingPairs
    }
    else { 1.0 }
    $groomingSeconds = [double]$groomingFrames.Count * 0.033
    $passed = $walkingPath -ge 100.0 -and
        $translatedPairs -ge 50 -and
        $stationaryRatio -le 0.01 -and
        $walkBouts.Count -ge 20 -and
        $durationStats.cv -ge 0.55 -and
        $shortFraction -ge 0.25 -and
        $longCount -ge 1 -and
        $speedStats.cv -ge 0.18 -and
        ($speedMaximum - $speedMinimum) -ge 20.0 -and
        $durationDistanceR2 -le 0.98 -and
        $flightBouts.Count -ge 10 -and
        $flightDistinctDurations -ge 3 -and
        $flightDurationStats.cv -ge 0.25 -and
        $flightSpeedStats.cv -ge 0.08 -and
        ($flightSpeedMaximum - $flightSpeedMinimum) -ge 30.0 -and
        $flightPath -ge 500.0 -and
        $flightSaccades -ge 2 -and
        $flightHorizontalTertilesVisited -ge 2 -and
        $flightLeftwardBouts -ge 1 -and
        $flightRightwardBouts -ge 1 -and
        $allFlightMotionNatural -and
        $observedFlightCaptures.Count -eq $requiredFlightCaptures.Count -and
        $groomingBouts -ge 1 -and
        $groomingSeconds -ge 1.5 -and
        $observedCaptures.Count -eq $requiredCaptures.Count -and
        $groomingSubstates.Count -ge 4
    return [pscustomobject][ordered]@{
        status = if ($passed) { 'PASS' } else { 'FAIL' }
        unique_modeled_frames = $samples.Count
        autonomous_complete_walk_bouts = $walkBouts.Count
        walk_duration_min_seconds = [Math]::Round((Get-NearestRank -SortedValues $durations -Quantile 0.0), 3)
        walk_duration_p50_seconds = [Math]::Round((Get-NearestRank -SortedValues $durations -Quantile 0.5), 3)
        walk_duration_p90_seconds = [Math]::Round((Get-NearestRank -SortedValues $durations -Quantile 0.9), 3)
        walk_duration_max_seconds = [Math]::Round((Get-NearestRank -SortedValues $durations -Quantile 1.0), 3)
        walk_duration_mean_seconds = [Math]::Round($durationStats.mean, 3)
        walk_duration_cv = [Math]::Round($durationStats.cv, 6)
        walk_bouts_at_or_below_one_second = $shortCount
        walk_short_bout_fraction = [Math]::Round($shortFraction, 6)
        walk_bouts_at_or_above_4_5_seconds = $longCount
        walk_mean_speed_min_pixels_per_second = [Math]::Round($speedMinimum, 3)
        walk_mean_speed_max_pixels_per_second = [Math]::Round($speedMaximum, 3)
        walk_mean_speed_cv = [Math]::Round($speedStats.cv, 6)
        walk_duration_distance_r_squared = [Math]::Round($durationDistanceR2, 6)
        walking_sample_pairs = $walkingPairs
        translated_walking_pairs = $translatedPairs
        walking_path_pixels = [Math]::Round($walkingPath, 3)
        stationary_rotation_pairs = $stationaryRotationPairs
        stationary_rotation_ratio = [Math]::Round($stationaryRatio, 6)
        complete_flight_bouts = $flightBouts.Count
        flight_distinct_durations = $flightDistinctDurations
        flight_duration_min_seconds = [Math]::Round((Get-NearestRank `
            -SortedValues $flightDurations -Quantile 0.0), 3)
        flight_duration_p50_seconds = [Math]::Round((Get-NearestRank `
            -SortedValues $flightDurations -Quantile 0.5), 3)
        flight_duration_max_seconds = [Math]::Round((Get-NearestRank `
            -SortedValues $flightDurations -Quantile 1.0), 3)
        flight_duration_cv = [Math]::Round($flightDurationStats.cv, 6)
        flight_mean_speed_min_pixels_per_second =
            [Math]::Round($flightSpeedMinimum, 3)
        flight_mean_speed_max_pixels_per_second =
            [Math]::Round($flightSpeedMaximum, 3)
        flight_mean_speed_cv = [Math]::Round($flightSpeedStats.cv, 6)
        flight_path_pixels = [Math]::Round($flightPath, 3)
        flight_saccades = $flightSaccades
        flight_normalized_x_min = [Math]::Round($flightNormalizedXMinimum, 6)
        flight_normalized_x_max = [Math]::Round($flightNormalizedXMaximum, 6)
        flight_observed_horizontal_span_fraction =
            [Math]::Round($flightObservedHorizontalSpan, 6)
        flight_horizontal_tertiles_visited = $flightHorizontalTertilesVisited
        flight_leftward_displacement_bouts = $flightLeftwardBouts
        flight_rightward_displacement_bouts = $flightRightwardBouts
        natural_flight_motion_all_bouts = $allFlightMotionNatural
        required_flight_captures = $requiredFlightCaptures
        observed_flight_captures = $observedFlightCaptures
        grooming_bouts = $groomingBouts
        grooming_frames = $groomingFrames.Count
        grooming_seconds = [Math]::Round($groomingSeconds, 3)
        grooming_substates = @($groomingSubstates | Sort-Object)
        required_grooming_captures = $requiredCaptures
        observed_grooming_captures = $observedCaptures
        gate = 'ground gates preserved; >= 10 complete flight bouts; >= 3 flight durations; flight duration CV >= 0.25; flight speed CV >= 0.08 and range >= 30 px/s; >= 500 px flight path; >= 2 rapid saccades; anti-confinement diagnostic visits at least two horizontal tertiles and includes both leftward and rightward bouts; no screen-coverage target or threshold; natural-flight flag and five phase captures present; grooming sequence and captures complete'
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
    'This launches the isolated N4.1-D exploratory-flight candidate for ten minutes.' +
        [Environment]::NewLine + [Environment]::NewLine +
        'Observe the first 30 seconds, then the final five-minute window.' +
        ' Watch specifically for varied walking, grooming, and visibly different' +
        ' flight lengths, speeds, and paths.' +
        [Environment]::NewLine + [Environment]::NewLine +
        'Trigger at least twelve separate flights across the early and late windows' +
        ' with Ctrl+Alt+L. After each trigger, move the pointer away and allow the' +
        ' complete takeoff, flight, maneuver, landing, and touchdown to finish.' +
        ' Keep the triggers separated so each flight is a complete bout.' +
        [Environment]::NewLine + [Environment]::NewLine +
        'The review does not deploy anything, change shortcuts, or write to AppData.' +
        ' Press Cancel if you cannot observe the full session.',
    'MechoFly N4.1-D exploratory-flight visual acceptance',
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
    Assert-Condition ([string]$launchReceipt.active_profile -ceq 'n41-b-natural-flight') 'Candidate did not activate n41-b-natural-flight.'
    Assert-Condition ([string]$launchReceipt.canonical_default_profile -ceq 'n4') 'Canonical default is not N4.'
    Assert-Condition ([string]$launchReceipt.parameter_sha256 -ceq $ExpectedParameterSha256) 'N4.1-D parameter identity mismatch.'
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
        $heldWindow = [MechoFly.N41VisualReview.WindowProbe]::Current(
            [int64]$petWindow.handle)
        if ($null -ne $heldWindow) {
            Assert-Condition (
                [MechoFly.N41VisualReview.WindowProbe]::SetEvidenceHold(
                    [int64]$heldWindow.handle,
                    $false)) 'Could not release the final evidence hold.'
        }
        Stop-ReviewCandidate -Process $process
        $objectiveMetrics = Get-NaturalMotionMetrics `
            -TracePath $tracePath `
            -CaptureDirectory $captureDirectory
        Write-Host ('NATURAL_MOTION_GATE=' + $objectiveMetrics.status +
            ' walk_bouts=' + [string]$objectiveMetrics.autonomous_complete_walk_bouts +
            ' walk_duration_cv=' + [string]$objectiveMetrics.walk_duration_cv +
            ' walk_speed_cv=' + [string]$objectiveMetrics.walk_mean_speed_cv +
            ' walking_path_pixels=' + [string]$objectiveMetrics.walking_path_pixels +
            ' flight_bouts=' + [string]$objectiveMetrics.complete_flight_bouts +
            ' flight_duration_cv=' + [string]$objectiveMetrics.flight_duration_cv +
            ' flight_speed_cv=' + [string]$objectiveMetrics.flight_mean_speed_cv +
            ' flight_saccades=' + [string]$objectiveMetrics.flight_saccades +
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
                    'Accept this exact N4.1-D exploratory-flight candidate binary only for the next guarded step?' +
                    ' This still does not authorize deployment or shortcut changes.',
                'MechoFly N4.1-D explicit visual-acceptance decision',
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
        schema_version = 5
        status = 'PASS'
        classification = 'single_owner_formative_uncued_exploratory_flight_review'
        candidate_profile = 'n41-b-natural-flight'
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
        Stop-ReviewCandidate -Process $process
    }
}
