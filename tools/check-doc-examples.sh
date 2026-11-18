#!/usr/bin/env bash
# check-doc-examples.sh - execute every documented Lovelace snippet against the built runner.
#
# Usage: bash tools/check-doc-examples.sh
#
# Why this exists: the family runners cover docs/language, docs/extensions and
# the tutorials, but a guide page that is not in one of those trees (for
# example docs/systems-analysis.md) had no gate at all, so a documented call
# could stop working in the current language and nothing failed. This gate is
# the backstop over every docs page: it extracts each ```lovelace fence, runs
# it through the lovelace-run built from this tree, and compares the observed
# output with the ```text block that must immediately follow it.
#
# The comparison rule is the one the documented output follows:
#   * a page under docs/language documents the runner's default `--text`
#     output, which includes the trailing `  _ = ...` variable line, so those
#     pages run with `--text`;
#   * every other page documents the result line only, so it runs with
#     `--text --omit-variables`, the same invocation the extension runner uses.
#   * a successful run (exit 0) is compared on stdout; a documented refusal
#     (exit 1) is compared on stderr, because that is where the runner prints
#     the typed error the documented block shows. Any other exit code is an
#     unexpected runner state and is a failure whatever it printed.
#   * whitespace runs are collapsed and the ends trimmed before comparison, so
#     indentation is not part of the contract.
#   * every run gets a scratch --plot-dir/--plot-file, so a documented plot
#     writes outside the repository and cannot make the gate depend on the
#     caller's application directory.
#
# Coverage and exclusions. Every Markdown page under docs/ is scanned except
# these trees, and each exclusion is printed on every run with its reason:
#   docs/internal/archive/  historical harness records, not current docs
#   docs/csharp-oracle/     carried fixtures
#   docs/goal-*/            harness working memory
#   docs/tutorials/         tools/run-tutorials.sh runs the checked-in scripts
#                           as contracts; those pages duplicate them and their
#                           snippet has no adjacent output block
#
# A ```lovelace fence with no immediately following ```text block is a
# failure, never a silent skip: a snippet whose output is not recorded is not
# evidence. A snippet that genuinely cannot run must be marked on the line
# immediately above its fence:
#
#     <!-- not-run: <reason> -->
#
# An exempted snippet is not executed and is counted as `exempted`, separately
# from `discovered` and `executed`. Every exemption is printed with page:line
# and reason, so the list is auditable; an annotation that is not attached to
# its lovelace fence, that has an empty reason, or that is not immediately
# above the fence is itself a failure.
#
# Zero work is not a pass. The gate fails when no snippet is executed, which
# covers both an empty tree and a tree whose every snippet is exempted; the
# two cases report different counts.
#
# Runner discovery is the same rule as the other example runners:
# ${CARGO_TARGET_DIR:-<repo>/target}/{debug,release}/lovelace-run, with the
# .exe suffix on Windows (Git Bash / MSYS / Cygwin). A missing runner is exit 2
# and never a pass: this gate needs the language built from the current tree.
# CI builds `cargo build -p lovelace-run` before running it.
#
# Exit status:
#   0  every discovered snippet was executed or exempted, and every executed
#      snippet's output matched
#   1  a mismatch, an unpaired fence, an annotation problem or an unexpected
#      runner exit
#   2  bad usage, no runner, or no executed snippet (zero work)
#
# The last stdout line is the machine-readable evidence line docs-report.sh
# collects. Every quantity is counted independently:
#   DOCREPORT gate=check-doc-examples status=<ok|fail> pages=<n> discovered=<n>
#             executed=<n> passed=<n> failed=<n> exempted=<n>

set -u

usage() {
    printf 'usage: bash tools/check-doc-examples.sh\n'
    printf '  executes every ```lovelace snippet under docs/ and compares its output\n'
}

for arg in "$@"; do
    case "$arg" in
        -h | --help)
            usage
            exit 0
            ;;
        *)
            printf 'check-doc-examples: unknown argument: %s\n' "$arg" >&2
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

