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
# subsection of one of the seven. A page that is not listed is not checked.
#
# Usage:
#   bash tools/check-reference-pages.sh
#   bash tools/check-reference-pages.sh --help
#
# A defect is printed as `path: section`, where the section is the missing,
# out-of-order or unexpected heading. Exit status:
#   0  every listed page conforms
#   1  at least one page does not conform
#   2  bad usage, a missing manifest, or no pages listed
#
# The script needs only bash, awk, mktemp and basic coreutils, so it runs in the
# ubuntu-latest and windows-latest (Git Bash) docs jobs. It is deliberately
# CRLF-tolerant: a Windows checkout may hand it CRLF Markdown.

set -u

usage() {
    printf 'usage: bash tools/check-reference-pages.sh\n'
    printf '  reads docs/reference-pages.md and checks every listed page for the\n'
    printf '  seven reference sections, in order, with no other level-2 heading\n'
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

work=$(mktemp -d "${TMPDIR:-/tmp}/check-reference-pages.XXXXXX") || exit 2
trap 'rm -rf "$work"' EXIT HUP INT TERM

required=(Purpose Inputs Outputs 'Precision behaviour' Errors Limitations Examples)
required_count=${#required[@]}

# The manifest is the gate's input: the first backticked token of every table
# row in the "## Manifest" section. The section ends at the next level-2
# heading, so the "Not in scope" table can never be mistaken for the manifest.
awk '
    { sub(/\r$/, "") }
    /^## Manifest[[:space:]]*$/ { in_manifest = 1; next }
    in_manifest && /^## / { in_manifest = 0 }
    in_manifest && /^\|/ {
        if (match($0, /`[^`]*`/)) {
            token = substr($0, RSTART + 1, RLENGTH - 2)
            if (token ~ /^docs\/.*\.md$/ && !seen[token]++) print token
        }
    }
' "$manifest" >"$work/pages"

pages=()
while IFS= read -r page; do
    [ -n "$page" ] && pages+=("$page")
done <"$work/pages"

if [ "${#pages[@]}" -eq 0 ]; then
    printf 'check-reference-pages: no pages found in the "## Manifest" table of %s\n' \
        "$manifest" >&2
    exit 2
fi

failures=0
failed_pages=0
sections_checked=0
pages_checked=0

fail() {
    # $1 page, $2 message
    printf '%s: %s\n' "$1" "$2" >&2
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

for page in "${pages[@]}"; do
    pages_checked=$((pages_checked + 1))

    if [ ! -f "$page" ]; then
        fail "$page" '(listed in docs/reference-pages.md, but the file does not exist)'
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
                print heading
            }
        }
    ' "$page" >"$work/headings"

    seen=()
    for ((i = 0; i < required_count; i++)); do
        seen[i]=0
    done
    last_position=-1
    page_failed=0

    while IFS= read -r heading; do
        [ -z "$heading" ] && continue

        if ! is_required "$heading"; then
            fail "$page" "$heading (unexpected level-2 heading; nest it under one of the seven)"
            page_failed=1
            continue
        fi

        position=-1
        for ((i = 0; i < required_count; i++)); do
            if [ "${required[i]}" = "$heading" ]; then
                position=$i
                break
            fi
        done

        if [ "${seen[position]}" -eq 1 ]; then
            fail "$page" "$heading (duplicate level-2 heading)"
            page_failed=1
            continue
        fi
        seen[position]=1

        if [ "$position" -lt "$last_position" ]; then
            fail "$page" "$heading (out of order; it must come after ${required[last_position]})"
            page_failed=1
        fi
        if [ "$position" -gt "$last_position" ]; then
            last_position=$position
        fi
    done <"$work/headings"

    for ((i = 0; i < required_count; i++)); do
        if [ "${seen[i]}" -eq 0 ]; then
            fail "$page" "${required[i]}"
            page_failed=1
        fi
    done

    if [ "$page_failed" -eq 0 ]; then
        sections_checked=$((sections_checked + required_count))
    else
        failed_pages=$((failed_pages + 1))
    fi
done

if [ "$failures" -gt 0 ]; then
    printf 'check-reference-pages: %d page(s) checked, %d conform, %d failed (%d defect(s))\n' \
        "$pages_checked" $((pages_checked - failed_pages)) "$failed_pages" "$failures"
    printf 'check-reference-pages: FAIL\n'
    exit 1
fi

printf 'check-reference-pages: %d page(s), %d required sections in order\n' \
    "$pages_checked" "$((pages_checked * required_count))"
printf 'check-reference-pages: OK\n'
exit 0
