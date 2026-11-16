#!/usr/bin/env bash
# check-docs.sh - documentation consistency checker for RustLace.
#
# Usage: bash tools/check-docs.sh [--symbols] [--claims] [--readme]
#
# Current documentation is every Markdown file under docs/ except the two
# specialised trees:
#   docs/goal-desktop-v4/  archived specification; its capture links are
#                          deliberately not resolvable in this repository
#   docs/goal-docs-v1/     harness working memory, not documentation
#
# Checks:
#   1. every relative Markdown link target resolves, relative to the file
#      that contains it, to an existing file or directory;
#   2. every path-shaped inline code reference (a `a/b.rs` style path that
#      contains "/" and ends in .rs, .toml, .yml, .yaml, .json or .md, with an
#      optional :NNN or :NNN-MMM line suffix) names an existing file;
#   3. with --symbols: no *.rs:NNN line-suffix citation outside docs/adr/
#      (those records are historical), and every cited snake_case function
#      name (lowercase with underscores followed by "()") occurs in
#      lovelace-*/src.
#   4. with --claims: no page uses the literal phrases "exact 30-place",
#      "exactly 30 places" or "nothing is floating point" as a claim outside
#      the three pages that prohibit, replace or correct them, and every page
#      under docs/tutorials/ has a "## Precision" section. Fenced blocks and
#      inline code spans are mentions, not claims, and are not matched.
#   5. with --readme: README.md states its value proposition in the first ten
#      lines, carries a "## Current limitations" section and a fenced block
#      containing lovelace-run, and places all three before "## The crates".
#
# Failures are printed as file:line: message and the exit status is non-zero
# when there is at least one. Exit status 2 means bad usage or a missing tree.
#
# The script needs only bash, find, awk, grep, sort and mktemp: the tools
# present in Git Bash on windows-latest and in ubuntu-latest's bash.

set -u

usage() {
  printf 'usage: bash tools/check-docs.sh [--symbols] [--claims] [--readme]\n'
  printf '  (no flag)   check relative Markdown links and code references\n'
  printf '  --symbols   also reject .rs:NNN citations and unknown Rust symbols\n'
  printf '  --claims    also reject absolutist numerical claims and tutorials without ## Precision\n'
  printf '  --readme    also assert the README value proposition, limitations, command and order\n'
}

symbols=0
claims=0
readme=0
for arg in "$@"; do
  case "$arg" in
    --symbols) symbols=1 ;;
    --claims) claims=1 ;;
    --readme) readme=1 ;;
    -h|--help) usage; exit 0 ;;
    *)
      printf 'check-docs: unknown argument: %s\n' "$arg" >&2
      usage >&2
      exit 2
      ;;
  esac
done

# Byte-wise matching keeps the checker's output identical on every runner and
# avoids locale-dependent character classes in the regular expressions below.
LC_ALL=C
export LC_ALL

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd) || exit 2
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd) || exit 2
cd -- "$repo_root" || exit 2

if [ ! -d docs ]; then
  printf 'check-docs: no docs/ directory under %s\n' "$repo_root" >&2
  exit 2
fi

tmp=$(mktemp -d "${TMPDIR:-/tmp}/check-docs.XXXXXX") || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

files_checked=0
links_checked=0
refs_checked=0
citations_checked=0
symbols_checked=0
claim_hits=0
claim_pages=0
tutorials_checked=0
failures=0

fail() {
  # $1 file, $2 line, $3 message
  printf '%s:%s: %s\n' "$1" "$2" "$3" >&2
  failures=$((failures + 1))
}