# ---------------------------------------------------------------------------
# Locate lovelace-run. A gate that cannot find the language is exit 2, never a
# skipped pass: the snippets must be executed by the current tree's runner.
# ---------------------------------------------------------------------------
case "$(uname -s 2>/dev/null || true)" in
    MINGW* | MSYS* | CYGWIN*) is_windows=1 ;;
    *) is_windows=0 ;;
esac
if ((is_windows)); then exe_suffix=.exe; else exe_suffix=; fi

if [[ -n ${CARGO_TARGET_DIR:-} ]]; then
    target_dir=$CARGO_TARGET_DIR
    case $target_dir in
        /* | [A-Za-z]:[\\/]*) : ;;
        *) target_dir=$repo_root/$target_dir ;;
    esac
else
    target_dir=$repo_root/target
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
    printf 'check-doc-examples: lovelace-run not found under %s/{debug,release}\n' "$target_dir" >&2
    printf 'check-doc-examples: build it first with: cargo build -p lovelace-run\n' >&2
    exit 2
fi
if ((!is_windows)) && [[ ! -x $exe ]]; then
    printf 'check-doc-examples: lovelace-run is not executable: %s\n' "$exe" >&2
    exit 2
fi

scratch=$(mktemp -d "${TMPDIR:-/tmp}/doc-examples.XXXXXX") || {
    printf 'check-doc-examples: could not create a scratch directory\n' >&2
    exit 2
}
cleanup() { rm -rf -- "$scratch"; }
trap cleanup EXIT HUP INT TERM

mkdir -p "$scratch/tmp" "$scratch/appdata" "$scratch/plots"
export TMPDIR=$scratch/tmp TEMP=$scratch/tmp TMP=$scratch/tmp
export XDG_CONFIG_HOME=$scratch/appdata APPDATA=$scratch/appdata

# Git Bash translates argv paths for a native child; cygpath makes it explicit
# where it exists, and the POSIX path is already native elsewhere.
native_path() {
    if command -v cygpath >/dev/null 2>&1; then
        cygpath -w -- "$1"
    else
        printf '%s' "$1"
    fi
}

normalise_text() {
    tr -s '[:space:]' ' ' | sed -e 's/^ //' -e 's/ $//'
}

# ---------------------------------------------------------------------------
# Page discovery. The exclusions are stated, printed and applied here only.
# ---------------------------------------------------------------------------
exclusions=(
    'docs/internal/archive/ historical harness records'
    'docs/csharp-oracle/ carried fixtures'
    'docs/goal- harness working memory'
    'docs/tutorials/ covered by tools/run-tutorials.sh'
)
printf 'check-doc-examples: scanning docs/ for ```lovelace fences\n'
for entry in "${exclusions[@]}"; do
    printf 'check-doc-examples: excluded %s\n' "$entry"
done

pages=()
while IFS= read -r page; do
    pages+=("$page")
done < <(
    find docs -type f -name '*.md' \
        -not -path 'docs/internal/archive/*' \
        -not -path 'docs/csharp-oracle/*' \
        -not -path 'docs/goal-*/*' \
        -not -path 'docs/tutorials/*' |
        sort
)
if [[ ${#pages[@]} -eq 0 ]]; then
    printf 'check-doc-examples: no Markdown page under docs/; refusing to pass on zero work\n' >&2
    printf 'DOCREPORT gate=check-doc-examples status=fail pages=0 discovered=0 executed=0 passed=0 failed=1 exempted=0\n'
    exit 2
fi

pages_scanned=0
pages_with_examples=0
discovered=0
executed=0
passed=0
failed=0
exempted=0
problems=0

for page in "${pages[@]}"; do
    pages_scanned=$((pages_scanned + 1))

    page_lines=()
    while IFS= read -r line || [[ -n $line ]]; do
        page_lines+=("${line%$'\r'}")
    done <"$page"
    line_count=${#page_lines[@]}

    # Split the page into fenced blocks and attach `not-run` annotations. The
    # pending annotation is promoted to active_reason when its fence opens, so
    # the reason survives until the block is recorded at the closing fence.
    block_lang=()
    block_text=()
    block_open=()
    block_close=()
    block_reason=()
    in_fence=0
    lang=
    buffer=
    sep=
    open_line=0
    active_reason=
    pending_reason=
    pending_line=0

    for ((n = 0; n < line_count; n++)); do
        line=${page_lines[n]}
        if ((in_fence)); then
            if [[ $line =~ ^[[:space:]]*'```'[[:space:]]*$ ]]; then
                in_fence=0
                block_lang+=("$lang")
                block_text+=("$buffer")
                block_open+=("$open_line")
                block_close+=($((n + 1)))
                block_reason+=("${active_reason}")
                active_reason=
            else
                buffer+=$sep$line
                sep=$'\n'
            fi
            continue
        fi

        if [[ $line =~ ^[[:space:]]*'```'[[:space:]]*([^[:space:]]*)[[:space:]]*$ ]]; then
            in_fence=1
            lang=$(printf '%s' "${BASH_REMATCH[1]}" | tr '[:upper:]' '[:lower:]')
            buffer=
            sep=
            open_line=$((n + 1))
            if [[ -n $pending_reason ]]; then
                if [[ $lang == lovelace ]] && ((open_line == pending_line + 1)); then
                    active_reason=$pending_reason
                else
                    printf 'FAIL %s:%d: a not-run annotation must sit immediately above a ```lovelace fence\n' \
                        "$page" "$pending_line"
                    problems=$((problems + 1))
                fi
                pending_reason=
                pending_line=0
            fi
            continue
        fi

        if [[ $line =~ ^[[:space:]]*'<!--'[[:space:]]*not-run: ]]; then
            if [[ -n $pending_reason ]]; then
                printf 'FAIL %s:%d: a not-run annotation for %s was not attached to a fence\n' \
                    "$page" "$pending_line" "$page"
                problems=$((problems + 1))
            fi
            reason=$line
            reason=${reason#*not-run:}
            reason=${reason%%-->*}
            reason=$(printf '%s' "$reason" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')
            if [[ -z $reason ]]; then
                printf 'FAIL %s:%d: not-run annotation has an empty reason\n' "$page" "$((n + 1))"
                problems=$((problems + 1))
                pending_reason=
                pending_line=0
                continue
            fi
            pending_reason=$reason
            pending_line=$((n + 1))
            continue
        fi
    done

    if ((in_fence)); then
        printf 'FAIL %s:%d: fence is never closed\n' "$page" "$open_line" >&2
        problems=$((problems + 1))
    fi
    if [[ -n $pending_reason ]]; then
        printf 'FAIL %s:%d: a not-run annotation has no fence (reason: %s)\n' \
            "$page" "$pending_line" "$pending_reason" >&2
        problems=$((problems + 1))
    fi

    # This page's documented-output mode: docs/language documents the variable
    # list, every other page documents the result line only.
    family_args=(--text)
    case $page in
        docs/language/*) : ;;
        *) family_args+=(--omit-variables) ;;
    esac

    page_examples=0
    page_passed=0
    block_count=${#block_lang[@]}
    for ((b = 0; b < block_count; b++)); do
        [[ ${block_lang[b]} == lovelace ]] || continue
        discovered=$((discovered + 1))
        page_examples=$((page_examples + 1))
        fence_line=${block_open[b]}

        if [[ -n ${block_reason[b]} ]]; then
            exempted=$((exempted + 1))
            printf 'EXCLUDED %s:%d (reason: %s)\n' "$page" "$fence_line" "${block_reason[b]}"
            continue
        fi

        expected=
        if ((b + 1 < block_count)) && [[ ${block_lang[b + 1]} == text ]]; then
            adjacent=1
            for ((n = block_close[b]; n < block_open[b + 1] - 1; n++)); do
                if [[ ${page_lines[n]} =~ [^[:space:]] ]]; then
                    adjacent=0
                    break
                fi
            done
            if ((adjacent)); then
                expected=${block_text[b + 1]}
            fi
        fi
        if [[ -z $expected ]]; then
            printf 'FAIL %s:%d: a ```lovelace fence must be followed immediately by its ```text output\n' \
                "$page" "$fence_line" >&2
            failed=$((failed + 1))
            continue
        fi

        executed=$((executed + 1))
        script=$scratch/example-$executed.lv
        printf '%s' "${block_text[b]}" >"$script"
        stdout_file=$scratch/example-$executed.out
        stderr_file=$scratch/example-$executed.err
        : >"$stdout_file"
        : >"$stderr_file"

        if "$exe" --file "$(native_path "$script")" \
            --plot-dir "$(native_path "$scratch/plots")" \
            --plot-file "example-$executed.svg" \
            "${family_args[@]}" >"$stdout_file" 2>"$stderr_file"; then
            rc=0
        else
            rc=$?
        fi

        # Only 0 (success) and 1 (documented refusal) are runner states this
        # contract knows. Anything else is a failure whatever the output says,
        # so a broken or replaced runner cannot pass by printing the right text.
        if ((rc != 0 && rc != 1)); then
            printf 'FAIL %s:%d: the runner exited %d, which is neither success (0) nor a documented refusal (1)\n' \
                "$page" "$fence_line" "$rc" >&2
            failed=$((failed + 1))
            continue
        fi

        # A matching message is not sufficient if a runner changed the exit
        # contract. Recognizable success and refusal transcripts imply their
        # corresponding process status. Unrecognized text remains subject to
        # the original output comparison.
        case "$expected" in
            '= '*) expected_rc=0 ;;
            'Error ['*) expected_rc=1 ;;
            *) expected_rc=-1 ;;
        esac
        if ((expected_rc >= 0 && rc != expected_rc)); then
            printf 'FAIL %s:%d: output resembles exit %d but runner exited %d\n' \
                "$page" "$fence_line" "$expected_rc" "$rc" >&2
            failed=$((failed + 1))
            continue
        fi

        if ((rc == 0)); then
            raw=$(cat "$stdout_file")
        else
            raw=$(cat "$stderr_file")
        fi

        want=$(printf '%s' "$expected" | normalise_text)
        got=$(printf '%s' "$raw" | normalise_text)
        if [[ $got == "$want" ]]; then
            passed=$((passed + 1))
            page_passed=$((page_passed + 1))
        else
            printf 'FAIL %s:%d: documented output does not match the current language (exit %d)\n' \
                "$page" "$fence_line" "$rc" >&2
            printf '  expected: %s\n' "$want" >&2
            printf '  observed: %s\n' "$got" >&2
            failed=$((failed + 1))
        fi
    done

    if ((page_examples > 0)); then
        pages_with_examples=$((pages_with_examples + 1))
        printf '%s: %d example(s), %d matched\n' "$page" "$page_examples" "$page_passed"
    fi
done

failures=$((failed + problems))
status=ok
if ((failures > 0)); then
    status=fail
    printf 'check-doc-examples: %d failure(s)\n' "$failures" >&2
fi

if ((executed == 0)); then
    printf 'check-doc-examples: docs/ yielded no executed example (%d discovered, %d exempted); refusing to pass on zero work\n' \
        "$discovered" "$exempted" >&2
    printf 'DOCREPORT gate=check-doc-examples status=fail pages=%d discovered=%d executed=0 passed=0 failed=%d exempted=%d\n' \
        "$pages_with_examples" "$discovered" $((failures + 1)) "$exempted"
    exit 2
fi

printf 'check-doc-examples: %d page(s) scanned, %d with examples, %d discovered, %d executed, %d matched, %d exempted, %d failure(s)\n' \
    "$pages_scanned" "$pages_with_examples" "$discovered" "$executed" "$passed" "$exempted" "$failures"
printf 'DOCREPORT gate=check-doc-examples status=%s pages=%d discovered=%d executed=%d passed=%d failed=%d exempted=%d\n' \
    "$status" "$pages_with_examples" "$discovered" "$executed" "$passed" "$failures" "$exempted"

if [[ $status == fail ]]; then
    exit 1
fi
exit 0
