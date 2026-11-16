#!/usr/bin/env bash
# Runs every ```lovelace block in the docs/language pages through
# `lovelace-run --file <tmp> --text` and compares the normalised result with
# the ```text block that follows it.
#
# This is the bash equivalent of run-examples.ps1 (kept in the tree for
# Windows PowerShell users). It reproduces the fenced-example check: the
# builtins catalogue cross-check against `--functions` remains in the
# PowerShell script.
#
# Usage (from the repository root, or any directory):
#   bash docs/language/examples/run-examples.sh
#
# The runner is looked up at
#   ${CARGO_TARGET_DIR:-<repo>/target}/{debug,release}/lovelace-run
# and, only on Windows (Git Bash / MSYS / Cygwin), also with a .exe suffix.
# Nothing is written inside the repository: every scratch file lives in a
# mktemp directory that is removed on exit.
#
# Exit status: 0 when every example matches, 1 on an example mismatch or a
# malformed page (missing/unterminated fence, missing expected block).
set -uo pipefail

script_dir=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
docs_dir=$repo/docs/language

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

scratch=$(mktemp -d "${TMPDIR:-/tmp}/lovelace-language-examples.XXXXXX") ||
    die 'could not create a temporary directory'
cleanup() { rm -rf -- "$scratch"; }
trap cleanup EXIT HUP INT TERM

mkdir -p "$scratch/tmp" "$scratch/plots" "$scratch/appdata"
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

# stdin -> stdout: drop CR, collapse runs of whitespace inside each line,
# trim each line, drop leading/trailing blank lines and join with newlines.
# This is Normalise-Text from the PowerShell script.
normalise_text() {
    awk '
        { gsub(/[[:space:]]+/, " "); sub(/^ /, ""); sub(/ $/, ""); line[NR] = $0 }
        END {
            first = 1; last = NR
            while (first <= last && line[first] == "") first++
            while (last >= first && line[last] == "") last--
            for (i = first; i <= last; i++) {
                if (i > first) printf "\n"
                printf "%s", line[i]
            }
        }'
}

# The pages the PowerShell runner covers, in the same order.
pages=(
    README.md
    lexical-structure.md
    values-and-types.md
    expressions-and-operators.md
    statements-and-control-flow.md
    functions-and-scope.md
    builtins.md
    errors-and-diagnostics.md
    precision-and-formatting.md
    plots.md
)

FENCE='```'
total=0
failed=0
case_no=0

for page in "${pages[@]}"; do
    page_path=$docs_dir/$page
    if [[ ! -f $page_path || ! -r $page_path ]]; then
        die "missing or unreadable page: docs/language/$page"
    fi

    page_lines=()
    while IFS= read -r line || [[ -n $line ]]; do
        page_lines+=("${line%$'\r'}")
    done <"$page_path"
    line_count=${#page_lines[@]}

    passed_here=0
    i=0
    while (( i < line_count )); do
        if [[ ${page_lines[i]} =~ ^${FENCE}lovelace[[:space:]]*$ ]]; then
            fence_line=$((i + 1))

            # --- the script block -----------------------------------------
            j=$((i + 1))
            while (( j < line_count )) && [[ ! ${page_lines[j]} =~ ^${FENCE}[[:space:]]*$ ]]; do
                j=$((j + 1))
            done
            if (( j >= line_count )); then
                die "docs/language/$page:$fence_line: unterminated lovelace fence"
            fi
            source=
            sep=
            for (( k = i + 1; k < j; k++ )); do
                source+=$sep${page_lines[k]}
                sep=$'\n'
            done

            # --- the expected text block ----------------------------------
            k=$((j + 1))
            while (( k < line_count )) && [[ ${page_lines[k]} =~ ^[[:space:]]*$ ]]; do
                k=$((k + 1))
            done
            if (( k >= line_count )); then
                die "docs/language/$page:$fence_line: lovelace block has no expected text block"
            fi
            if [[ ! ${page_lines[k]} =~ ^${FENCE}text[[:space:]]*$ ]]; then
                die "docs/language/$page:$fence_line: expected a \`\`\`text block, found '${page_lines[k]}'"
            fi
            m=$((k + 1))
            while (( m < line_count )) && [[ ! ${page_lines[m]} =~ ^${FENCE}[[:space:]]*$ ]]; do
                m=$((m + 1))
            done
            if (( m >= line_count )); then
                die "docs/language/$page:$((k + 1)): unterminated text fence"
            fi
            expected=
            sep=
            for (( x = k + 1; x < m; x++ )); do
                expected+=$sep${page_lines[x]}
                sep=$'\n'
            done

            # --- run it ---------------------------------------------------
            case_no=$((case_no + 1))
            total=$((total + 1))
            lv_file=$scratch/case-$case_no.lv
            stdout_file=$scratch/case-$case_no.out
            stderr_file=$scratch/case-$case_no.err
            printf '%s' "$source" >"$lv_file"
            : >"$stdout_file"
            : >"$stderr_file"

            if "$exe" --file "$(native_path "$lv_file")" --text \
                --plot-dir "$(native_path "$scratch/plots")" \
                --plot-file "case-$case_no.svg" \
                >"$stdout_file" 2>"$stderr_file"; then
                rc=0
            else
                rc=$?
            fi
            rm -f -- "$lv_file"

            if (( rc == 0 )); then
                raw=$(cat "$stdout_file")
            else
                raw=$(cat "$stderr_file")
            fi
            want=$(printf '%s' "$expected" | normalise_text)
            got=$(printf '%s' "$raw" | normalise_text)

            if [[ $got == "$want" ]]; then
                passed_here=$((passed_here + 1))
            else
                failed=$((failed + 1))
                printf '\nFAIL docs/language/%s:%d (exit %d): output mismatch\n' \
                    "$page" "$fence_line" "$rc"
                printf '  expected:\n'
                printf '%s\n' "$want" | sed 's/^/    /'
                printf '  observed:\n'
                printf '%s\n' "$got" | sed 's/^/    /'
                if (( rc == 0 )) && [[ -s $stderr_file ]]; then
                    printf '  stderr:\n'
                    cat "$stderr_file" | normalise_text | sed 's/^/    /'
                    printf '\n'
                fi
            fi

            i=$m
            continue
        fi
        i=$((i + 1))
    done

    printf '%s: %d example(s) passed\n' "$page" "$passed_here"
done

if (( failed > 0 )); then
    printf '\n%d of %d example(s) failed\n' "$failed" "$total"
    exit 1
fi

printf '\nAll %d examples passed.\n' "$total"
exit 0
