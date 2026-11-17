#!/usr/bin/env bash
# check-docs.sh - documentation consistency checker for RustLace.
#
# Usage: bash tools/check-docs.sh [--symbols] [--claims] [--readme]
#
# Scanned classes (each is checked for relative links, path-shaped code
# references and heading fragments unless a mode below says otherwise):
#   docs/                    current documentation, every Markdown page;
#   root pages               README.md, AGENTS.md, CONTRACT.md - the pages a
#                            reader or contributor is expected to open first;
#   agent instructions       .github/copilot-instructions.md and
#                            .dsh/skills/*/SKILL.md - the files a coding agent
#                            is told to treat as operative.
#
# Three trees under docs/ are deliberately excluded, for stated reasons:
#   docs/goal-desktop-v4/  archived specification; its capture links are
#                          deliberately not resolvable in this repository
#   docs/goal-docs-*/      harness working memory for any goal (v2, …),
#                          not documentation. The wildcard is deliberate: a new
#                          goal directory must not turn the gate red, and a
#                          journal is not a reader-facing page.
#   docs/internal/archive/ the archived goal-docs-v1 harness journals, moved
#                          out of the public documentation tree into the
#                          internal archive (RQ-024). They are evidence, not
#                          navigation, and are kept byte-for-byte.
#
# Checks:
#   1. every relative Markdown link target resolves, relative to the file
#      that contains it, to an existing file or directory;
#   2. every heading fragment (`page.md#anchor`, `#anchor`) names a heading in
#      the destination page under GitHub's slug rules: lowercase, punctuation
#      and HTML stripped, spaces turned into hyphens, with GitHub's -1/-2
#      suffix for a repeated heading. This is an ASCII approximation of the
#      GitHub anchor algorithm (non-ASCII letters are dropped); it verifies
#      that the anchor exists, not that the surrounding sentence is true;
#   3. every path-shaped inline code reference names an existing path. A
#      `a/b.rs` style file reference contains "/" and ends in .rs, .toml,
#      .yml, .yaml, .json or .md, with an optional :NNN or :NNN-MMM suffix,
#      and must resolve. A backticked path that ends in "/" and has at least
#      two segments names a directory and must resolve too; that half is what
#      catches a moved directory quoted in prose (RQ-015, EVD-056): before it,
#      `docs/goal-docs-v1/` survived although the directory had moved to
#      `docs/internal/archive/goal-docs-v1/`. A single-segment name (`rust/`,
#      `lang/`, `round-33/`) is not treated as a repository path: it is as
#      often a historical proper name, a corpus-id prefix or a capture round
#      named in the LovelaceSharp repository. An `http(s)://` URL is skipped,
#      and so is a path the citing line itself declares absent ("... exist
#      yet", "no longer ..."): that sentence is not a claim that it exists;
#   4. with --symbols: no *.rs:NNN line-suffix citation on a docs/ page
#      outside docs/adr/ (those records are historical). Root pages and agent
#      instructions may cite a source line as an operational anchor: AGENTS.md
#      does, and RQ-022's command audit is that class's gate. Every cited
#      snake_case function name (lowercase with underscores followed by "()")
#      must occur as an identifier in lovelace-*/src - a TOKEN SEARCH, not
#      semantic resolution: it proves the name exists somewhere in the source
#      text, not that the cited call site or signature exists.
#   5. with --claims: no page uses the literal phrases "exact 30-place",
#      "exactly 30 places" or "nothing is floating point" as a claim outside
#      the three pages that prohibit, replace or correct them, and every page
#      under docs/tutorials/ has a "## Precision" section. Fenced blocks and
#      inline code spans are mentions, not claims, and are not matched.
#   6. with --claims: no current page, root page or agent instruction uses the
#      retired claim-vocabulary term "precision-bounded" (TODO-007). The four
#      claim kinds are exact, precision-limited, error-bounded and
#      parity-verified (docs/editorial.md). The archived harness records never
#      reach this scan (docs/internal/archive/ and docs/goal-docs-*/ are not
#      current pages), and docs/adr/ may quote the term only as an explicit
#      historical quotation: the term in quotation marks on a line that also
#      marks it as history ("previously", "historically", "was called", ...).
#      A use reports file:line and the replacement to write.
#   7. with --readme: README.md states its value proposition in the first ten
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
  printf '  (no flag)   check relative Markdown links, heading fragments and code references\n'
  printf '              in docs/, README.md, AGENTS.md, CONTRACT.md and the agent instructions\n'
  printf '  --symbols   also reject .rs:NNN citations on docs/ pages and unknown Rust symbols\n'
  printf '              (a token search of lovelace-*/src, not semantic resolution)\n'
  printf '  --claims    also reject absolutist numerical claims, the retired claim term\n'
  printf '              and tutorials without ## Precision\n'
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
docs_pages=0
root_pages=0
agent_pages=0
links_checked=0
anchors_checked=0
refs_checked=0
citations_checked=0
symbols_checked=0
claim_hits=0
claim_pages=0
retired_hits=0
tutorials_checked=0
precision_sections=0
readme_requirements=0
failures=0

