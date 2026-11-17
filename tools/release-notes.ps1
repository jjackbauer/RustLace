<#
.SYNOPSIS
    Generate the Markdown body for a RustLace GitHub Release.

.DESCRIPTION
    Reads the commits in the release range (the commits since the last vX.Y.Z
    tag, or the whole history for the first release), groups them by
    conventional-commit type, and writes a release-notes file. The workflow
    passes the artifact identity so the published note carries the same size
    and SHA-256 as SHA256SUMS.

.PARAMETER RepoRoot
    Repository root. Defaults to the parent of this script's directory.

.PARAMETER Version
    Version being released, without the leading v (for example 0.2.0).

.PARAMETER PreviousTag
    Tag to compare from. When omitted the nearest reachable vX.Y.Z tag is used,
    or the whole history when no tag exists.

.PARAMETER OutputPath
    File to write. Defaults to RELEASE_NOTES.md in the current directory.

.PARAMETER Repository
    owner/repo, used for the compare link.

.PARAMETER CiUrl
    URL of the CI run that produced the artifact.

.PARAMETER CiSha
    Commit SHA the artifact was built from.

.PARAMETER Sha256
    SHA-256 of the released Windows executable.

.PARAMETER SizeBytes
    Size of the released executable in bytes.

.PARAMETER RustcVersion
    The exact `rustc --version` of the release job, recorded in the provenance section.

.PARAMETER Targets
    The build targets, recorded in the provenance section.
#>
[CmdletBinding()]
param(
    [string] $RepoRoot,
    [Parameter(Mandatory = $true)] [string] $Version,
    [string] $PreviousTag,
    [string] $OutputPath = 'RELEASE_NOTES.md',
    [string] $Repository,
    [string] $CiUrl,
    [string] $CiSha,
    [string] $Sha256,
    [string] $SizeBytes,
    [string] $RustcVersion,
    [string] $Targets = 'x86_64-pc-windows-msvc, x86_64-unknown-linux-gnu'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not $RepoRoot) {
    $RepoRoot = Split-Path -Parent $PSScriptRoot
}
if (-not $OutputPath) {
    $OutputPath = 'RELEASE_NOTES.md'
}

function Invoke-Git {
    param([string] $Root, [string[]] $Arguments)
    $saved = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $lines = & git -C $Root @Arguments 2>$null
        $exit = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $saved
    }
    return [pscustomobject]@{ ExitCode = $exit; Lines = @($lines) }
}

function Get-Type {
    param([string] $Subject)
    if ($Subject -match '^(?<type>[A-Za-z][A-Za-z0-9_-]*)(?:\((?<scope>[^)]*)\))?(?<bang>!)?:\s+') {
        return [pscustomobject]@{ Type = $matches['type'].ToLowerInvariant(); Bang = [bool] $matches['bang'] }
    }
    return [pscustomobject]@{ Type = $null; Bang = $false }
}

$tag = $PreviousTag
if (-not $tag) {
    $described = Invoke-Git -Root $RepoRoot -Arguments @('describe', '--tags', '--abbrev=0', '--match', 'v[0-9]*')
    if ($described.ExitCode -eq 0 -and $described.Lines.Count -gt 0) {
        $tag = ([string] $described.Lines[0]).Trim()
    }
}
if ($tag -and $tag -notmatch '^v\d+\.\d+\.\d+$') {
    throw "previous tag '$tag' is not vX.Y.Z"
}
$range = if ($tag) { "$tag..HEAD" } else { 'HEAD' }
$format = '%h%x1f%s%x1f%b%x1e'
$logged = Invoke-Git -Root $RepoRoot -Arguments @('log', $range, "--format=$format")
if ($logged.ExitCode -ne 0) {
    throw "git log $range failed"
}

