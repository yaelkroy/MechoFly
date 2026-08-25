#requires -version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
$Forbidden = @(
    ('Desktop' + 'Fly'),
    ('desktop' + 'fly'),
    ('DESKTOP' + 'FLY')
)
$TextExtensions = @('.cs', '.ps1', '.cmd', '.md', '.yml', '.yaml', '.json', '.txt')
$Failures = New-Object System.Collections.Generic.List[string]
Get-ChildItem -LiteralPath $Root -Recurse -File | ForEach-Object {
    if ($_.FullName.IndexOf((Join-Path $Root '.git'), [StringComparison]::OrdinalIgnoreCase) -eq 0) {
        return
    }
    $RelativePath = $_.FullName.Substring($Root.Length).TrimStart('\', '/')
    foreach ($Token in $Forbidden) {
        if ($RelativePath.IndexOf($Token, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            $Failures.Add('path: ' + $RelativePath)
        }
    }
    if ($TextExtensions -contains $_.Extension.ToLowerInvariant()) {
        $Text = [System.IO.File]::ReadAllText($_.FullName)
        foreach ($Token in $Forbidden) {
            if ($Text.IndexOf($Token, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
                $Failures.Add('content: ' + $_.FullName + ' token=' + $Token)
            }
        }
    }
}
if ($Failures.Count -gt 0) {
    $Failures | ForEach-Object { Write-Error $_ }
    throw 'Tree identity verification failed.'
}
Write-Host 'MECHOFLY_TREE_IDENTITY=PASS'
