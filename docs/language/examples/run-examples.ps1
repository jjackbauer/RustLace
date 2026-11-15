#requires -version 5.1
<#
    Runs every ```lovelace block in docs/language/*.md through lovelace-run --text
    and compares the normalised result with the ```text block that follows it.

    It then checks the catalogue in docs/language/builtins.md against the live
    registry (`lovelace-run --functions --function-limit 500`): the per-category
    counts in "Registry at a glance", every section-heading count, and every
    Name/Arity/Parameters row of the "Full catalogue".

    Usage (from the repository root):
        pwsh -File docs/language/examples/run-examples.ps1

    The binary defaults to target/debug/lovelace-run.exe and can be overridden with
    the LOVELACE_RUN environment variable. The runner writes its scratch files under
    target/example-run (a build-output directory) and exits 0 only when every
    example matches and the catalogue agrees with the registry.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$docs = Join-Path $repo 'docs\language'
$exe = if ($env:LOVELACE_RUN) { $env:LOVELACE_RUN } else {
    if ($env:OS -eq 'Windows_NT') { Join-Path $repo 'target\debug\lovelace-run.exe' }
    else { Join-Path $repo 'target\debug\lovelace-run' }
}
if (-not (Test-Path -LiteralPath $exe)) {
    throw "lovelace-run not found at '$exe'. Build it first with: cargo build -p lovelace-run"
}

