<#
.SYNOPSIS
    Decide the next RustLace version from the commit range.

.DESCRIPTION
    A pure function over (current version, commits): the caller supplies the
    commits since the last vX.Y.Z tag, or every commit when no such tag exists.
    When -Commits is not supplied the commits are read from git, so the script
    can be driven with synthetic history in tests.

    Version policy (stated in docs/release.md and AGENTS.md):

      * major version 0 (the project makes no stability promise):
          feat:                     -> minor   0.1.0 -> 0.2.0
          fix: / perf:              -> patch   0.1.0 -> 0.1.1
          type!: or BREAKING CHANGE -> minor   0.1.0 -> 0.2.0
          explicit promotion        -> major   0.1.0 -> 1.0.0
      * major version 1 or later:
          type!: or BREAKING CHANGE -> major   x.y.z -> (x+1).0.0
          feat:                     -> minor
          fix: / perf:              -> patch
      * docs:, test:, ci:, chore:, style:, refactor:, build:, unknown prefixes
        and freeform messages do not release on their own.
      * 1.0.0 is reachable ONLY from a 0.x tree through an explicit promotion:
        a subject containing "release: 1.0.0", or a BREAKING CHANGE footer whose
        first line carries "major: 1". Once the major version is 1 or later the
        promotion token is ignored; nothing lowers or re-raises the major by
        hand.

    Output (default): one machine-readable verdict line and the evidence:

        previous=0.1.0 next=0.2.0 bump=minor reason="feat: ..." policy=0.x
        range=v0.1.0..HEAD
        commits=3
        deciding commits:
          1234abcd feat: ...

    With -Json the script prints one JSON object with the same fields instead.

.PARAMETER RepoRoot
    Repository root. Defaults to the parent of this script's directory.

.PARAMETER CurrentVersion
    Version to bump from. Defaults to [workspace.package] version in Cargo.toml.

.PARAMETER Commits
    Synthetic commit messages (subject line first, optional body after a
    newline). When supplied, git is not consulted at all. An empty array means
    "no commits" and therefore no bump.

.PARAMETER Json
    Emit a single JSON object instead of the human-readable block.

.PARAMETER FailOnNoBump
    Exit 2 when the range warrants no release. Default is exit 0 with bump=none.

.EXAMPLE
    ./tools/next-version.ps1

.EXAMPLE
    ./tools/next-version.ps1 -CurrentVersion 0.1.0 -Commits 'fix: correct the parser'

.EXAMPLE
    ./tools/next-version.ps1 -Json
#>
[CmdletBinding()]
param(
    [string] $RepoRoot,
    [string] $CurrentVersion,
    [string[]] $Commits,
    [switch] $Json,
    [switch] $FailOnNoBump
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not $RepoRoot) {
    $RepoRoot = Split-Path -Parent $PSScriptRoot
}

function ConvertTo-VersionParts {
    param([string] $Text, [string] $What)
    if ($Text -notmatch '^(?<major>\d+)\.(?<minor>\d+)\.(?<patch>\d+)$') {
        throw "$What '$Text' is not a semver X.Y.Z version"
    }
    return [pscustomobject]@{
        Major = [int] $matches['major']
        Minor = [int] $matches['minor']
        Patch = [int] $matches['patch']
    }
}

function Get-WorkspaceVersion {
    param([string] $Root)
    $cargoPath = Join-Path $Root 'Cargo.toml'
    if (-not (Test-Path -LiteralPath $cargoPath)) {
        throw "Cargo.toml not found at $cargoPath"
    }
    $inSection = $false
    foreach ($rawLine in [System.IO.File]::ReadAllLines($cargoPath)) {
        $line = ($rawLine -replace '#.*$', '').Trim()
        if ($line -match '^\[(?<name>[^\]]+)\]$') {
            $inSection = ($matches['name'].Trim() -eq 'workspace.package')
            continue
        }
        if ($inSection -and $line -match '^version\s*=\s*"(?<version>[^"]+)"$') {
            return $matches['version']
        }
    }
    throw "[workspace.package] version not found in $cargoPath"
}