$recordSeparator = [string] [char] 0x1e
$fieldSeparator = [string] [char] 0x1f
$text = ($logged.Lines -join "`n")
$commits = @()
foreach ($record in ($text -split [regex]::Escape($recordSeparator))) {
    $record = $record.Trim()
    if (-not $record) { continue }
    $parts = $record -split [regex]::Escape($fieldSeparator)
    if ($parts.Count -lt 2) { continue }
    $body = ''
    if ($parts.Count -gt 2) {
        $body = ($parts[2..($parts.Count - 1)] -join $fieldSeparator)
    }
    $subject = $parts[1].Trim()
    $typed = Get-Type $subject
    $breakingFooter = $false
    if ($subject -match '^BREAKING[ -]CHANGE:') { $breakingFooter = $true }
    foreach ($line in ($body -split "`n")) {
        if ($line -match '^BREAKING[ -]CHANGE:') { $breakingFooter = $true }
    }
    $commits += [pscustomobject]@{
        Sha      = $parts[0].Trim()
        Subject  = $subject
        Type     = $typed.Type
        Breaking = [bool] ($typed.Bang -or $breakingFooter)
    }
}

$sections = [ordered] @{
    breaking    = New-Object System.Collections.ArrayList
    features    = New-Object System.Collections.ArrayList
    fixes       = New-Object System.Collections.ArrayList
    docs        = New-Object System.Collections.ArrayList
    tests       = New-Object System.Collections.ArrayList
    ci          = New-Object System.Collections.ArrayList
    maintenance = New-Object System.Collections.ArrayList
    other       = New-Object System.Collections.ArrayList
}
foreach ($commit in $commits) {
    if ($commit.Breaking) { [void] $sections.breaking.Add($commit); continue }
    switch ($commit.Type) {
        'feat' { [void] $sections.features.Add($commit) }
        'fix' { [void] $sections.fixes.Add($commit) }
        'perf' { [void] $sections.fixes.Add($commit) }
        'docs' { [void] $sections.docs.Add($commit) }
        'test' { [void] $sections.tests.Add($commit) }
        'ci' { [void] $sections.ci.Add($commit) }
        'chore' { [void] $sections.maintenance.Add($commit) }
        'style' { [void] $sections.maintenance.Add($commit) }
        'refactor' { [void] $sections.maintenance.Add($commit) }
        'build' { [void] $sections.maintenance.Add($commit) }
        'skills' { [void] $sections.maintenance.Add($commit) }
        'release' { [void] $sections.maintenance.Add($commit) }
        default { [void] $sections.other.Add($commit) }
    }
}

function Format-CommitList {
    param($Items, [int] $Limit)
    $lines = @()
    $count = 0
    foreach ($item in $Items) {
        if ($Limit -gt 0 -and $count -ge $Limit) { break }
        $lines += "- ``$($item.Sha)`` $($item.Subject)"
        $count++
    }
    if ($Limit -gt 0 -and $Items.Count -gt $Limit) {
        $hidden = $Items.Count - $Limit
        $lines += "- _... and $hidden more commit(s)_"
    }
    return $lines
}

$out = New-Object System.Collections.ArrayList
[void] $out.Add("# RustLace v$Version")
[void] $out.Add('')
if ($CiSha) {
    if ($CiUrl) {
        [void] $out.Add("Automatic release from commit ``$CiSha`` ([CI run]($CiUrl)).")
    } else {
        [void] $out.Add("Automatic release from commit ``$CiSha``.")
    }
} else {
    [void] $out.Add('Automatic release.')
}
if ($tag) {
    [void] $out.Add("")
    [void] $out.Add("Changes since ``$tag`` ($($commits.Count) commit(s)).")
} else {
    [void] $out.Add("")
    [void] $out.Add("First release: the whole history up to this tag ($($commits.Count) commit(s)).")
}