fail() {
  # $1 file, $2 line, $3 message
  printf '%s:%s: %s\n' "$1" "$2" "$3" >&2
  failures=$((failures + 1))
}

# ---------------------------------------------------------------------------
# Scanned classes. The exclusions are policy, not an implementation detail:
# the archived specification and every goal's harness memory are not
# reader-facing documentation, and their internal links are evidence, not
# navigation. Everything a reader or an agent is told to open is scanned.
# ---------------------------------------------------------------------------
is_current_doc() {
  case "$1" in
    docs/goal-desktop-v4/*) return 1 ;;
    docs/goal-docs-*) return 1 ;;
    docs/internal/archive/*) return 1 ;;
    docs/*) return 0 ;;
    README.md|AGENTS.md|CONTRACT.md) return 0 ;;
    .github/copilot-instructions.md) return 0 ;;
    .dsh/skills/*/SKILL.md) return 0 ;;
    *) return 1 ;;
  esac
}

page_class() {
  case "$1" in
    docs/*) printf 'docs' ;;
    README.md|AGENTS.md|CONTRACT.md) printf 'root' ;;
    *) printf 'agent' ;;
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

# Retired claim vocabulary (TODO-007). The four kinds are exact,
# precision-limited, error-bounded and parity-verified; "precision-bounded" is
# the retired name and may survive only in a historical record. The archived
# harness trees never reach this scan because is_current_doc excludes them.
# docs/adr/ is decision history, so it may quote the old wording as history:
# the term must be in quotation marks AND the same line must mark it as
# historical ("previously", "was called", ...). An unquoted use in an ADR is
# current prose and fails, so the ADR tree is not a blanket exemption.
retired_term_allowed() {
  # $1 file, $2 lowercased citing line
  case "$1" in
    docs/internal/archive/*|docs/goal-docs-*) return 0 ;;
    docs/adr/*)
      case "$2" in
        *'"precision-bounded"'*|*'`precision-bounded`'*) ;;
        *) return 1 ;;
      esac
      case "$2" in
        *previously*|*historically*|*"used to"*|*"was called"*|*retired*|*former*|*superseded*|*originally*) return 0 ;;
        *) return 1 ;;
      esac
      ;;
    *) return 1 ;;
  esac
}

# A path that the citing line itself declares absent is not a stale claim that
# the path exists: docs/editorial.md names `docs/how-to/` as a planned page
# shape and says in the same line that no how-to pages exist yet, and a change
# record may name a file it deleted. The markers are deliberately narrow and
# are measured on the citing line.
ref_documented_absent() {
  case "$1" in
    *"exist yet"*|*"does not exist"*|*"do not exist"*|*"not part of this repository"*|*"no longer"*|*"not yet"*) return 0 ;;
    *"deleted"*|*"deletes"*|*"deleting"*|*"deletion"*|*"removed"*|*"removes"*|*"removing"*|*"removal"*) return 0 ;;
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

# ---------------------------------------------------------------------------
# Heading anchors. GitHub derives a page fragment from the heading text:
# lowercase, Markdown/HTML formatting stripped, punctuation stripped, spaces
# turned into hyphens, and a -1/-2 suffix on a repeated heading. This awk
# implementation is ASCII-only (a non-ASCII letter is dropped, where GitHub
# keeps it); the file's own links are the check that matters, and every
# destination headline currently used is ASCII. Fenced blocks are skipped: a
# heading-shaped line in an example is not an anchor. Setext headings
# (`Title` over `====`) are not used in this repository and are not modelled.
# ---------------------------------------------------------------------------
heading_anchors() {
  awk '
    function slugify(s,   out, i, n, ch) {
      sub(/^[ \t]+/, "", s)
      sub(/[ \t]+$/, "", s)
      gsub(/<[^>]*>/, "", s)          # inline HTML
      gsub(/\]\([^)]*\)/, "", s)      # an inline link keeps its text only
      s = tolower(s)
      out = ""
      n = length(s)
      for (i = 1; i <= n; i++) {
        ch = substr(s, i, 1)
        if ((ch >= "a" && ch <= "z") || (ch >= "0" && ch <= "9") ||
            ch == "_" || ch == "-" || ch == " ") out = out ch
      }
      gsub(/ /, "-", out)
      return out
    }
    { sub(/\r$/, "") }
    {
      if (match($0, /^[ \t]*(```+|~~~+)/)) {
        marker = substr($0, RSTART, RLENGTH)
        sub(/^[ \t]*/, "", marker)
        if (fence == "") { fence = marker; next }
        if (substr(marker, 1, 1) == substr(fence, 1, 1)) { fence = ""; next }
      }
      if (fence != "") next
      if ($0 !~ /^#{1,6}[ \t]+/) next
      text = $0
      sub(/^#{1,6}[ \t]+/, "", text)
      sub(/[ \t]+#+[ \t]*$/, "", text)
      slug = slugify(text)
      if (slug == "") next
      if (slug in seen) {
        seen[slug]++
        print slug "-" seen[slug]
      } else {
        seen[slug] = 0
        print slug
      }
    }
  ' "$1"
}

# anchor_cache FILE -> print (and memoise) the anchor list of FILE. The key is
# the path's MD5, so two destinations cannot share a cache entry (a sanitised
# path would make `a.b.md` and `a-b.md` collide).
anchor_cache() {
  local dest=$1 key cache
  key=$(printf '%s' "$dest" | md5sum)
  key=${key%% *}
  cache="$tmp/anchors/$key"
  if [ ! -f "$cache" ]; then
    mkdir -p -- "$tmp/anchors" || exit 2
    heading_anchors "$dest" >"$cache" || return 1
  fi
  printf '%s\n' "$cache"
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
  case $(page_class "$file") in
    docs) docs_pages=$((docs_pages + 1)) ;;
    root) root_pages=$((root_pages + 1)) ;;
    agent) agent_pages=$((agent_pages + 1)) ;;
  esac
  doc_dir=$(dirname -- "$file")

  # Process substitution, not a pipeline: the counters and failures below are
  # accumulated in this shell, not in a subshell.
  while IFS=$'\t' read -r line_no text; do
    # --- links and heading fragments ---------------------------------------
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

      # External targets are not repository paths.
      case "$target" in
        '//'*) continue ;;
      esac
      if [[ $target =~ ^[A-Za-z][A-Za-z0-9+.-]*: ]]; then
        continue
      fi

      fragment=
      case "$target" in
        *'#'*) fragment=${target#*#} ;;
      esac
      link_path=${target%%#*}
      link_path=${link_path%%\?*}
      link_path=${link_path//%20/ }

      if [ -z "$link_path" ]; then
        # A pure anchor points into the page that contains it.
        resolved=$file
      else
        links_checked=$((links_checked + 1))
        if [[ $link_path == /* ]]; then
          resolved="$repo_root$link_path"
        else
          resolved="$doc_dir/$link_path"
        fi
        if [ ! -e "$resolved" ]; then
          fail "$file" "$line_no" \
            "broken link '$target' (no such file or directory: ${resolved#"$repo_root"/})"
          continue
        fi
      fi

      [ -n "$fragment" ] || continue
      # Only a Markdown destination has headings; a fragment on a directory or
      # a binary file is not an anchor this checker can interpret.
      case "$resolved" in
        *.md) ;;
        *) continue ;;
      esac
      [ -f "$resolved" ] || continue
      anchors_checked=$((anchors_checked + 1))
      anchor_list=$(anchor_cache "$resolved") || continue
      if ! grep -qxF -- "$fragment" "$anchor_list"; then
        fail "$file" "$line_no" \
          "broken heading anchor '#$fragment' in ${resolved#"$repo_root"/} (no heading has that GitHub anchor)"
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
            docs/*)
              fail "$file" "$line_no" \
                "line-suffix citation '$body' is not allowed outside docs/adr/"
              ;;
            *)
              # Root pages and agent instructions cite a source line as an
              # operational anchor (AGENTS.md does); the reader-facing rule
              # above is a docs/ rule, and RQ-022 audits this class.
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

      # ---- directory references --------------------------------------------
      # A path that ends in "/" names a directory, and the file-extension case
      # below cannot see it. This is EVD-056's shape: a moved directory quoted
      # in prose passed every check while the old location was gone. A
      # single-segment name is skipped: `rust/`, `lang/` and `round-33/` are
      # historical proper names or corpus-id prefixes more often than they are
      # repository paths. A URL, an absolute host path and a path the citing
      # line declares absent are not directory claims either.
      case "$ref" in
        */)
          case "$ref" in
            *://*|/*|*\\*) continue ;;
          esac
          dir_ref=${ref%/}
          case "$dir_ref" in
            */*) ;;
            *) continue ;;
          esac
          if ref_documented_absent "${text,,}"; then
            continue
          fi
          refs_checked=$((refs_checked + 1))
          if ! resolve_ref "$ref" "$doc_dir"; then
            fail "$file" "$line_no" \
              "code reference '$body' does not name an existing directory"
          fi
          continue
          ;;
      esac

      case "$ref" in
        */*.rs|*/*.toml|*/*.yml|*/*.yaml|*/*.json|*/*.md) ;;
        *) continue ;;
      esac

      # A file the citing line declares deleted/removed is a removal record, not
      # a claim that the file still exists. The negative case (an undeclared
      # missing path) still fails below.
      if ref_documented_absent "${text,,}"; then
        continue
      fi

      refs_checked=$((refs_checked + 1))
      if ! resolve_ref "$ref" "$doc_dir"; then
        fail "$file" "$line_no" "code reference '$body' does not name an existing file"
      fi
    done

    # ---- retired claim vocabulary (--claims) ------------------------------
    # Measured once per line on the raw text, inline code span included: a
    # backticked use still teaches the retired term. The archived trees never
    # reach this loop (is_current_doc excludes them), and docs/adr/ may quote
    # it as history; retired_term_allowed is the only exception.
    if [ "$claims" -eq 1 ]; then
      lower_text=${text,,}
      if [[ $lower_text == *precision-bounded* ]] &&
        ! retired_term_allowed "$file" "$lower_text"; then
        retired_hits=$((retired_hits + 1))
        fail "$file" "$line_no" \
          "retired term 'precision-bounded' (use 'precision-limited'; the four claim kinds are exact, precision-limited, error-bounded and parity-verified)"
      fi
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
      print NR "\t" $0
    }
  ' "$file")
done < <(
  {
    find docs -type f -name '*.md' -print0
    for page in README.md AGENTS.md CONTRACT.md .github/copilot-instructions.md; do
      [ -f "$page" ] && printf '%s\0' "$page"
    done
    if [ -d .dsh/skills ]; then
      find .dsh/skills -type f -name 'SKILL.md' -print0
    fi
  } | sort -z
)

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
          "prohibited numerical claim '$claim_phrase' (label it exact, precision-limited, error-bounded or parity-verified; see docs/editorial.md)"
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
        "missing '## Precision' section (every tutorial labels its values exact, precision-limited, error-bounded or parity-verified)"
    else
      precision_sections=$((precision_sections + 1))
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
  readme_requirements=4
  check_readme
fi

if [ "$files_checked" -eq 0 ]; then
  printf 'check-docs: no current Markdown files found under docs/\n' >&2
  exit 2
fi

if [ "$symbols" -eq 1 ]; then
  printf 'check-docs: %d Markdown files (%d docs/, %d root, %d agent instructions), %d relative links, %d heading anchors, %d code references, %d line-suffix citations, %d symbol citations\n' \
    "$files_checked" "$docs_pages" "$root_pages" "$agent_pages" \
    "$links_checked" "$anchors_checked" "$refs_checked" "$citations_checked" "$symbols_checked"
else
  printf 'check-docs: %d Markdown files (%d docs/, %d root, %d agent instructions), %d relative links, %d heading anchors, %d code references\n' \
    "$files_checked" "$docs_pages" "$root_pages" "$agent_pages" \
    "$links_checked" "$anchors_checked" "$refs_checked"
fi

if [ "$claims" -eq 1 ]; then
  printf 'check-docs: claims audit: %d pages under docs/, %d tutorial pages, %d prohibited phrase(s), %d retired-term use(s)\n' \
    "$claim_pages" "$tutorials_checked" "$claim_hits" "$retired_hits"
fi
if [ "$readme" -eq 1 ]; then
  printf 'check-docs: README.md structural audit: 4 requirements checked\n'
fi

# ---------------------------------------------------------------------------
# The machine-readable evidence line (RQ-023). It is additive: the human
# summary above and the exit status below keep their meaning, and this line is
# always the last thing on stdout. `modes` lists the audits that ran on top of
# the link/heading/reference scan, so three invocations of this gate are
# distinguishable in tools/docs-report.sh.
# ---------------------------------------------------------------------------
modes=none
if [ "$symbols" -eq 1 ] || [ "$claims" -eq 1 ] || [ "$readme" -eq 1 ]; then
  modes=
  [ "$symbols" -eq 1 ] && modes="${modes}symbols,"
  [ "$claims" -eq 1 ] && modes="${modes}claims,"
  [ "$readme" -eq 1 ] && modes="${modes}readme,"
  modes=${modes%,}
fi

docreport_status=ok
if [ "$failures" -gt 0 ]; then
  printf 'check-docs: FAIL (%d failure(s))\n' "$failures"
  docreport_status=fail
else
  printf 'check-docs: OK\n'
fi

printf 'DOCREPORT gate=check-docs status=%s pages=%d links=%d anchors=%d refs=%d citations=%d symbols=%d claims=%d tutorials=%d sections=%d readme=%d modes=%s failures=%d\n' \
  "$docreport_status" "$files_checked" "$links_checked" "$anchors_checked" \
  "$refs_checked" "$citations_checked" "$symbols_checked" "$claim_hits" \
  "$tutorials_checked" "$precision_sections" "$readme_requirements" \
  "$modes" "$failures"

if [ "$docreport_status" = fail ]; then
  exit 1
fi
exit 0