function Invoke-Git {
    # Native stderr is data here, not a terminating error: Windows PowerShell
    # 5.1 turns redirected stderr into a terminating one under
    # $ErrorActionPreference = 'Stop'. Keep the caller's preference and inspect
    # the exit code instead.
    param([string] $Root, [string[]] $Arguments)
    $saved = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $lines = & git -C $Root @Arguments 2>$null
        $exit = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $saved
    }
    return [pscustomobject]@{
        ExitCode = $exit
        Lines    = @($lines)
    }
}

function Read-GitCommits {
    param([string] $Root)
    $tag = $null
    $described = Invoke-Git -Root $Root -Arguments @('describe', '--tags', '--abbrev=0', '--match', 'v[0-9]*')
    if ($described.ExitCode -eq 0 -and $described.Lines.Count -gt 0) {
        $tag = ([string] $described.Lines[0]).Trim()
    }
    if ($tag -and $tag -notmatch '^v\d+\.\d+\.\d+$') {
        throw "the nearest reachable tag '$tag' is not a vX.Y.Z release tag; refusing to guess a range"
    }
    $range = if ($tag) { "$tag..HEAD" } else { 'HEAD' }
    $format = '%H%x1f%s%x1f%b%x1e'
    $logged = Invoke-Git -Root $Root -Arguments @('log', $range, "--format=$format")
    if ($logged.ExitCode -ne 0) {
        throw "git log $range failed"
    }
    $text = ($logged.Lines -join "`n")
    $recordSeparator = [string] [char] 0x1e
    $fieldSeparator = [string] [char] 0x1f
    $commits = @()
    foreach ($record in ($text -split [regex]::Escape($recordSeparator))) {
        $record = $record.Trim()
        if (-not $record) { continue }
        $parts = $record -split [regex]::Escape($fieldSeparator)
        if ($parts.Count -lt 2) { continue }
        $sha = $parts[0].Trim()
        if ($sha.Length -gt 8) { $sha = $sha.Substring(0, 8) }
        $body = ''
        if ($parts.Count -gt 2) {
            $body = ($parts[2..($parts.Count - 1)] -join $fieldSeparator)
        }
        $commits += [pscustomobject]@{
            Sha     = $sha
            Subject = $parts[1].Trim()
            Body    = $body
        }
    }
    return [pscustomobject]@{
        Range   = if ($tag) { "$tag..HEAD" } else { 'HEAD (no vX.Y.Z tag)' }
        Commits = $commits
    }
}

function ConvertFrom-SuppliedCommits {
    param([string[]] $Messages)
    $commits = @()
    $index = 0
    foreach ($message in $Messages) {
        $index++
        $normalized = ([string] $message) -replace "`r`n", "`n"
        $lines = $normalized -split "`n"
        $subject = $lines[0].Trim()
        $body = ''
        if ($lines.Count -gt 1) {
            $body = ($lines[1..($lines.Count - 1)] -join "`n")
        }
        $commits += [pscustomobject]@{
            Sha     = "synthetic-$index"
            Subject = $subject
            Body    = $body
        }
    }
    return [pscustomobject]@{
        Range   = 'supplied -Commits'
        Commits = $commits
    }
}

function Get-CommitFacts {
    param($Commit)
    $subject = $Commit.Subject
    $body = $Commit.Body
    $type = $null
    $bang = $false
    if ($subject -match '^(?<type>[A-Za-z][A-Za-z0-9_-]*)(?:\((?<scope>[^)]*)\))?(?<bang>!)?:\s+(?<desc>\S.*)$') {
        $type = $matches['type'].ToLowerInvariant()
        $bang = [bool] $matches['bang']
    }

    $breakingFooter = $false
    $promotionFooter = $false
    $footerLines = @()
    if ($subject -match '^BREAKING[ -]CHANGE:') {
        $footerLines += $subject
    }
    foreach ($line in ($body -split "`n")) {
        if ($line -match '^BREAKING[ -]CHANGE:') {
            $footerLines += $line
        }
    }
    foreach ($line in $footerLines) {
        $breakingFooter = $true
        if ($line -match '^BREAKING[ -]CHANGE:(?<rest>.*)$' -and $matches['rest'] -match '\bmajor:\s*1\b') {
            $promotionFooter = $true
        }
    }

    $promotionSubject = $false
    if ($subject -match '(?i)\brelease:\s*1\.0\.0\b') {
        $promotionSubject = $true
    }

    return [pscustomobject]@{
        Sha         = $Commit.Sha
        Subject     = $subject
        Type        = $type
        Breaking    = [bool] ($bang -or $breakingFooter)
        Promotion   = [bool] ($promotionSubject -or $promotionFooter)
        Conventional = [bool] $type
    }
}