$scratch = Join-Path $repo 'target\example-run'
$tmp = Join-Path $scratch 'tmp'
$plots = Join-Path $scratch 'plots'
$appdata = Join-Path $scratch 'appdata'
foreach ($dir in @($scratch, $tmp, $plots, $appdata)) {
    if (-not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
}

# Keep plots and any other temporary output inside the workspace, and give the
# engine a writable temp directory even on a restricted host.
$env:TEMP = $tmp
$env:TMP = $tmp
$env:APPDATA = $appdata

$utf8 = New-Object System.Text.UTF8Encoding($false)

function Quote-Argument([string]$value) {
    if ($value -match '[\s"]') {
        return '"' + ($value -replace '"', '\"') + '"'
    }
    return $value
}

function Normalise-Text([string]$text) {
    $text = $text -replace "`r`n", "`n"
    $text = $text -replace "`r", "`n"
    $lines = @($text -split "`n" | ForEach-Object { ($_ -replace '\s+', ' ').Trim() })
    $first = 0
    while ($first -lt $lines.Count -and $lines[$first] -eq '') { $first++ }
    $last = $lines.Count - 1
    while ($last -ge $first -and $lines[$last] -eq '') { $last-- }
    if ($last -lt $first) { return '' }
    return (($lines[$first..$last]) -join "`n")
}

function Get-Examples([string]$path) {
    $lines = [System.IO.File]::ReadAllLines($path, $utf8)
    $examples = New-Object System.Collections.ArrayList
    $i = 0
    while ($i -lt $lines.Count) {
        if ($lines[$i] -match '^```lovelace\s*$') {
            $sourceStart = $i + 1
            $j = $sourceStart
            while ($j -lt $lines.Count -and $lines[$j] -notmatch '^```\s*$') { $j++ }
            if ($j -ge $lines.Count) { throw "${path}:$($i + 1): unterminated lovelace fence" }
            $source = ($lines[$sourceStart..($j - 1)] -join "`n")

            # The expected output is the next fenced block, which must be a text block.
            $k = $j + 1
            while ($k -lt $lines.Count -and $lines[$k] -match '^\s*$') { $k++ }
            if ($k -ge $lines.Count) { throw "${path}:$($i + 1): lovelace block has no expected text block" }
            if ($lines[$k] -notmatch '^```text\s*$') {
                throw "${path}:$($i + 1): expected a ```text block, found '$($lines[$k])'"
            }
            $textStart = $k + 1
            $m = $textStart
            while ($m -lt $lines.Count -and $lines[$m] -notmatch '^```\s*$') { $m++ }
            if ($m -ge $lines.Count) { throw "${path}:$($k + 1): unterminated text fence" }
            $expected = ($lines[$textStart..($m - 1)] -join "`n")

            [void]$examples.Add([pscustomobject]@{
                Path = $path
                Line = $i + 1
                Source = $source
                Expected = $expected
            })
            $i = $m
            continue
        }
        $i++
    }
    return $examples
}

function Invoke-Case([string]$source, [int]$index) {
    $file = Join-Path $scratch ("case-$index.lv")
    [System.IO.File]::WriteAllText($file, $source, $utf8)
    $stdout = Join-Path $scratch ("case-$index.out")
    $stderr = Join-Path $scratch ("case-$index.err")
    if (Test-Path -LiteralPath $stdout) { Remove-Item -LiteralPath $stdout -Force }
    if (Test-Path -LiteralPath $stderr) { Remove-Item -LiteralPath $stderr -Force }

    $arguments = @('--file', (Quote-Argument $file), '--text', '--plot-dir', (Quote-Argument $plots), '--plot-file', ('case-' + $index + '.svg'))
    $process = Start-Process -FilePath $exe -ArgumentList ($arguments -join ' ') `
        -WorkingDirectory $repo -NoNewWindow -Wait -PassThru `
        -RedirectStandardOutput $stdout -RedirectStandardError $stderr

    $outText = if (Test-Path -LiteralPath $stdout) { [System.IO.File]::ReadAllText($stdout, $utf8) } else { '' }
    $errText = if (Test-Path -LiteralPath $stderr) { [System.IO.File]::ReadAllText($stderr, $utf8) } else { '' }
    Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue

    return [pscustomobject]@{
        ExitCode = $process.ExitCode
        Stdout = $outText
        Stderr = $errText
    }
}

$pages = @(
    'README.md',
    'lexical-structure.md',
    'values-and-types.md',
    'expressions-and-operators.md',
    'statements-and-control-flow.md',
    'functions-and-scope.md',
    'builtins.md',
    'errors-and-diagnostics.md',
    'precision-and-formatting.md',
    'plots.md'
)

$total = 0
$failed = 0
$case = 0
foreach ($page in $pages) {
    $path = Join-Path $docs $page
    if (-not (Test-Path -LiteralPath $path)) { throw "missing page: $path" }
    $examples = Get-Examples $path
    $passedHere = 0
    foreach ($example in $examples) {
        $case++
        $total++
        $run = Invoke-Case $example.Source $case
        $actualRaw = if ($run.ExitCode -eq 0) { $run.Stdout } else { $run.Stderr }
        $expected = Normalise-Text $example.Expected
        $actual = Normalise-Text $actualRaw
        if ($actual -ceq $expected) {
            $passedHere++
            continue
        }

        $failed++
        Write-Host ""
        Write-Host "FAIL ${page}:$($example.Line) (exit $($run.ExitCode))" -ForegroundColor Red
        Write-Host "--- script ---"
        Write-Host $example.Source
        Write-Host "--- expected ---"
        Write-Host $expected
        Write-Host "--- actual ---"
        Write-Host $actual
        if ($run.ExitCode -eq 0 -and $run.Stderr) {
            Write-Host "--- stderr ---"
            Write-Host (Normalise-Text $run.Stderr)
        }
    }
    Write-Host ("{0}: {1} example(s) passed" -f $page, $passedHere)
}

# ---------------------------------------------------------------------------
# The catalogue check.
#
# The examples prove the prose and the snippets; they do not prove the
# catalogue tables in builtins.md. This second check reads the live registry
# through `--functions --function-limit 500` and compares it with every table
# on the page: the per-category totals in "Registry at a glance", the count in
# each section heading, and every Name/Arity/Parameters row of the "Full
# catalogue". A mismatch is a documentation defect, so it fails the run.
# ---------------------------------------------------------------------------
function ConvertTo-ArityText([object] $info) {
    if ($info.variadic) { return ("{0}+" -f $info.minArity) }
    if ([int] $info.minArity -eq [int] $info.parameterCount) { return ("{0}" -f $info.minArity) }
    return ("{0}-{1}" -f $info.minArity, $info.parameterCount)
}

function ConvertTo-ParameterText([object] $info) {
    $names = @($info.parameters)
    if ($names.Count -eq 0) { return '&mdash;' }
    return ($names -join ', ')
}

$catalogueProblems = New-Object System.Collections.ArrayList
$builtinsPath = Join-Path $docs 'builtins.md'
if (-not (Test-Path -LiteralPath $builtinsPath)) {
    throw "missing catalogue page: $builtinsPath"
}

$listingText = (& $exe --functions --function-limit 500) -join "`n"
$listing = $listingText | ConvertFrom-Json
$registry = @{}
$registryByCategory = @{}
foreach ($info in @($listing.functions)) {
    $registry[$info.name] = $info
    if (-not $registryByCategory.ContainsKey($info.category)) {
        $registryByCategory[$info.category] = 0
    }
    $registryByCategory[$info.category]++
}
if ([int] $listing.total -ne $registry.Count) {
    [void] $catalogueProblems.Add("--functions reports total $($listing.total) but returned $($registry.Count) entries")
}
if ($listing.truncated) {
    [void] $catalogueProblems.Add("--functions --function-limit 500 truncated the listing at $($registry.Count) of $($listing.total) entries")
}

$lines = [System.IO.File]::ReadAllLines($builtinsPath, $utf8)

# 1. The "Registry at a glance" table.
$inGlance = $false
$glance = @{}
$glanceTotal = $null
foreach ($line in $lines) {
    if ($line -match '^## Registry at a glance\s*$') { $inGlance = $true; continue }
    if ($inGlance -and $line -match '^## ') { $inGlance = $false }
    if (-not $inGlance) { continue }
    if ($line -match '^\| \*\*Total\*\* \| \*\*(\d+)\*\* \|') {
        $glanceTotal = [int] $matches[1]
        continue
    }
    if ($line -match '^\| ([A-Za-z][^|]*) \| (\d+) \|') {
        $glance[$matches[1].Trim()] = [int] $matches[2]
    }
}
if ($null -eq $glanceTotal) {
    [void] $catalogueProblems.Add('the "Registry at a glance" table has no Total row')
} elseif ($glanceTotal -ne [int] $listing.total) {
    [void] $catalogueProblems.Add("the glance Total says $glanceTotal but --functions reports $($listing.total)")
}
$glanceSum = ($glance.Values | Measure-Object -Sum).Sum
if ($glanceSum -ne [int] $listing.total) {
    [void] $catalogueProblems.Add("the glance categories sum to $glanceSum but --functions reports $($listing.total)")
}
foreach ($category in $registryByCategory.Keys) {
    if (-not $glance.ContainsKey($category)) {
        [void] $catalogueProblems.Add("the glance table omits category '$category' ($($registryByCategory[$category]) builtins)")
    } elseif ($glance[$category] -ne $registryByCategory[$category]) {
        [void] $catalogueProblems.Add("the glance table says '$category' has $($glance[$category]); the registry has $($registryByCategory[$category])")
    }
}
foreach ($category in $glance.Keys) {
    if (-not $registryByCategory.ContainsKey($category)) {
        [void] $catalogueProblems.Add("the glance table names category '$category', which the registry does not publish")
    }
}

# 2. The "Full catalogue" section: headings and rows.
$inFull = $false
$docRows = @{}
$sectionDeclared = @{}
$section = $null
foreach ($line in $lines) {
    if ($line -match '^## Full catalogue\s*$') { $inFull = $true; continue }
    if (-not $inFull) { continue }
    if ($line -match '^## ') { break }
    if ($line -match '^### ([^(]+) \((\d+) builtins?\)\s*$') {
        $section = $matches[1].Trim()
        $sectionDeclared[$section] = [int] $matches[2]
        continue
    }
    if ($line -match '^\| `([^`]+)` \| ([^|]+?) \| ([^|]+?) \|$') {
        $name = $matches[1]
        $arity = $matches[2].Trim()
        $parameters = $matches[3].Trim()
        if ($null -eq $section) {
            [void] $catalogueProblems.Add("catalogue row '$name' sits under no section heading")
            continue
        }
        if ($docRows.ContainsKey($name)) {
            [void] $catalogueProblems.Add("catalogue row '$name' appears twice")
            continue
        }
        $docRows[$name] = [pscustomobject]@{ Category = $section; Arity = $arity; Parameters = $parameters }
    }
}
foreach ($category in $registryByCategory.Keys) {
    $rowsHere = @($docRows.Values | Where-Object { $_.Category -eq $category }).Count
    if (-not $sectionDeclared.ContainsKey($category)) {
        [void] $catalogueProblems.Add("the full catalogue has no '$category' section")
    } else {
        if ($sectionDeclared[$category] -ne $registryByCategory[$category]) {
            [void] $catalogueProblems.Add("the '$category' heading says $($sectionDeclared[$category]) builtins; the registry has $($registryByCategory[$category])")
        }
        if ($rowsHere -ne $sectionDeclared[$category]) {
            [void] $catalogueProblems.Add("the '$category' section declares $($sectionDeclared[$category]) rows but lists $rowsHere")
        }
    }
}
foreach ($name in $registry.Keys) {
    $info = $registry[$name]
    if (-not $docRows.ContainsKey($name)) {
        [void] $catalogueProblems.Add("the full catalogue omits '$name'")
        continue
    }
    $row = $docRows[$name]
    if ($row.Category -ne $info.category) {
        [void] $catalogueProblems.Add("'$name' is listed under '$($row.Category)'; the registry says '$($info.category)'")
    }
    $expectedArity = ConvertTo-ArityText $info
    if ($row.Arity -ne $expectedArity) {
        [void] $catalogueProblems.Add("'$name' lists arity '$($row.Arity)'; the registry implies '$expectedArity'")
    }
    $expectedParameters = ConvertTo-ParameterText $info
    if ($row.Parameters -ne $expectedParameters) {
        [void] $catalogueProblems.Add("'$name' lists parameters '$($row.Parameters)'; the registry declares '$expectedParameters'")
    }
}
foreach ($name in $docRows.Keys) {
    if (-not $registry.ContainsKey($name)) {
        [void] $catalogueProblems.Add("the full catalogue lists '$name', which the registry does not publish")
    }
}

if ($catalogueProblems.Count -gt 0) {
    Write-Host ""
    Write-Host "catalogue: $($catalogueProblems.Count) mismatch(es) against --functions --function-limit 500" -ForegroundColor Red
    foreach ($problem in $catalogueProblems) { Write-Host "  $problem" -ForegroundColor Red }
    exit 1
}
Write-Host ("catalogue: {0} builtins in {1} categories match --functions --function-limit 500" -f $registry.Count, $registryByCategory.Count)

if ($failed -ne 0) {
    Write-Host ""
    Write-Host "$failed of $total example(s) failed" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "All $total examples passed."
exit 0
