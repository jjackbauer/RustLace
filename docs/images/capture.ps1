# Capture the documentation screenshots from the real Windows workbench.
#
# Every PNG under docs/images is produced by this script running the release build of
# lovelace-desktop (target-release/release/lovelace-desktop.exe) on Windows. The window is
# captured with PrintWindow(PW_RENDERFULLCONTENT), which works whether or not the desktop session
# is locked and never depends on the window being unoccluded. UI Automation is used to read the
# accessible tree and to invoke controls; text is set through the editor's ValuePattern, so no
# synthetic keystrokes are needed except where a scenario says otherwise.
#
# Usage (Windows PowerShell 5.1 or pwsh 7, ASCII-only source):
#   powershell -NoProfile -ExecutionPolicy Bypass -File docs/images/capture.ps1 -Scenario welcome-dark
#   powershell -NoProfile -ExecutionPolicy Bypass -File docs/images/capture.ps1 -Scenario all
#
# The application's APPDATA is redirected outside the repository (default: %TEMP%\lovelace-docs-capture),
# so plot files, settings and recovery records never land beside the sources.
[CmdletBinding()]
param(
    [ValidateSet('welcome-dark', 'welcome-light', 'editor-and-results', 'plot', 'symbolic-inspector',
        'find-and-replace', 'signature-help', 'log-flood', 'layout-compact-200', 'progress-and-cancel', 'all')]
    [string]$Scenario = 'all',
    [string]$Exe = '',
    [string]$OutDir = '',
    [string]$WorkRoot = '',
    [int]$Width = 1600,
    [int]$Height = 1000,
    [int]$MinBytes = 20480,
    [int]$MaxBytes = 358400
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing, System.Windows.Forms, UIAutomationClient, UIAutomationTypes

# Make this process per-monitor DPI aware before any window is touched, so every coordinate in
# this script is a physical pixel and the capture is never silently scaled by Windows.
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class DpiAwareness {
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr context);
  [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
}
"@
try { [void][DpiAwareness]::SetProcessDpiAwarenessContext([IntPtr](-4)) } catch { }
try { [void][DpiAwareness]::SetThreadDpiAwarenessContext([IntPtr](-4)) } catch { }

if ($OutDir -eq '') { $OutDir = Join-Path (Split-Path -Parent $PSScriptRoot) 'images' }
if (-not (Test-Path -LiteralPath $OutDir)) { New-Item -ItemType Directory -Force -Path $OutDir | Out-Null }
if ($WorkRoot -eq '') { $WorkRoot = Join-Path $env:TEMP 'lovelace-docs-capture' }
New-Item -ItemType Directory -Force -Path $WorkRoot | Out-Null
if ($Exe -eq '') {
    $candidate = Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'target-release\release\lovelace-desktop.exe'
    if (Test-Path -LiteralPath $candidate) { $Exe = $candidate }
    else { throw 'pass -Exe: the release binary was not found at target-release/release/lovelace-desktop.exe' }
}
if (-not (Test-Path -LiteralPath $Exe)) { throw "exe not found: $Exe" }

Add-Type -TypeDefinition @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public struct CapRect { public int Left, Top, Right, Bottom; }
public static class CapNative {
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out CapRect r);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern bool RedrawWindow(IntPtr h, IntPtr rect, IntPtr region, uint flags);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int max);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
  public delegate bool EnumProc(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc call, IntPtr p);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  public static IntPtr WindowOf(uint pid, string part) {
    // Pick the LARGEST visible window of the process whose title contains `part`. A workbench
    // process can own small auxiliary windows (tooltips, IME candidates) that also carry the
    // application name; the main viewport is the big one.
    IntPtr best = IntPtr.Zero;
    long bestArea = 0;
    EnumWindows((h, p) => {
      uint owner; GetWindowThreadProcessId(h, out owner);
      if (owner != pid || !IsWindowVisible(h)) { return true; }
      var sb = new StringBuilder(512); GetWindowText(h, sb, 512);
      if (sb.ToString().IndexOf(part, StringComparison.OrdinalIgnoreCase) < 0) { return true; }
      CapRect r; if (!GetWindowRect(h, out r)) { return true; }
      long area = (long)(r.Right - r.Left) * (r.Bottom - r.Top);
      if (area > bestArea) { bestArea = area; best = h; }
      return true;
    }, IntPtr.Zero);
    return best;
  }
}
"@

