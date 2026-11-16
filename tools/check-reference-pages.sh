#!/usr/bin/env bash
# check-reference-pages.sh - the DOC-014 reference-page gate.
#
# Every page listed in the "## Manifest" table of docs/reference-pages.md must
# carry the seven sections of docs/reference-template.md, at level 2, with
# exactly these names and in exactly this order:
#
#     Purpose, Inputs, Outputs, Precision behaviour, Errors, Limitations, Examples
#
# No other level-2 heading is allowed on a listed page: everything else is a
# subsection of one of the seven. A required section with no content before the
# next level-2 heading is a defect too: a heading that introduces nothing is
# not a section.
#
# The manifest is two-way. The three reference families - docs/language/,
# docs/extensions/ and docs/crates/ - are the scoped trees, and every Markdown
# page anywhere under them must be classified exactly once:
#   * in the "## Manifest" table, as a reference page that carries the seven
#     sections; or
#   * in the "## Not in scope" table, as a declared non-reference page with a
#     reason. A page declared there must still exist: a stale exemption would
#     silently let a future reference page through.
# A page in neither table is an unclassified page and fails. A page in both is
# a contradiction and fails. A manifested page that does not exist fails. The
# seven-section contract keeps the 16 pages currently manifested.
#
# Usage:
#   bash tools/check-reference-pages.sh
#   bash tools/check-reference-pages.sh --help
#
# Every diagnostic is printed as `file:line: message`: the page and heading
# line for a section defect, or docs/reference-pages.md and the table row for a
# manifest defect. Exit status:
#   0  every scoped page is classified and every listed page conforms
#   1  at least one page is unclassified, missing, or does not conform
#   2  bad usage, a missing manifest, no pages listed, or a missing scoped tree
#
# The script needs only bash, awk, find, sed, grep, sort, mktemp and basic
# coreutils, so it runs in the ubuntu-latest and windows-latest (Git Bash) docs
# jobs. It is deliberately CRLF-tolerant: a Windows checkout may hand it CRLF
# Markdown.

set -u

usage() {
    printf 'usage: bash tools/check-reference-pages.sh\n'
    printf '  reads docs/reference-pages.md and checks every scoped page:\n'
    printf '  manifested pages carry the seven sections, in order, non-empty;\n'
    printf '  every page under docs/language/, docs/extensions/ or docs/crates/\n'
    printf '  is manifested or declared non-reference with a reason\n'
}

for arg in "$@"; do
    case "$arg" in
        -h | --help)
            usage
            exit 0
            ;;
        *)
            printf 'check-reference-pages: unknown argument: %s\n' "$arg" >&2
            usage >&2
            exit 2
            ;;
    esac
done

LC_ALL=C
export LC_ALL

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd) || exit 2
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd) || exit 2
cd -- "$repo_root" || exit 2

manifest=docs/reference-pages.md
if [ ! -f "$manifest" ]; then
    printf 'check-reference-pages: no manifest at %s\n' "$manifest" >&2
    exit 2
fi

scoped_trees=(docs/language docs/extensions docs/crates)
for tree in "${scoped_trees[@]}"; do
    if [ ! -d "$tree" ]; then
        printf 'check-reference-pages: missing scoped tree %s\n' "$tree" >&2
        exit 2
    fi
done

work=$(mktemp -d "${TMPDIR:-/tmp}/check-reference-pages.XXXXXX") || exit 2
trap 'rm -rf "$work"' EXIT HUP INT TERM

