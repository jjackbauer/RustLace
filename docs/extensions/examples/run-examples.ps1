<#
.SYNOPSIS
    Executes every `lovelace` block in docs/extensions/*.md and checks it against the `text` block
    that follows it.

.DESCRIPTION
    For each pair, the snippet is written to a .lv file and evaluated with

        lovelace-run --file <path> --text --omit-variables

    The combined stdout/stderr is compared with the documented block after whitespace
    normalisation (runs of whitespace collapse to one space, then trim). A snippet that is meant to
    fail still passes: the error line is its observed output, and the process exit code is reported
    but not compared.

    `--file` is used rather than `--eval` on purpose. Windows PowerShell 5.1 strips embedded double
    quotes when it builds a native command line, so `--eval 'symbol("x")'` delivers `symbol(x)`.
    That makes the engine answer `Undefined variable 'x'.` for a shell reason. Passing a file is
    immune to it.

    TEMP and APPDATA are redirected to a scratch directory inside the repository so nothing is
    written to the user profile.

.PARAMETER Exe
    Path to the lovelace-run executable. Defaults to target/debug/lovelace-run.exe, then
    target/release/lovelace-run.exe, and finally tries `cargo build -p lovelace-run`.

.PARAMETER Filter
    Only run examples from pages whose name matches this substring (e.g. -Filter dsp).

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File docs/extensions/examples/run-examples.ps1
#>
[CmdletBinding()]
param(
    [string]$Exe,
    [string]$Filter
)

$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$docsDir   = Split-Path -Parent $scriptDir
$root      = Split-Path -Parent (Split-Path -Parent $docsDir)

# Scratch space outside the repository: the runner must not write into the clone or the profile.
$scratch = Join-Path ([System.IO.Path]::GetTempPath()) 'lovelace-extensions-scratch'
New-Item -ItemType Directory -Force -Path $scratch | Out-Null
$env:TEMP = $scratch
$env:TMP = $scratch
$env:APPDATA = $scratch

function Resolve-LovelaceRun {
    param([string]$Given, [string]$Root)
    if ($Given) {
        if (Test-Path $Given) { return (Resolve-Path $Given).Path }
        throw "lovelace-run not found at '$Given'."
    }
    foreach ($candidate in @('target\debug\lovelace-run.exe', 'target\release\lovelace-run.exe')) {
        $path = Join-Path $Root $candidate
        if (Test-Path $path) { return $path }
    }
    Write-Host 'lovelace-run not built; running cargo build -p lovelace-run ...'
    $toolchain = Join-Path $Root '.toolchain'
    if (Test-Path $toolchain) {
        $env:RUSTUP_HOME = Join-Path $toolchain 'rustup'
        $env:CARGO_HOME = Join-Path $toolchain 'cargo'
        $cargo = Join-Path $toolchain 'cargo\bin\cargo.exe'
    } else {
        $cargo = 'cargo'
    }
    & $cargo build -p lovelace-run 2>&1 | Out-Null
    $path = Join-Path $Root 'target\debug\lovelace-run.exe'
    if (Test-Path $path) { return $path }
    throw 'lovelace-run could not be built.'
}

function Invoke-Snippet {
    param([string]$ExePath, [string]$Source, [string]$WorkDir)
    $file = Join-Path $WorkDir ('snippet-' + [guid]::NewGuid().ToString('N') + '.lv')
    [System.IO.File]::WriteAllText($file, $Source, (New-Object System.Text.UTF8Encoding($false)))
    try {
        $psi = New-Object System.Diagnostics.ProcessStartInfo
        $psi.FileName = $ExePath
        $psi.UseShellExecute = $false
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        $psi.CreateNoWindow = $true
        $psi.Arguments = '--file "' + $file + '" --text --omit-variables'
        $p = [System.Diagnostics.Process]::Start($psi)
        $out = $p.StandardOutput.ReadToEnd()
        $err = $p.StandardError.ReadToEnd()
        $p.WaitForExit()
        return [pscustomobject]@{
            Exit = $p.ExitCode
            Text = ((($out.Trim()) + "`n" + ($err.Trim())).Trim())
        }
    } finally {
        Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue
    }
}

function ConvertTo-Normalized([string]$s) {
    if ($null -eq $s) { return '' }
    return (($s -replace '\s+', ' ').Trim())
}

# Reads a page into an ordered list of fenced blocks: language, content, start line.
function Get-Blocks {
    param([string]$Path)
    $lines = Get-Content -LiteralPath $Path
    $blocks = New-Object System.Collections.ArrayList
    $inFence = $false
    $lang = ''
    $buffer = New-Object System.Collections.ArrayList
    $start = 0
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $line = $lines[$i]
        if (-not $inFence) {
            if ($line -match '^```\s*(\S*)\s*$') {
                $inFence = $true
                $lang = $Matches[1].ToLowerInvariant()
                $buffer.Clear()
                $start = $i + 2   # 1-based line number of the first content line
            }
        } elseif ($line -match '^```\s*$') {
            $inFence = $false
            [void]$blocks.Add([pscustomobject]@{ Lang = $lang; Content = ($buffer -join "`n"); Line = $start })
        } else {
            [void]$buffer.Add($line)
        }
    }
    return $blocks
}

$exePath = Resolve-LovelaceRun -Given $Exe -Root $root
Write-Host ("lovelace-run: {0}" -f $exePath)

$pages = Get-ChildItem -LiteralPath $docsDir -Filter '*.md' -File | Sort-Object Name
if ($Filter) { $pages = $pages | Where-Object { $_.Name -like "*$Filter*" } }
if (-not $pages) { throw "No pages found in $docsDir." }

$checked = 0
$passed = 0
$failed = 0
$failures = New-Object System.Collections.ArrayList
$unpaired = New-Object System.Collections.ArrayList

foreach ($page in $pages) {
    $blocks = Get-Blocks -Path $page.FullName
    for ($i = 0; $i -lt $blocks.Count; $i++) {
        $block = $blocks[$i]
        if ($block.Lang -ne 'lovelace') { continue }
        $expected = $null
        if (($i + 1) -lt $blocks.Count -and $blocks[$i + 1].Lang -eq 'text') {
            $expected = $blocks[$i + 1]
        }
        if ($null -eq $expected) {
            [void]$unpaired.Add(("{0}:{1}" -f $page.Name, $block.Line))
            continue
        }
        $i++   # consume the text block
        $checked++
        $result = Invoke-Snippet -ExePath $exePath -Source $block.Content -WorkDir $scratch
        $want = ConvertTo-Normalized $expected.Content
        $got = ConvertTo-Normalized $result.Text
        if ($got -ceq $want) {
            $passed++
            Write-Host ("PASS  {0}:{1} (exit {2})" -f $page.Name, $block.Line, $result.Exit)
        } else {
            $failed++
            Write-Host ("FAIL  {0}:{1} (exit {2})" -f $page.Name, $block.Line, $result.Exit)
            Write-Host ("      want: {0}" -f $want)
            Write-Host ("      got : {0}" -f $got)
            [void]$failures.Add(("{0}:{1}" -f $page.Name, $block.Line))
        }
    }
}

if ($unpaired.Count -gt 0) {
    Write-Host ''
    Write-Host 'lovelace blocks with no following text block:'
    foreach ($u in $unpaired) { Write-Host ("  {0}" -f $u) }
}

Write-Host ''
Write-Host ("pages: {0}   examples checked: {1}   passed: {2}   failed: {3}   unpaired: {4}" -f `
    $pages.Count, $checked, $passed, $failed, $unpaired.Count)

if ($checked -eq 0) {
    Write-Host 'No examples were found; nothing was verified.'
    exit 2
}
if ($failed -gt 0 -or $unpaired.Count -gt 0) {
    exit 1
}
Write-Host ("OK: {0} examples match the documented output." -f $checked)
exit 0