$script:LogPath = Join-Path $WorkRoot 'capture.log'
$script:Scale = 1.0
function Get-Scaled([int]$value) { return [int][Math]::Round($value * $script:Scale) }
function Write-Capture([string]$line) {
    $stamp = (Get-Date).ToString('HH:mm:ss')
    Write-Host "$stamp $line"
    Add-Content -LiteralPath $script:LogPath -Value "$stamp $line" -Encoding UTF8
}

# ---------------------------------------------------------------------------------------------
# UI Automation helpers
# ---------------------------------------------------------------------------------------------
function Get-LiveHandle([IntPtr]$h) {
    # eframe can recreate the OS window when its size or DPI changes, so every UIA read and every
    # capture re-resolves the handle from the process instead of trusting the one from launch.
    if ($script:AppPid) {
        $candidate = [CapNative]::WindowOf([uint32]$script:AppPid, 'Lovelace Desktop')
        if ($candidate -ne [IntPtr]::Zero) { return $candidate }
    }
    return $h
}

function Get-Root([IntPtr]$h) {
    return [System.Windows.Automation.AutomationElement]::FromHandle((Get-LiveHandle $h))
}

function Get-All([IntPtr]$h) {
    $root = Get-Root $h
    return $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
}

function Get-Names([IntPtr]$h) {
    # Scan Text and Button nodes only: a symbolic result can put thousands of nodes in the raw
    # tree, and the two roles carry every string this script waits for.
    $names = New-Object System.Collections.Generic.List[string]
    $root = Get-Root $h
    foreach ($type in @([System.Windows.Automation.ControlType]::Text, [System.Windows.Automation.ControlType]::Button)) {
        $condition = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty, $type)
        try {
            foreach ($element in $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $condition)) {
                if ($element.Current.Name) { $names.Add($element.Current.Name) }
            }
        } catch { }
    }
    return $names
}

function Wait-Name([IntPtr]$h, [string]$needle, [int]$TimeoutSec) {
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        foreach ($name in (Get-Names $h)) { if ($name -like "*$needle*") { return $name } }
        Start-Sleep -Milliseconds 150
    }
    return $null
}

function Find-Element([IntPtr]$h, [string]$namePrefix, [string]$controlType) {
    $all = Get-All $h
    foreach ($element in $all) {
        try {
            $current = $element.Current
            $type = $current.ControlType.ProgrammaticName.Replace('ControlType.', '')
            if ($controlType -ne '' -and $type -ne $controlType) { continue }
            if ($current.Name -like "$namePrefix*") { return $element }
        } catch { }
    }
    return $null
}

function Invoke-Control([IntPtr]$h, [string]$name) {
    $element = Find-Element $h $name 'Button'
    if (-not $element) { throw "no Button named '$name'" }
    try {
        $pattern = $element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
        $pattern.Invoke()
        Write-Capture "invoked '$name' by InvokePattern"
        return
    } catch { }
    # Some painted toggles expose the button role without the Invoke pattern; click its centre.
    $rect = $element.Current.BoundingRectangle
    if ($rect.Width -le 0 -or $rect.Height -le 0) { throw "'$name' supports no Invoke pattern and has no rectangle" }
    [void][CapNative]::SetForegroundWindow($h)
    Start-Sleep -Milliseconds 120
    [void][CapNative]::SetCursorPos([int]($rect.X + $rect.Width / 2), [int]($rect.Y + $rect.Height / 2))
    Start-Sleep -Milliseconds 60
    [CapNative]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [CapNative]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
    Write-Capture "clicked '$name' at $([int]($rect.X + $rect.Width / 2)),$([int]($rect.Y + $rect.Height / 2))"
}

function Send-Chord([IntPtr]$h, [string]$chord) {
    [void][CapNative]::SetForegroundWindow($h)
    Start-Sleep -Milliseconds 200
    [System.Windows.Forms.SendKeys]::SendWait($chord)
    Write-Capture "sent keys $chord"
}

function Focus-Element([IntPtr]$h, [string]$namePrefix, [string]$controlType) {
    $element = Find-Element $h $namePrefix $controlType
    if (-not $element) { throw "no $controlType named '$namePrefix*'" }
    $element.SetFocus()
    Write-Capture "focused '$($element.Current.Name)'"
}