if ($sections.breaking.Count -gt 0) {
    [void] $out.Add('')
    [void] $out.Add('## Breaking changes')
    [void] $out.Add('')
    foreach ($line in (Format-CommitList -Items $sections.breaking -Limit 0)) { [void] $out.Add($line) }
    [void] $out.Add('')
    [void] $out.Add('This is a 0.x release: breaking changes ship as a minor bump, and no stability promise is made. See `docs/release.md`.')
}
if ($sections.features.Count -gt 0) {
    [void] $out.Add('')
    [void] $out.Add('## Features')
    [void] $out.Add('')
    foreach ($line in (Format-CommitList -Items $sections.features -Limit 0)) { [void] $out.Add($line) }
}
if ($sections.fixes.Count -gt 0) {
    [void] $out.Add('')
    [void] $out.Add('## Fixes and performance')
    [void] $out.Add('')
    foreach ($line in (Format-CommitList -Items $sections.fixes -Limit 0)) { [void] $out.Add($line) }
}
if ($sections.docs.Count -gt 0) {
    [void] $out.Add('')
    [void] $out.Add('## Documentation')
    [void] $out.Add('')
    foreach ($line in (Format-CommitList -Items $sections.docs -Limit 0)) { [void] $out.Add($line) }
}
if ($sections.tests.Count -gt 0) {
    [void] $out.Add('')
    [void] $out.Add('## Tests')
    [void] $out.Add('')
    foreach ($line in (Format-CommitList -Items $sections.tests -Limit 0)) { [void] $out.Add($line) }
}
if ($sections.ci.Count -gt 0) {
    [void] $out.Add('')
    [void] $out.Add('## CI')
    [void] $out.Add('')
    foreach ($line in (Format-CommitList -Items $sections.ci -Limit 0)) { [void] $out.Add($line) }
}
if ($sections.maintenance.Count -gt 0) {
    [void] $out.Add('')
    [void] $out.Add('## Maintenance')
    [void] $out.Add('')
    foreach ($line in (Format-CommitList -Items $sections.maintenance -Limit 0)) { [void] $out.Add($line) }
}
if ($sections.other.Count -gt 0) {
    [void] $out.Add('')
    [void] $out.Add('## Other changes')
    [void] $out.Add('')
    foreach ($line in (Format-CommitList -Items $sections.other -Limit 15)) { [void] $out.Add($line) }
}

[void] $out.Add('')
[void] $out.Add('## Artifacts')
[void] $out.Add('')
if ($Sha256) {
    $sizeText = if ($SizeBytes) { "$SizeBytes bytes" } else { 'size not recorded' }
    [void] $out.Add("- ``lovelace-desktop.exe`` - Windows x86_64, $sizeText, SHA-256 ``$Sha256``")
} else {
    [void] $out.Add('- `lovelace-desktop.exe` - Windows x86_64')
}
[void] $out.Add('- `SHA256SUMS.windows` - the Windows hash in `sha256sum` format')
[void] $out.Add('- `lovelace-desktop-linux-x86_64` - Linux x86_64, built at the same tag; its SHA-256 is in `SHA256SUMS.linux`')
[void] $out.Add('- `SHA256SUMS.linux` - the Linux hash in `sha256sum` format')
[void] $out.Add('')
[void] $out.Add('The Linux binary is built on Ubuntu 24.04 and needs glibc >= 2.39. It is verified')
[void] $out.Add('with software rendering (llvmpipe); no GPU driver is claimed. Run')
[void] $out.Add('`chmod +x lovelace-desktop-linux-x86_64` before launching it.')
[void] $out.Add('')
[void] $out.Add('The builds are unsigned: Windows SmartScreen shows "Unknown publisher" until the')
[void] $out.Add('hash earns reputation. The hash identifies the exact file, not a signer.')

[void] $out.Add('')
[void] $out.Add('## Provenance')
[void] $out.Add('')
[void] $out.Add("- Version: ``v$Version``")
if ($CiSha) { [void] $out.Add("- Commit: ``$CiSha``") }
if ($CiUrl) { [void] $out.Add("- CI run: $CiUrl") }
if ($RustcVersion) { [void] $out.Add("- Toolchain: ``$RustcVersion``") }
[void] $out.Add("- Build targets: ``$Targets``")
[void] $out.Add('- Checksums: `SHA256SUMS.windows` and `SHA256SUMS.linux` (attached)')

if ($tag -and $Repository) {
    [void] $out.Add('')
    [void] $out.Add("**Full changelog**: https://github.com/$Repository/compare/$tag...v$Version")
}

$joined = ($out -join "`n") + "`n"
$directory = Split-Path -Parent $OutputPath
if ($directory -and -not (Test-Path -LiteralPath $directory)) {
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
}
if (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path (Get-Location) $OutputPath
}
$utf8 = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($OutputPath, $joined, $utf8)
Write-Output "wrote $OutputPath ($($commits.Count) commits, previous tag: $(if ($tag) { $tag } else { 'none' }))"