# ---------------------------------------------------------------------------
# Current documentation: explicit, because the exclusions are policy, not an
# implementation detail.
# ---------------------------------------------------------------------------
is_current_doc() {
  case "$1" in
    docs/goal-desktop-v4/*|docs/goal-docs-v1/*) return 1 ;;
    docs/*) return 0 ;;
    *) return 1 ;;
  esac
}

# The three pages below are the only places that may contain the prohibited
# phrases: editorial.md forbids them, ADR-004 cites them as the wording it
# replaces, and the DSP page quotes-and-corrects them. Every other current
# documentation page must state the claim precisely instead.
claims_phrase_allowed() {
  case "$1" in
    docs/editorial.md|docs/adr/004-thirty-digit-default.md|docs/extensions/dsp.md) return 0 ;;
    *) return 1 ;;
  esac
}

# ---------------------------------------------------------------------------
# Code references are repository paths, so they are resolved from the
# repository root. Documentation also uses two shorter conventions that are
# still repository paths in context: crate-relative paths (`src/lib.rs`,
# `ui/app.rs`) and references into the carried archived fixture tree
# (`round-64/REF-cancel.json`). Resolve against those documented roots so a
# renamed or deleted file still fails while the conventions do not produce
# false positives.
# ---------------------------------------------------------------------------
ref_roots=("$repo_root" "$repo_root/docs" "$repo_root/docs/goal-desktop-v4")
for d in "$repo_root"/lovelace-*/; do
  [ -d "$d" ] || continue
  ref_roots+=("$d" "${d}src" "${d}tests")
done

resolve_ref() {
  # $1 repo-relative candidate, $2 directory of the citing file
  local candidate=$1 doc_dir=$2 base
  candidate=${candidate#./}
  candidate=${candidate#/}
  [ -z "$candidate" ] && return 1
  [ -e "$candidate" ] && return 0
  [ -e "$doc_dir/$candidate" ] && return 0
  for base in "${ref_roots[@]}"; do
    [ -e "$base/$candidate" ] && return 0
  done
  return 1
}

# The identifier index is built once: grepping lovelace-*/src for each cited
# symbol separately would be correct but needlessly quadratic.
symbol_index="$tmp/source-identifiers.txt"
if [ "$symbols" -eq 1 ]; then
  if ! grep -rhoE '[a-z][a-z0-9_]*' lovelace-*/src > "$symbol_index" 2>/dev/null; then
    : # grep exits 1 on "no match"; an empty index is still a valid index.
  fi
  sort -u -o "$symbol_index" "$symbol_index"
fi

bt=$(printf '\140')                 # a literal backtick
inline_code_re="${bt}[^${bt}]+${bt}"
link_re='\]\(([^\)]*)\)'
rs_citation_re='\.rs:[0-9]+(-[0-9]+)?'
symbol_re='[a-z][a-z0-9_]*\(\)'

# ---------------------------------------------------------------------------
# Main scan: one pass over every current page. Fenced code blocks are skipped
# (their contents are examples, not Markdown links or documentation
# citations); inline code spans are skipped for link parsing because Markdown
# renders them as code, not as links.
# ---------------------------------------------------------------------------
while IFS= read -r -d '' file; do
  is_current_doc "$file" || continue
  files_checked=$((files_checked + 1))
  doc_dir=$(dirname -- "$file")

  # Process substitution, not a pipeline: the counters and failures below are
  # accumulated in this shell, not in a subshell.
  while IFS=$'\t' read -r line_no text; do
    # --- links -------------------------------------------------------------
    stripped=$text
    while [[ $stripped =~ $inline_code_re ]]; do
      stripped=${stripped/"${BASH_REMATCH[0]}"/}
    done

    rest=$stripped
    while [[ $rest =~ $link_re ]]; do
      match=${BASH_REMATCH[0]}
      target=${BASH_REMATCH[1]}
      rest=${rest#*"$match"}

      # Leading whitespace, angle brackets and an optional title.
      while [[ $target == [[:space:]]* ]]; do target=${target#?}; done
      case "$target" in
        '<'*) target=${target#<}; target=${target%%>*} ;;
        *) target=${target%%[[:space:]]*} ;;
      esac
      [ -z "$target" ] && continue

      # External targets and pure anchors are not relative paths.
      case "$target" in
        '#'*) continue ;;
        '//'*) continue ;;
      esac
      if [[ $target =~ ^[A-Za-z][A-Za-z0-9+.-]*: ]]; then
        continue
      fi

      link_path=${target%%#*}
      link_path=${link_path%%\?*}
      link_path=${link_path//%20/ }
      [ -z "$link_path" ] && continue

      links_checked=$((links_checked + 1))
      if [[ $link_path == /* ]]; then
        resolved="$repo_root$link_path"
      else
        resolved="$doc_dir/$link_path"
      fi
      if [ ! -e "$resolved" ]; then
        fail "$file" "$line_no" "broken link '$target' (no such file or directory: ${resolved#"$repo_root"/})"
      fi
    done

    # --- inline code references -------------------------------------------
    code_rest=$text
    while [[ $code_rest =~ $inline_code_re ]]; do
      span=${BASH_REMATCH[0]}
      body=${span:1:${#span}-2}
      code_rest=${code_rest#*"$span"}

      # ---- --symbols: line-suffix citations and Rust function names -------
      if [ "$symbols" -eq 1 ]; then
        if [[ $body =~ $rs_citation_re ]]; then
          citations_checked=$((citations_checked + 1))
          case "$file" in
            docs/adr/*) ;;
            *)
              fail "$file" "$line_no" \
                "line-suffix citation '$body' is not allowed outside docs/adr/"
              ;;
          esac
        fi

        sym_rest=$body
        line_symbols=' '
        while [[ $sym_rest =~ $symbol_re ]]; do
          token=${BASH_REMATCH[0]}
          sym_rest=${sym_rest#*"$token"}
          name=${token%()}
          # "lowercase with underscores": plain words such as len() are not
          # symbol citations for this check.
          case "$name" in
            *_*) ;;
            *) continue ;;
          esac
          case "$line_symbols" in
            *" $name "*) continue ;;
          esac
          line_symbols="$line_symbols$name "
          symbols_checked=$((symbols_checked + 1))
          if ! grep -qxF -- "$name" "$symbol_index"; then
            fail "$file" "$line_no" \
              "cited Rust function '$name()' is not defined in lovelace-*/src"
          fi
        done
      fi

      # ---- code reference resolution --------------------------------------
      ref=$body
      while [[ $ref == [[:space:]]* ]]; do ref=${ref#?}; done
      while [[ $ref == *[[:space:]] ]]; do ref=${ref%?}; done
      [ -z "$ref" ] && continue

      # Commands and glob/placeholder shapes are not repository paths.
      case "$ref" in
        *[[:space:]]*|*'*'*|*'?'*|*'<'*|*'>'*|*'{'*|*'}'*) continue ;;
      esac
      ref=${ref%%#*}

      # Strip a trailing :NNN or :NNN-MMM line suffix before resolving.
      if [[ $ref =~ :[0-9]+(-[0-9]+)?$ ]]; then
        ref=${ref%:*}
      fi

      case "$ref" in
        */*.rs|*/*.toml|*/*.yml|*/*.yaml|*/*.json|*/*.md) ;;
        *) continue ;;
      esac

      refs_checked=$((refs_checked + 1))
      if ! resolve_ref "$ref" "$doc_dir"; then
        fail "$file" "$line_no" "code reference '$body' does not name an existing file"
      fi
    done
  done < <(awk '
    { sub(/\r$/, "") }
    {
      if (match($0, /^[ \t]*(```+|~~~+)/)) {
        marker = substr($0, RSTART, RLENGTH)
        sub(/^[ \t]*/, "", marker)
        if (fence == "") { fence = marker; next }
        if (substr(marker, 1, 1) == substr(fence, 1, 1)) { fence = ""; next }
      }
      if (fence != "") next
      print NR "\t" $0
    }
  ' "$file")
done < <(find docs -type f -name '*.md' -print0 | sort -z)

# ---------------------------------------------------------------------------
# --claims: the phrase half of the audit. Every Markdown page under docs/ is
# scanned, including the archived and working-memory trees, so no page escapes
# the measurement. Only prose counts as a claim: fenced blocks are examples and
# inline code spans are mentions (a quoted grep command or a forbidden phrase
# being discussed), so both are removed before matching. The three permitted
# files are the only file-level exceptions.
# ---------------------------------------------------------------------------
if [ "$claims" -eq 1 ]; then
  while IFS= read -r -d '' file; do
    claim_pages=$((claim_pages + 1))
    while IFS=$'\t' read -r line_no text; do
      claim_text=${text,,}
      claim_phrase=
      case "$claim_text" in
        *"exact 30-place"*) claim_phrase="exact 30-place" ;;
        *"exactly 30 places"*) claim_phrase="exactly 30 places" ;;
        *"nothing is floating point"*) claim_phrase="nothing is floating point" ;;
      esac
      if [ -n "$claim_phrase" ] && ! claims_phrase_allowed "$file"; then
        claim_hits=$((claim_hits + 1))
        fail "$file" "$line_no" \
          "prohibited numerical claim '$claim_phrase' (label it exact, precision-bounded or parity; see docs/editorial.md)"
      fi
    done < <(awk '
      { sub(/\r$/, "") }
      {
        if (match($0, /^[ \t]*(```+|~~~+)/)) {
          marker = substr($0, RSTART, RLENGTH)
          sub(/^[ \t]*/, "", marker)
          if (fence == "") { fence = marker; next }
          if (substr(marker, 1, 1) == substr(fence, 1, 1)) { fence = ""; next }
        }
        if (fence != "") next
        gsub(/`[^`]*`/, "", $0)
        print NR "\t" $0
      }
    ' "$file")
  done < <(find docs -type f -name '*.md' -print0 | sort -z)
fi

# ---------------------------------------------------------------------------
# --claims: every tutorial page must carry its precision section.
# ---------------------------------------------------------------------------
if [ "$claims" -eq 1 ]; then
  if [ ! -d docs/tutorials ]; then
    printf 'check-docs: no docs/tutorials/ directory under %s\n' "$repo_root" >&2
    exit 2
  fi
  while IFS= read -r -d '' file; do
    tutorials_checked=$((tutorials_checked + 1))
    if ! awk '
      { sub(/\r$/, "") }
      {
        if (match($0, /^[ \t]*(```+|~~~+)/)) {
          marker = substr($0, RSTART, RLENGTH)
          sub(/^[ \t]*/, "", marker)
          if (fence == "") { fence = marker; next }
          if (substr(marker, 1, 1) == substr(fence, 1, 1)) { fence = ""; next }
        }
        if (fence == "" && $0 ~ /^## Precision[ \t]*$/) { found = 1; exit }
      }
      END { exit(found ? 0 : 1) }
    ' "$file"; then
      fail "$file" 1 \
        "missing '## Precision' section (every tutorial labels its values exact, precision-bounded or parity)"
    fi
  done < <(find docs/tutorials -type f -name '*.md' -print0 | sort -z)
fi

# ---------------------------------------------------------------------------
# --readme: the structural promises the README makes to a newcomer, measured
# instead of asserted. The value-proposition rule accepts a first-ten-lines
# sentence only when it names a capability; a sentence that only claims
# lineage does not pass.
# ---------------------------------------------------------------------------
readme_missing() {
  printf 'README.md: missing %s\n' "$1" >&2
  failures=$((failures + 1))
}

# first_level2_heading FILE '## Exact heading' -> the first matching heading
# line outside a fenced block, or nothing. Shared by the README placement rules
# so a code sample cannot satisfy a section requirement.
first_level2_heading() {
  awk -v wanted="$2" '
    { sub(/\r$/, "") }
    {
      if (match($0, /^[ \t]*(```+|~~~+)/)) {
        marker = substr($0, RSTART, RLENGTH)
        sub(/^[ \t]*/, "", marker)
        if (fence == "") { fence = marker; next }
        if (substr(marker, 1, 1) == substr(fence, 1, 1)) { fence = ""; next }
      }
      if (fence != "") next
      sub(/[ \t]+$/, "", $0)
      if ($0 == wanted) { print NR; exit }
    }
  ' "$1"
}

check_readme() {
  local value_line limitations_line command_line crates_line

  if [ ! -f README.md ]; then
    readme_missing "the file itself (no README.md at the repository root)"
    return
  fi

  value_line=$(awk '
    { sub(/\r$/, "") }
    NR > 10 { exit }
    {
      if (match($0, /^[ \t]*(```+|~~~+)/)) {
        marker = substr($0, RSTART, RLENGTH)
        sub(/^[ \t]*/, "", marker)
        if (fence == "") { fence = marker; next }
        if (substr(marker, 1, 1) == substr(fence, 1, 1)) { fence = ""; next }
      }
      if (fence != "") next
      line = tolower($0)
      if ($0 !~ /^[ \t]*[#|]/ && NF >= 5 &&
          line ~ /(arithmetic|mathemat|array|symbolic|precision|scripting|computing|differentiat|polynomial|signal)/ &&
          line !~ /(lovelacesharp|successor|descends|descended|port of|c#)/) {
        print NR
        exit
      }
    }
  ' README.md)

  limitations_line=$(first_level2_heading README.md "## Current limitations")

  command_line=$(awk '
    { sub(/\r$/, "") }
    {
      if (match($0, /^[ \t]*(```+|~~~+)/)) {
        marker = substr($0, RSTART, RLENGTH)
        sub(/^[ \t]*/, "", marker)
        if (fence == "") { fence = marker; next }
        if (substr(marker, 1, 1) == substr(fence, 1, 1)) { fence = ""; next }
      }
      if (fence != "" && $0 ~ /lovelace-run/) { print NR; exit }
    }
  ' README.md)

  crates_line=$(first_level2_heading README.md "## The crates")

  if [ -z "$value_line" ]; then
    readme_missing "value proposition in the first 10 lines (a sentence naming what the software does, not its lineage)"
  fi
  if [ -z "$limitations_line" ]; then
    readme_missing "'## Current limitations' section"
  fi
  if [ -z "$command_line" ]; then
    readme_missing "a fenced block containing lovelace-run"
  fi
  if [ -z "$crates_line" ]; then
    readme_missing "'## The crates' section (needed to order the required material)"
    return
  fi

  if [ -n "$value_line" ] && [ "$value_line" -gt "$crates_line" ]; then
    readme_missing "required order: the value proposition must appear before '## The crates'"
  fi
  if [ -n "$limitations_line" ] && [ "$limitations_line" -gt "$crates_line" ]; then
    readme_missing "required order: '## Current limitations' must appear before '## The crates'"
  fi
  if [ -n "$command_line" ] && [ "$command_line" -gt "$crates_line" ]; then
    readme_missing "required order: the lovelace-run command must appear before '## The crates'"
  fi
}

if [ "$readme" -eq 1 ]; then
  check_readme
fi

if [ "$files_checked" -eq 0 ]; then
  printf 'check-docs: no current Markdown files found under docs/\n' >&2
  exit 2
fi

if [ "$symbols" -eq 1 ]; then
  printf 'check-docs: %d Markdown files, %d relative links, %d code references, %d line-suffix citations, %d symbol citations\n' \
    "$files_checked" "$links_checked" "$refs_checked" "$citations_checked" "$symbols_checked"
else
  printf 'check-docs: %d Markdown files, %d relative links, %d code references\n' \
    "$files_checked" "$links_checked" "$refs_checked"
fi

if [ "$claims" -eq 1 ]; then
  printf 'check-docs: claims audit: %d pages under docs/, %d tutorial pages, %d prohibited phrase(s)\n' \
    "$claim_pages" "$tutorials_checked" "$claim_hits"
fi
if [ "$readme" -eq 1 ]; then
  printf 'check-docs: README.md structural audit: 4 requirements checked\n'
fi

if [ "$failures" -gt 0 ]; then
  printf 'check-docs: FAIL (%d failure(s))\n' "$failures"
  exit 1
fi
printf 'check-docs: OK\n'
exit 0