function ConvertTo-SendKeysText([string]$text) {
    # SendKeys treats these as syntax; a literal character is sent inside braces.
    $out = ''
    foreach ($ch in $text.ToCharArray()) {
        if ('+^%~(){}[]'.IndexOf($ch) -ge 0) { $out = $out + '{' + $ch + '}' } else { $out = $out + $ch }
    }
    return $out
}

function Send-Text([IntPtr]$h, [string]$text) {
    [void][CapNative]::SetForegroundWindow($h)
    Start-Sleep -Milliseconds 250
    [System.Windows.Forms.SendKeys]::SendWait((ConvertTo-SendKeysText $text))
    Write-Capture "typed '$text'"
}

function Set-EditorText([IntPtr]$h, [string]$text) {
    $edit = Find-Element $h 'Script editor' 'Edit'
    if (-not $edit) { throw 'no Script editor Edit' }
    $pattern = $edit.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
    $pattern.SetValue($text)
    Write-Capture "set editor text chars=$($text.Length)"
}

function Set-Value([IntPtr]$h, [string]$namePrefix, [string]$value) {
    $edit = Find-Element $h $namePrefix 'Edit'
    if (-not $edit) { throw "no Edit named '$namePrefix*'" }
    $pattern = $edit.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
    $pattern.SetValue($value)
    Write-Capture "set '$($edit.Current.Name)' = '$value'"
}

function Get-StatusText([IntPtr]$h) {
    $root = Get-Root $h
    $barCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::StatusBar)
    try {
        $bar = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $barCondition)
        if ($bar) {
            $textCondition = New-Object System.Windows.Automation.PropertyCondition(
                [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
                [System.Windows.Automation.ControlType]::Text)
            foreach ($text in $bar.FindAll([System.Windows.Automation.TreeScope]::Descendants, $textCondition)) {
                if ($text.Current.Name -match '\|\s*run ') { return $text.Current.Name }
            }
        }
    } catch {
        if (-not $script:StatusErrorLogged) {
            Write-Capture "uia status read failed: $($_.Exception.Message)"
            $script:StatusErrorLogged = $true
        }
    }
    return ''
}

function Wait-Status([IntPtr]$h, [string]$needle, [int]$TimeoutSec) {
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $last = ''
    $ticks = 0
    while ((Get-Date) -lt $deadline) {
        $last = Get-StatusText $h
        if ($last -like "*$needle*") { return $last }
        $ticks++
        if ($ticks % 40 -eq 0) {
            $live = Get-LiveHandle $h
            $size = Get-WindowSize $live
            Write-Capture "waiting for '$needle': hwnd=$h live=$live rect=$($size.W)x$($size.H) status='$last'"
        }
        Start-Sleep -Milliseconds 150
    }
    return $last
}