required=(Purpose Inputs Outputs 'Precision behaviour' Errors Limitations Examples)
required_count=${#required[@]}

# A table row is a page row when it contains a backticked `docs/*.md` path.
# The section ends at the next level-2 heading, so a page mentioned in the
# prose of "## Not in scope" is not read as a row.
table_rows() {
    # $1 section name, $2 "path" or "path-reason"
    awk -v wanted="$1" -v mode="$2" '
        { sub(/\r$/, "") }
        $0 ~ "^## " wanted "[[:space:]]*$" { inside = 1; next }
        inside && /^## / { inside = 0 }
        inside && /^\|/ {
            if (!match($0, /`[^`]*`/)) next
            token = substr($0, RSTART + 1, RLENGTH - 2)
            if (token !~ /^docs\/.*\.md$/) next
            if (mode == "path") {
                print NR "\t" token
                next
            }
            row = $0
            sub(/^[[:space:]]*\|/, "", row)
            sub(/[[:space:]]*\|[[:space:]]*$/, "", row)
            cells = split(row, part, "|")
            # The reason is the second cell of the "Page | Why" table. A pipe
            # inside a reason would split it, but the cell is still non-empty,
            # so the row is not falsely rejected.
            reason = (cells >= 2) ? part[2] : ""
            gsub(/^[[:space:]]+|[[:space:]]+$/, "", reason)
            print NR "\t" token "\t" reason
        }
    ' "$manifest"
}

table_rows Manifest path >"$work/manifest.rows"
table_rows 'Not in scope' path-reason >"$work/nonref.rows"

manifest_pages=()
manifest_lines=()
while IFS=$'\t' read -r line page; do
    [ -n "$page" ] || continue
    manifest_lines+=("$line")
    manifest_pages+=("$page")
done <"$work/manifest.rows"

nonref_pages=()
nonref_lines=()
nonref_reasons=()
while IFS=$'\t' read -r line page reason; do
    [ -n "$page" ] || continue
    nonref_lines+=("$line")
    nonref_pages+=("$page")
    nonref_reasons+=("$reason")
done <"$work/nonref.rows"

if [ "${#manifest_pages[@]}" -eq 0 ]; then
    printf 'check-reference-pages: no pages found in the "## Manifest" table of %s\n' \
        "$manifest" >&2
    exit 2
fi

failures=0
failed_pages=0
sections_checked=0
pages_checked=0
nonref_checked=0
unclassified=0

fail() {
    # $1 file, $2 line, $3 message
    printf '%s:%s: %s\n' "$1" "$2" "$3" >&2
    failures=$((failures + 1))
}

is_required() {
    case "$1" in
        Purpose | Inputs | Outputs | 'Precision behaviour' | Errors | Limitations | Examples)
            return 0
            ;;
        *) return 1 ;;
    esac
}

list_contains() {
    # $1 needle, remaining arguments: the list
    local needle=$1 item
    shift
    for item in "$@"; do
        [ "$item" = "$needle" ] && return 0
    done
    return 1
}

# ---------------------------------------------------------------------------
# The declared non-reference list: every entry names a page that still exists
# and carries a reason. An exemption without a reason, or for a file that no
# longer exists, is a defect in the manifest rather than a free pass.
# ---------------------------------------------------------------------------
for ((i = 0; i < ${#nonref_pages[@]}; i++)); do
    page=${nonref_pages[i]}
    line=${nonref_lines[i]}
    reason=${nonref_reasons[i]}
    nonref_checked=$((nonref_checked + 1))
    if [ -z "$reason" ]; then
        fail "$manifest" "$line" \
            "non-reference page '$page' has no reason (say why it is not a reference page)"
    fi
    if [ ! -f "$page" ]; then
        fail "$manifest" "$line" \
            "declared non-reference page '$page' does not exist (remove or retarget the exemption)"
    fi
    if list_contains "$page" "${manifest_pages[@]}"; then
        fail "$manifest" "$line" \
            "page '$page' is both manifested and declared non-reference"
    fi
done

# ---------------------------------------------------------------------------
# Every Markdown page under the scoped trees must be classified. This is the
# direction the single-way manifest was blind to: a new callable-surface page
# used to pass by simply not being listed.
# ---------------------------------------------------------------------------
{
    for tree in "${scoped_trees[@]}"; do
        find "$tree" -type f -name '*.md' -print0
    done
} | sort -z >"$work/scoped.z"

scoped_total=0
while IFS= read -r -d '' page; do
    scoped_total=$((scoped_total + 1))
    if list_contains "$page" "${manifest_pages[@]}"; then
        continue
    fi
    if list_contains "$page" "${nonref_pages[@]}"; then
        continue
    fi
    unclassified=$((unclassified + 1))
    fail "$page" 1 \
        "page is not classified in $manifest (add a '## Manifest' row, or a '## Not in scope' row with a reason)"
done <"$work/scoped.z"

# ---------------------------------------------------------------------------
# The seven-section contract, one listed page at a time.
# ---------------------------------------------------------------------------
for ((p = 0; p < ${#manifest_pages[@]}; p++)); do
    page=${manifest_pages[p]}
    listed_line=${manifest_lines[p]}
    pages_checked=$((pages_checked + 1))

    if [ ! -f "$page" ]; then
        fail "$manifest" "$listed_line" \
            "listed reference page '$page' does not exist"
        failed_pages=$((failed_pages + 1))
        continue
    fi

    # Level-2 headings, outside fenced code blocks (an example may contain a
    # line that looks like a heading, and a page's fenced examples are opaque).
    awk '
        { sub(/\r$/, "") }
        {
            if (match($0, /^[ \t]*(```+|~~~+)/)) {
                marker = substr($0, RSTART, RLENGTH)
                sub(/^[ \t]*/, "", marker)
                if (fence == "") { fence = marker; next }
                if (substr(marker, 1, 1) == substr(fence, 1, 1)) { fence = ""; next }
            }
            if (fence != "") next
            if (match($0, /^##[ \t]+/)) {
                heading = substr($0, RLENGTH + 1)
                sub(/[ \t]+#+[ \t]*$/, "", heading)
                sub(/[ \t]+$/, "", heading)
                print NR "\t" heading
            }
        }
    ' "$page" >"$work/headings"

    l2_lines=()
    l2_names=()
    while IFS=$'\t' read -r lineno heading; do
        [ -z "$heading" ] && continue
        l2_lines+=("$lineno")
        l2_names+=("$heading")
    done <"$work/headings"

    seen=()
    for ((i = 0; i < required_count; i++)); do
        seen[i]=0
    done
    last_position=-1
    page_failed=0

    for ((i = 0; i < ${#l2_names[@]}; i++)); do
        heading=${l2_names[i]}
        lineno=${l2_lines[i]}

        if ! is_required "$heading"; then
            fail "$page" "$lineno" \
                "unexpected level-2 heading '$heading' (nest it under one of the seven)"
            page_failed=1
            continue
        fi

        position=-1
        for ((j = 0; j < required_count; j++)); do
            if [ "${required[j]}" = "$heading" ]; then
                position=$j
                break
            fi
        done

        if [ "${seen[position]}" -eq 1 ]; then
            fail "$page" "$lineno" "duplicate level-2 heading '$heading'"
            page_failed=1
            continue
        fi
        seen[position]=1

        if [ "$position" -lt "$last_position" ]; then
            fail "$page" "$lineno" \
                "'$heading' is out of order; it must come after ${required[last_position]}"
            page_failed=1
        fi
        if [ "$position" -gt "$last_position" ]; then
            last_position=$position
        fi

        # A required heading must introduce something before the next level-2
        # heading. The body may be prose, a list, a table or a fenced example;
        # a subsection at level 3 or deeper counts as content because it is
        # nested under this section.
        if [ $((i + 1)) -lt ${#l2_names[@]} ]; then
            body_end=${l2_lines[i + 1]}
        else
            body_end=$(( $(awk 'END { print NR }' "$page") + 1 ))
        fi
        if [ "$body_end" -le $((lineno + 1)) ]; then
            body_empty=1
        elif sed -n "$((lineno + 1)),$((body_end - 1))p" -- "$page" |
            grep -q '[^[:space:]]'; then
            body_empty=0
        else
            body_empty=1
        fi
        if [ "$body_empty" -eq 1 ]; then
            fail "$page" "$lineno" \
                "required section '$heading' is empty (no content before the next level-2 heading)"
            page_failed=1
        fi
    done

    for ((i = 0; i < required_count; i++)); do
        if [ "${seen[i]}" -eq 0 ]; then
            fail "$page" 1 "missing required section '${required[i]}'"
            page_failed=1
        fi
    done

    if [ "$page_failed" -eq 0 ]; then
        sections_checked=$((sections_checked + required_count))
    else
        failed_pages=$((failed_pages + 1))
    fi
done

docreport_status=ok
if [ "$failures" -gt 0 ]; then
    printf 'check-reference-pages: %d scoped page(s) (%d manifested, %d declared non-reference, %d unclassified), %d defect(s)\n' \
        "$scoped_total" "$pages_checked" "$nonref_checked" "$unclassified" "$failures"
    printf 'check-reference-pages: %d manifested page(s) conform, %d failed\n' \
        "$((pages_checked - failed_pages))" "$failed_pages"
    printf 'check-reference-pages: FAIL\n'
    docreport_status=fail
else
    printf 'check-reference-pages: %d scoped page(s): %d manifested, %d declared non-reference, every page classified\n' \
        "$scoped_total" "$pages_checked" "$nonref_checked"
    printf 'check-reference-pages: %d required sections in order and non-empty\n' "$sections_checked"
    printf 'check-reference-pages: OK\n'
fi

# The machine-readable evidence line (RQ-023), additive and last on stdout.
printf 'DOCREPORT gate=check-reference-pages status=%s pages=%d manifested=%d non_reference=%d unclassified=%d sections=%d failed_pages=%d failures=%d\n' \
    "$docreport_status" "$scoped_total" "$pages_checked" "$nonref_checked" \
    "$unclassified" "$sections_checked" "$failed_pages" "$failures"

if [ "$docreport_status" = fail ]; then
    exit 1
fi
exit 0
