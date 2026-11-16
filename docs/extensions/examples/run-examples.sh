#!/usr/bin/env bash
# Executes every ```lovelace block in docs/extensions/*.md and checks it
# against the ```text block that follows it.
#
# This is the bash equivalent of run-examples.ps1 (kept in the tree for
# Windows PowerShell users). For each pair the snippet is written to a
# temporary .lv file and evaluated with
#
#     lovelace-run --file <path> --text --omit-variables
#
# The combined stdout/stderr is compared with the documented block after
# whitespace normalisation (runs of whitespace collapse to one space, then
# trim). A snippet that is meant to fail still passes: the error line is its
# observed output, and the process exit code is reported but not compared.
#
# Usage (from the repository root, or any directory):
#   bash docs/extensions/examples/run-examples.sh
#
# The runner is looked up at
#   ${CARGO_TARGET_DIR:-<repo>/target}/{debug,release}/lovelace-run
# and, only on Windows (Git Bash / MSYS / Cygwin), also with a .exe suffix.
# Nothing is written inside the repository: every scratch file lives in a
# mktemp directory that is removed on exit.
#
# Exit status: 0 when every paired example matches, 1 when any example
# mismatches or a lovelace block has no following text block, 2 when no
# examples were found at all. A malformed page (unterminated fence, no pages)
# is a fatal error, not a silently skipped page.
set -uo pipefail

script_dir=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
docs_dir=$repo/docs/extensions

die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

# Windows needs the .exe suffix; no other platform may fall back to it.
case "$(uname -s 2>/dev/null || true)" in
    MINGW* | MSYS* | CYGWIN*) is_windows=1 ;;
    *) is_windows=0 ;;
esac
if (( is_windows )); then exe_suffix=.exe; else exe_suffix=; fi