# ---------------------------------------------------------------------------------------------
# Window helpers
# ---------------------------------------------------------------------------------------------
function Start-App([string[]]$extraArgs, [string]$appDataName, [hashtable]$environment) {
    $appData = Join-Path $WorkRoot ("appdata\" + $appDataName)
    if (Test-Path -LiteralPath $appData) { Remove-Item -LiteralPath $appData -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $appData | Out-Null
    $tmp = Join-Path $WorkRoot ("tmp\" + $appDataName)
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null
    $env:APPDATA = $appData
    $env:TMP = $tmp
    $env:TEMP = $tmp
    $env:LOVELACE_LOG_FLOOD = ''
    $env:LOVELACE_LOG_START_SCROLLED = ''
    if ($environment) { foreach ($key in $environment.Keys) { Set-Item -Path ("env:" + $key) -Value $environment[$key] } }
    if ($extraArgs -and $extraArgs.Count -gt 0) {
        $p = Start-Process -FilePath $Exe -ArgumentList $extraArgs -PassThru
    } else {
        $p = Start-Process -FilePath $Exe -PassThru
    }
    Write-Capture "launch pid=$($p.Id) appdata=$appData"
    $deadline = (Get-Date).AddSeconds(45)
    $h = [IntPtr]::Zero
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 200
        $h = [CapNative]::WindowOf([uint32]$p.Id, 'Lovelace Desktop')
        if ($h -ne [IntPtr]::Zero) { break }
    }
    if ($h -eq [IntPtr]::Zero) { throw 'the workbench window never appeared' }
    if ([CapNative]::IsIconic($h)) { [void][CapNative]::ShowWindow($h, 9) }
    Start-Sleep -Milliseconds 400
    # A stable UIA tree: at least 30 descendants and a StatusBar with a run state.
    $deadline = (Get-Date).AddSeconds(30)
    while ((Get-Date) -lt $deadline) {
        try {
            if ((Get-All $h).Count -ge 30 -and (Get-StatusText $h) -ne '') { break }
        } catch { }
        Start-Sleep -Milliseconds 200
    }
    # Logical size in device-independent pixels; the window is a physical-pixel window, so it is
    # sized by the window's own DPI scale (1.0 on a 100% display, 3.0 on a 300% one).
    $dpi = [DpiAwareness]::GetDpiForWindow($h)
    if ($dpi -ge 96) { $script:Scale = [double]$dpi / 96.0 }
    Write-Capture "dpi=$dpi scale=$($script:Scale)"
    $physicalW = Get-Scaled $Width
    $physicalH = Get-Scaled $Height
    [void][CapNative]::SetWindowPos($h, [IntPtr]::Zero, 20, 20, $physicalW, $physicalH, 0x0040)
    [void][CapNative]::BringWindowToTop($h)
    [void][CapNative]::SetForegroundWindow($h)
    Start-Sleep -Milliseconds 900
    # eframe can restore or maximize the window after its first pass; settle it here once so every
    # scenario starts from the same geometry.
    [void](Ensure-WindowSize $h $physicalW $physicalH 6)
    $script:AppPid = $p.Id
    return @{ Process = $p; Handle = (Get-LiveHandle $h) }
}

function Get-WindowSize([IntPtr]$h) {
    $rect = New-Object CapRect
    [void][CapNative]::GetWindowRect($h, [ref]$rect)
    return @{ W = [int]($rect.Right - $rect.Left); H = [int]($rect.Bottom - $rect.Top) }
}

function Test-WindowFraming([IntPtr]$h, [int]$targetW, [int]$targetH) {
    # The window only has to be *the right shape*: this host scales the logical size by its DPI
    # factor, so a 1600x1000 layout can be reported as 4800x3000 physical pixels. The frame is
    # captured at whatever size is reported and resampled to the requested logical size.
    $size = Get-WindowSize $h
    if ($size.W -lt [int]($targetW * 0.9) -or $size.H -lt [int]($targetH * 0.9)) { return $false }
    $targetAspect = $targetW / $targetH
    $aspect = $size.W / $size.H
    if ([Math]::Abs($aspect - $targetAspect) -gt 0.08) { return $false }
    return $true
}

function Ensure-WindowSize([IntPtr]$h, [int]$targetW, [int]$targetH, [int]$Tries) {
    for ($i = 0; $i -lt $Tries; $i++) {
        if (Test-WindowFraming $h $targetW $targetH) { return $true }
        # eframe restores or maximizes the window on its own schedule during the first seconds;
        # un-maximize and re-assert the requested size, then look again.
        [void][CapNative]::ShowWindow($h, 9)
        Start-Sleep -Milliseconds 200
        [void][CapNative]::SetWindowPos($h, [IntPtr]::Zero, 20, 20, $targetW, $targetH, 0x0040)
        Start-Sleep -Milliseconds 800
    }
    return (Test-WindowFraming $h $targetW $targetH)
}

function Save-Frame([IntPtr]$h, [string]$name, [int]$targetW, [int]$targetH) {
    $path = Join-Path $OutDir ($name + '.png')
    if (-not $targetW) { $targetW = $Width }
    if (-not $targetH) { $targetH = $Height }
    $physicalW = Get-Scaled $targetW
    $physicalH = Get-Scaled $targetH
    $h = Get-LiveHandle $h
    [void](Ensure-WindowSize $h $physicalW $physicalH 6)
    $bytes = 0
    $captured = $null
    for ($attempt = 1; $attempt -le 4; $attempt++) {
        [void][CapNative]::RedrawWindow($h, [IntPtr]::Zero, [IntPtr]::Zero, 0x0185)
        Start-Sleep -Milliseconds (350 + 250 * $attempt)
        $size = Get-WindowSize $h
        $w = $size.W
        $ht = $size.H
        if ($w -lt 400 -or $ht -lt 300) {
            [void](Ensure-WindowSize $h $physicalW $physicalH 2)
            continue
        }
        $bmp = New-Object System.Drawing.Bitmap($w, $ht)
        $graphics = [System.Drawing.Graphics]::FromImage($bmp)
        $dc = $graphics.GetHdc()
        $ok = [CapNative]::PrintWindow($h, $dc, 2)
        $graphics.ReleaseHdc($dc)
        $graphics.Dispose()
        $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
        $bmp.Dispose()
        $bytes = (Get-Item -LiteralPath $path).Length
        $captured = @{ W = $w; H = $ht }
        Write-Capture "printwindow name=$name attempt=$attempt ok=$ok rect=${w}x${ht} bytes=$bytes"
        if ($ok -and $bytes -ge $MinBytes) { break }
        [void](Ensure-WindowSize $h $physicalW $physicalH 2)
        Start-Sleep -Milliseconds 600
    }
    if ($bytes -lt $MinBytes) { throw "capture $name is only $bytes bytes; refusing an empty frame" }
    # Frame the page: every PNG is the requested logical size (1600x1000, or 900x600 for the
    # compact layout). A frame captured at a different size is resampled, never cropped.
    $source = [System.Drawing.Image]::FromFile($path)
    if ($source.Width -ne $targetW -or $source.Height -ne $targetH) {
        $scaled = New-Object System.Drawing.Bitmap($targetW, $targetH)
        $g = [System.Drawing.Graphics]::FromImage($scaled)
        $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $g.DrawImage($source, 0, 0, $targetW, $targetH)
        $g.Dispose()
        $source.Dispose()
        $scaled.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
        $scaled.Dispose()
        Write-Capture "resampled name=$name from $($captured.W)x$($captured.H) to ${targetW}x${targetH}"
    } else {
        $source.Dispose()
    }
    $bytes = (Get-Item -LiteralPath $path).Length
    if ($bytes -gt $MaxBytes) {
        $scaledW = [int]($targetW * 0.75)
        $scaledH = [int]($targetH * 0.75)
        $source = [System.Drawing.Image]::FromFile($path)
        $scaled = New-Object System.Drawing.Bitmap($scaledW, $scaledH)
        $g = [System.Drawing.Graphics]::FromImage($scaled)
        $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $g.DrawImage($source, 0, 0, $scaledW, $scaledH)
        $g.Dispose()
        $source.Dispose()
        $scaled.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
        $scaled.Dispose()
        $bytes = (Get-Item -LiteralPath $path).Length
        Write-Capture "shrunk name=$name to ${scaledW}x${scaledH} bytes=$bytes"
    }
    $sha = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
    $size = "${targetW} x ${targetH}"
    Write-Capture "saved name=$name bytes=$bytes sha256=$sha"
    return @{ Name = $name; Path = $path; Bytes = $bytes; Sha256 = $sha; Size = $size }
}

function Close-App($app) {
    $p = $app.Process
    try { [void]$p.CloseMainWindow() } catch { }
    Start-Sleep -Milliseconds 900
    if (-not $p.HasExited) { try { $p.Kill() } catch { } }
    Start-Sleep -Milliseconds 300
}

# ---------------------------------------------------------------------------------------------
# Scenario fixtures
# ---------------------------------------------------------------------------------------------
function Write-Fixture([string]$name, [string[]]$lines) {
    $dir = Join-Path $WorkRoot 'scripts'
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $path = Join-Path $dir $name
    [System.IO.File]::WriteAllLines($path, $lines, (New-Object System.Text.UTF8Encoding($false)))
    return $path
}

function Write-Settings([string]$appDataName, [hashtable]$values) {
    $dir = Join-Path (Join-Path $WorkRoot 'appdata') $appDataName
    $appDir = Join-Path $dir 'Lovelace'
    New-Item -ItemType Directory -Force -Path $appDir | Out-Null
    $pairs = @()
    foreach ($key in $values.Keys) {
        $value = $values[$key]
        if ($value -is [bool]) { $json = $(if ($value) { 'true' } else { 'false' }) }
        elseif ($value -is [int]) { $json = "$value" }
        else { $json = '"' + $value + '"' }
        $pairs += ('"' + $key + '": ' + $json)
    }
    $json = '{' + ($pairs -join ', ') + '}'
    [System.IO.File]::WriteAllText((Join-Path $appDir 'desktop.json'), $json, (New-Object System.Text.UTF8Encoding($false)))
    Write-Capture "settings $appDataName = $json"
}

function Reset-AppData([string]$appDataName) {
    $dir = Join-Path (Join-Path $WorkRoot 'appdata') $appDataName
    if (Test-Path -LiteralPath $dir) { Remove-Item -LiteralPath $dir -Recurse -Force }
}

# ---------------------------------------------------------------------------------------------
# Scenarios
# ---------------------------------------------------------------------------------------------
$results = New-Object System.Collections.Generic.List[hashtable]

function Invoke-Scenario([string]$name) {
    Write-Capture "=== scenario $name ==="
    switch ($name) {
        'welcome-dark' {
            Reset-AppData 'welcome-dark'
            $app = Start-App @() 'welcome-dark' @{}
            Start-Sleep -Milliseconds 1200
            $results.Add((Save-Frame $app.Handle 'welcome-dark'))
            Close-App $app
        }
        'welcome-light' {
            Reset-AppData 'welcome-light'
            $app = Start-App @() 'welcome-light' @{}
            $toggle = Find-Element $app.Handle 'Theme:' 'Button'
            if (-not $toggle) { throw 'no theme toggle button' }
            Invoke-Control $app.Handle $toggle.Current.Name
            $light = Wait-Name $app.Handle 'Theme: Light' 10
            if (-not $light) { throw 'the theme toggle did not switch to the light token set' }
            Start-Sleep -Milliseconds 900
            $results.Add((Save-Frame $app.Handle 'welcome-light'))
            Close-App $app
        }
        'editor-and-results' {
            Reset-AppData 'editor-and-results'
            $script = Write-Fixture 'squares.lv' @(
                '# A short script: one function, exact integers, a matrix.',
                'func square(k) { k * k }',
                'n = 20',
                'squares = 1..n',
                'sum_squares = sum(squares ^ 2)',
                'm = [[1, 2], [3, 4]]',
                'det_m = det(m)',
                'inv_m = inv(m)',
                'sum_squares'
            )
            $app = Start-App @('"' + $script + '"') 'editor-and-results' @{}
            Invoke-Control $app.Handle 'Run'
            $status = Wait-Status $app.Handle 'run Finished' 60
            Write-Capture "status=$status"
            if ($status -notlike '*run Finished*') { throw "the run never finished: $status" }
            $vars = Wait-Name $app.Handle 'sum_squares' 10
            if (-not $vars) { throw 'the variables table never listed sum_squares' }
            Start-Sleep -Milliseconds 700
            $results.Add((Save-Frame $app.Handle 'editor-and-results'))
            Close-App $app
        }
        'plot' {
            Reset-AppData 'plot'
            $script = Write-Fixture 'plot.lv' @(
                '# Plot: a sampled parabola with a title and axis labels.',
                'x = 1..24',
                'y = (x - 12) ^ 2',
                'plot(x, y, "squares: (x - 12)^2")'
            )
            $app = Start-App @('"' + $script + '"') 'plot' @{}
            Invoke-Control $app.Handle 'Run'
            $status = Wait-Status $app.Handle 'run Finished' 60
            if ($status -notlike '*run Finished*') { throw "the run never finished: $status" }
            # Rasterising a 4800x3000 frame takes a moment on this display; the pane publishes its
            # provenance line when the texture is ready, and the capture waits it out.
            $canvas = Wait-Name $app.Handle 'SVG plot canvas' 25
            if ($canvas) { Write-Capture "plot canvas: $canvas" } else { Write-Capture 'plot canvas not in the tree yet; capturing the settled pane' }
            Start-Sleep -Milliseconds 2500
            $results.Add((Save-Frame $app.Handle 'plot'))
            Close-App $app
        }
        'symbolic-inspector' {
            Reset-AppData 'symbolic-inspector'
            $script = Write-Fixture 'symbolic.lv' @(
                '# Symbolic: differentiate and simplify an expression.',
                'x = symbol("x")',
                'expr = x^3 - 3*x + 1',
                'derivative = diff(expr, x)',
                'collected = simplify(derivative)',
                'collected'
            )
            $app = Start-App @('"' + $script + '"') 'symbolic-inspector' @{}
            Invoke-Control $app.Handle 'Run'
            # A symbolic result makes the accessible tree very large, and UIA reads of it become
            # unreliable; the run itself takes milliseconds, so wait out its settling time and
            # capture the committed state (the pane header in the frame states the outcome).
            Start-Sleep -Milliseconds 2500
            $results.Add((Save-Frame $app.Handle 'symbolic-inspector'))
            Close-App $app
        }
        'find-and-replace' {
            Reset-AppData 'find-and-replace'
            $script = Write-Fixture 'find.lv' @(
                '# Find: the same identifier appears three times.',
                'sum_a = sum(1..10)',
                'sum_b = sum(1..20)',
                'sum_c = sum_a + sum_b',
                'total = sum([sum_a, sum_b, sum_c])'
            )
            $app = Start-App @('"' + $script + '"') 'find-and-replace' @{}
            Invoke-Control $app.Handle 'Find...'
            Start-Sleep -Milliseconds 500
            Set-Value $app.Handle 'Find' 'sum'
            $count = Wait-Name $app.Handle '' 1
            Start-Sleep -Milliseconds 900
            $results.Add((Save-Frame $app.Handle 'find-and-replace'))
            Close-App $app
        }
        'signature-help' {
            Reset-AppData 'signature-help'
            $app = Start-App @() 'signature-help' @{}
            # Signature help opens on real editing input, so click into the editor and type the
            # call prefix rather than setting the buffer's value.
            Focus-Element $app.Handle 'Script editor' 'Edit'
            Invoke-Control $app.Handle 'New'
            Start-Sleep -Milliseconds 700
            Focus-Element $app.Handle 'Script editor' 'Edit'
            Send-Text $app.Handle 'matmul('
            $signature = Wait-Name $app.Handle 'Signature:' 12
            if (-not $signature) { $signature = Wait-Name $app.Handle 'matmul' 5 }
            if (-not $signature) { throw 'the editor published no signature help' }
            Write-Capture "signature='$signature'"
            Start-Sleep -Milliseconds 600
            $results.Add((Save-Frame $app.Handle 'signature-help'))
            Close-App $app
        }
        'log-flood' {
            Reset-AppData 'log-flood'
            $app = Start-App @() 'log-flood' @{ 'LOVELACE_LOG_FLOOD' = '4000' }
            $dropped = Wait-Name $app.Handle 'earlier records' 20
            if (-not $dropped) { $dropped = Wait-Name $app.Handle 'records' 5 }
            Start-Sleep -Milliseconds 1200
            $results.Add((Save-Frame $app.Handle 'log-flood'))
            Close-App $app
        }
        'layout-compact-200' {
            Reset-AppData 'layout-compact-200'
            Write-Settings 'layout-compact-200' @{ 'schema_version' = 1; 'ui_scale_percent' = 200 }
            $app = Start-App @() 'layout-compact-200' @{}
            Start-Sleep -Milliseconds 1400
            $results.Add((Save-Frame $app.Handle 'layout-compact-200' 900 600))
            Close-App $app
        }
        'progress-and-cancel' {
            Reset-AppData 'progress-and-cancel'
            $script = Write-Fixture 'slow.lv' @(
                '# Three square roots at high precision: seconds of exact big-integer work.',
                'setprecision(90000)',
                'a = sqrt(2)',
                'b = sqrt(3)',
                'c = sqrt(5)',
                'a + b + c'
            )
            $app = Start-App @('"' + $script + '"') 'progress-and-cancel' @{}
            Invoke-Control $app.Handle 'Run'
            $running = Wait-Status $app.Handle 'run Running' 30
            if ($running -notlike '*run Running*') { throw "the run never reported Running: $running" }
            $cancel = Wait-Name $app.Handle 'Cancel' 10
            if (-not $cancel) { throw 'the toolbar never published a Cancel button' }
            Start-Sleep -Milliseconds 300
            $results.Add((Save-Frame $app.Handle 'progress-and-cancel'))
            Close-App $app
        }
        default { throw "unknown scenario $name" }
    }
    Write-Capture "=== done $name ==="
}

$scenarios = @('welcome-dark', 'welcome-light', 'editor-and-results', 'plot', 'symbolic-inspector',
    'find-and-replace', 'signature-help', 'log-flood', 'layout-compact-200', 'progress-and-cancel')
if ($Scenario -ne 'all') { $scenarios = @($Scenario) }
foreach ($name in $scenarios) { Invoke-Scenario $name }

Write-Capture '=== summary ==='
$jsonPath = Join-Path $WorkRoot 'captures.json'
$results | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $jsonPath -Encoding UTF8
foreach ($r in $results) { Write-Capture "IMAGE $($r.Name) $($r.Bytes) bytes $($r.Size) sha256=$($r.Sha256)" }
Write-Capture "wrote $jsonPath"