# ---- inputs -----------------------------------------------------------------

if ($CurrentVersion) {
    $previousParts = ConvertTo-VersionParts -Text $CurrentVersion -What 'current version'
    $previous = $CurrentVersion
} else {
    $previous = Get-WorkspaceVersion -Root $RepoRoot
    $previousParts = ConvertTo-VersionParts -Text $previous -What 'workspace version'
}

$overrides = $PSBoundParameters.ContainsKey('Commits')
if ($overrides) {
    $history = ConvertFrom-SuppliedCommits -Messages $Commits
} else {
    $history = Read-GitCommits -Root $RepoRoot
}

$policy = if ($previousParts.Major -eq 0) { '0.x' } else { '1.x' }
$facts = @($history.Commits | ForEach-Object { Get-CommitFacts -Commit $_ })

$promotion = @($facts | Where-Object { $_.Promotion })
$breaking = @($facts | Where-Object { $_.Breaking -and -not $_.Promotion })
$feat = @($facts | Where-Object { $_.Type -eq 'feat' -and -not $_.Breaking -and -not $_.Promotion })
$patch = @($facts | Where-Object { ($_.Type -eq 'fix' -or $_.Type -eq 'perf') -and -not $_.Breaking -and -not $_.Promotion })

$bump = 'none'
$deciding = @()
if ($promotion.Count -gt 0 -and $previousParts.Major -eq 0) {
    $bump = 'major'
    $deciding = $promotion
} elseif ($breaking.Count -gt 0) {
    $bump = if ($previousParts.Major -eq 0) { 'minor' } else { 'major' }
    $deciding = $breaking
} elseif ($feat.Count -gt 0) {
    $bump = 'minor'
    $deciding = $feat
} elseif ($patch.Count -gt 0) {
    $bump = 'patch'
    $deciding = $patch
}

switch ($bump) {
    'major' { $next = "$($previousParts.Major + 1).0.0" }
    'minor' { $next = "$($previousParts.Major).$($previousParts.Minor + 1).0" }
    'patch' { $next = "$($previousParts.Major).$($previousParts.Minor).$($previousParts.Patch + 1)" }
    default {
        $next = $previous
        $bump = 'none'
    }
}

if ($deciding.Count -gt 0) {
    $reasonText = $deciding[0].Subject
} else {
    $reasonText = 'no release-worthy commits'
}
$reason = $reasonText -replace '"', "'"

$decidingObjects = @($deciding | ForEach-Object {
    [pscustomobject]@{ sha = $_.Sha; subject = $_.Subject }
})

if ($Json) {
    $payload = [pscustomobject]@{
        previous  = $previous
        next      = $next
        bump      = $bump
        reason    = $reason
        policy    = $policy
        range     = $history.Range
        inspected = $facts.Count
        deciding  = $decidingObjects
    }
    Write-Output ($payload | ConvertTo-Json -Depth 5 -Compress)
} else {
    Write-Output "previous=$previous next=$next bump=$bump reason=`"$reason`" policy=$policy"
    Write-Output "range=$($history.Range)"
    Write-Output "commits=$($facts.Count)"
    if ($decidingObjects.Count -eq 0) {
        Write-Output 'deciding commits: none'
    } else {
        Write-Output 'deciding commits:'
        foreach ($item in $decidingObjects) {
            Write-Output "  $($item.sha) $($item.subject)"
        }
    }
}

if ($FailOnNoBump -and $bump -eq 'none') {
    exit 2
}
exit 0