if [[ -n ${CARGO_TARGET_DIR:-} ]]; then
    target_dir=$CARGO_TARGET_DIR
    case $target_dir in
        /* | [A-Za-z]:[\\/]*) : ;;
        *) target_dir=$repo/$target_dir ;;
    esac
else
    target_dir=$repo/target
fi

exe=
for profile in debug release; do
    candidate=$target_dir/$profile/lovelace-run$exe_suffix
    if [[ -f $candidate ]]; then
        exe=$candidate
        break
    fi
done
if [[ -z $exe ]]; then
    die "lovelace-run not found under '$target_dir/{debug,release}'. Build it first with: cargo build -p lovelace-run"
fi
if (( ! is_windows )) && [[ ! -x $exe ]]; then
    die "lovelace-run is not executable: $exe"
fi

scratch=$(mktemp -d "${TMPDIR:-/tmp}/lovelace-extensions-examples.XXXXXX") ||
    die 'could not create a temporary directory'
cleanup() { rm -rf -- "$scratch"; }
trap cleanup EXIT HUP INT TERM

mkdir -p "$scratch/tmp" "$scratch/appdata"
export TMPDIR=$scratch/tmp TEMP=$scratch/tmp TMP=$scratch/tmp
export XDG_CONFIG_HOME=$scratch/appdata APPDATA=$scratch/appdata

# MSYS translates POSIX paths for native binaries, but not for environment
# values and not consistently for every argument; hand the child a native path
# when cygpath exists (Windows), a POSIX path everywhere else.
native_path() {
    if command -v cygpath >/dev/null 2>&1; then
        cygpath -w -- "$1"
    else
        printf '%s' "$1"
    fi
}

# stdin -> stdout: collapse every run of whitespace (including newlines) to a
# single space, then trim. This is ConvertTo-Normalized from the PowerShell
# script.
normalise_text() {
    tr -s '[:space:]' ' ' | sed -e 's/^ //' -e 's/ $//'
}

pages=()
for page_path in "$docs_dir"/*.md; do
    [[ -e $page_path ]] && pages+=("$page_path")
done
if (( ${#pages[@]} == 0 )); then
    die "no pages found in docs/extensions"
fi

FENCE='```'
checked=0
passed=0
failed=0
unpaired=()

for page_path in "${pages[@]}"; do
    page_name=${page_path##*/}
    if [[ ! -r $page_path ]]; then
        die "cannot read page: docs/extensions/$page_name"
    fi

    page_lines=()
    while IFS= read -r line || [[ -n $line ]]; do
        page_lines+=("${line%$'\r'}")
    done <"$page_path"
    line_count=${#page_lines[@]}

    # Split the page into fenced blocks: language, content, first content line.
    block_lang=()
    block_content=()
    block_line=()
    in_fence=0
    lang=
    buffer=
    sep=
    start_line=0
    open_line=0
    for (( n = 0; n < line_count; n++ )); do
        line=${page_lines[n]}
        if (( !in_fence )); then
            if [[ $line =~ ^${FENCE}[[:space:]]*([^[:space:]]*)[[:space:]]*$ ]]; then
                in_fence=1
                lang=$(printf '%s' "${BASH_REMATCH[1]}" | tr '[:upper:]' '[:lower:]')
                buffer=
                sep=
                start_line=$((n + 2)) # 1-based line number of the first content line
                open_line=$((n + 1))
            fi
        else
            if [[ $line =~ ^${FENCE}[[:space:]]*$ ]]; then
                in_fence=0
                block_lang+=("$lang")
                block_content+=("$buffer")
                block_line+=("$start_line")
            else
                buffer+=$sep$line
                sep=$'\n'
            fi
        fi
    done
    if (( in_fence )); then
        die "docs/extensions/$page_name:$open_line: unterminated fence"
    fi

    block_count=${#block_lang[@]}
    for (( idx = 0; idx < block_count; idx++ )); do
        if [[ ${block_lang[idx]} != lovelace ]]; then
            continue
        fi
        source=${block_content[idx]}
        start_line=${block_line[idx]}

        if (( idx + 1 < block_count )) && [[ ${block_lang[idx + 1]} == text ]]; then
            expected=${block_content[idx + 1]}
        else
            unpaired+=("$page_name:$start_line")
            continue
        fi

        checked=$((checked + 1))
        snippet=$scratch/snippet-$checked.lv
        stdout_file=$scratch/snippet-$checked.out
        stderr_file=$scratch/snippet-$checked.err
        printf '%s' "$source" >"$snippet"
        : >"$stdout_file"
        : >"$stderr_file"

        if "$exe" --file "$(native_path "$snippet")" --text --omit-variables \
            >"$stdout_file" 2>"$stderr_file"; then
            rc=0
        else
            rc=$?
        fi
        rm -f -- "$snippet"

        combined=$(printf '%s\n%s' "$(cat "$stdout_file")" "$(cat "$stderr_file")")
        want=$(printf '%s' "$expected" | normalise_text)
        got=$(printf '%s' "$combined" | normalise_text)

        if [[ $got == "$want" ]]; then
            passed=$((passed + 1))
            printf 'PASS  %s:%d (exit %d)\n' "$page_name" "$start_line" "$rc"
        else
            failed=$((failed + 1))
            printf 'FAIL  %s:%d (exit %d)\n' "$page_name" "$start_line" "$rc"
            printf '      expected: %s\n' "$want"
            printf '      observed: %s\n' "$got"
        fi
    done
done

if (( ${#unpaired[@]} > 0 )); then
    printf '\nlovelace blocks with no following text block:\n'
    for entry in "${unpaired[@]}"; do
        printf '  %s\n' "$entry"
    done
fi

printf '\npages: %d   examples checked: %d   passed: %d   failed: %d   unpaired: %d\n' \
    "${#pages[@]}" "$checked" "$passed" "$failed" "${#unpaired[@]}"

if (( checked == 0 )); then
    printf 'No examples were found; nothing was verified.\n'
    exit 2
fi
if (( failed > 0 || ${#unpaired[@]} > 0 )); then
    exit 1
fi
printf 'OK: %d examples match the documented output.\n' "$checked"
exit 0
