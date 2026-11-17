#!/usr/bin/env bash
# docs-report.sh - run every documentation gate and print machine-readable evidence.
#
# Usage: bash tools/docs-report.sh
#
# Why this exists (RQ-023): a gate that prints only "OK" cannot be audited from
# a CI transcript. Every gate now ends with one line of the form
#
#     DOCREPORT gate=<name> status=<ok|fail> <key>=<n> ...
#
# on stdout, saying what it checked as well as whether it passed. This script
# runs the gates in order, forwards their human output, and prints each
# DOCREPORT line once (the gate's own copy is the source; it is not repeated).
# The last line is
#
#     DOCREPORT total gates=<n> ok=<n> failed=<n> commit=<sha> os=<uname>
#
# and the exit status is 0 only when failed=0.
#
# Gates, in this order:
#   1. check-crate-examples.sh   compile and run the docs/crates/ Rust fences
#   2. run-tutorials.sh          run the four tutorials as contracts
#   3. check-doc-examples.sh     execute every documented ```lovelace snippet
#   4. check-docs.sh --symbols --claims --readme
#                                links, anchors, references, citations, symbols,
#                                claims, tutorial precision sections, README
#   5. check-reference-pages.sh  the seven-section reference-page contract
#
# check-docs.sh runs once with all three audit flags, the way the script is
# designed to compose, so one line carries every mode's counts with
# `modes=symbols,claims,readme`. A defect in one mode therefore fails exactly
# one gate, which keeps the total unambiguous; the docs CI job also runs the
# three modes as separate steps so a red job still names the mode.
#
# Failure rules, deliberately strict:
#   * a gate that produced no DOCREPORT line is a FAILURE, even when it exited
#     0: a silent pass is not evidence;
#   * a gate that produced more than one DOCREPORT line is a FAILURE (the
#     contract is one final line, so a second line means the summary is not
#     trustworthy);
#   * a gate whose DOCREPORT names a different gate is a FAILURE;
#   * a gate that reported status=ok but exited non-zero is a FAILURE: the
#     exit status and the summary must agree.
#
# A missing summary is reported as a synthesized line so the transcript still
# says which gate is unaccounted for:
#
#     DOCREPORT gate=<name> status=fail summary=missing exit=<status>
#
# Requires bash, awk, grep, sed, git (optional; <sha> is `unknown` outside a
# git checkout) and uname - the tools present in ubuntu-latest and in Git Bash
# on windows-latest.
#
# Exit status: 0 every gate passed and reported; 1 any gate failed, was
# missing a summary or contradicted its exit status; 2 bad usage or setup.

set -u

usage() {
    printf 'usage: bash tools/docs-report.sh\n'
    printf '  runs every documentation gate in order and prints one DOCREPORT\n'
    printf '  line per gate, then a DOCREPORT total with commit and OS\n'
}

for arg in "$@"; do
    case "$arg" in
        -h | --help)
            usage
            exit 0
            ;;
        *)
            printf 'docs-report: unknown argument: %s\n' "$arg" >&2
            usage >&2
            exit 2
            ;;
    esac
done

LC_ALL=C
export LC_ALL

script_dir=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd) || exit 2
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd) || exit 2
cd -- "$repo_root" || exit 2

work=$(mktemp -d "${TMPDIR:-/tmp}/docs-report.XXXXXX") || {
    printf 'docs-report: could not create a scratch directory under %s\n' "${TMPDIR:-/tmp}" >&2
    exit 2
}
trap 'rm -rf -- "$work"' EXIT HUP INT TERM

gates_run=0
gates_ok=0
gates_failed=0

# run_gate EXPECTED_GATE_NAME COMMAND... - run one gate, print its human output
# (DOCREPORT lines removed), then print that single line once.
run_gate() {
    local expected=$1
    shift
    local log rc=0 docreport_count summary actual status gate_failed=0

    gates_run=$((gates_run + 1))
    log=$work/gate-$gates_run.log

    printf '===== gate %d: %s =====\n' "$gates_run" "$*"
    if "$@" >"$log" 2>&1; then
        rc=0
    else
        rc=$?
    fi

    # Forward everything the gate said except its machine-readable line: that
    # line is printed exactly once, below, after the human transcript.
    sed '/^DOCREPORT /d' -- "$log"

    docreport_count=$(grep -c '^DOCREPORT ' "$log" || true)
    summary=$(grep '^DOCREPORT ' "$log" | tail -n 1 || true)

    if [ "$docreport_count" -eq 0 ]; then
        printf 'docs-report: gate=%s exit=%d produced no DOCREPORT line; a missing summary is a failure\n' \
            "$expected" "$rc" >&2
        printf 'DOCREPORT gate=%s status=fail summary=missing exit=%d\n' "$expected" "$rc"
        gates_failed=$((gates_failed + 1))
        return 1
    fi

    printf 'docs-report: gate=%s exit=%d summaries=%d\n' "$expected" "$rc" "$docreport_count"
    printf '%s\n' "$summary"

    if [ "$docreport_count" -ne 1 ]; then
        printf 'docs-report: gate=%s produced %d DOCREPORT lines; the contract is exactly one final line\n' \
            "$expected" "$docreport_count" >&2
        gate_failed=1
    fi

    actual=${summary#DOCREPORT gate=}
    actual=${actual%% *}
    if [ "$actual" != "$expected" ]; then
        printf 'docs-report: gate=%s printed gate=%s in its summary\n' \
            "$expected" "$actual" >&2
        gate_failed=1
    fi

    status=${summary#*status=}
    status=${status%% *}
    if [ "$status" != ok ] && [ "$status" != fail ]; then
        printf 'docs-report: gate=%s printed no usable status= key\n' "$expected" >&2
        gate_failed=1
        status=fail
    fi
    if [ "$status" = fail ]; then
        gate_failed=1
    fi
    if [ "$rc" -ne 0 ] && [ "$status" = ok ]; then
        printf 'docs-report: gate=%s reported status=ok but exited %d; the exit status and the summary disagree\n' \
            "$expected" "$rc" >&2
        gate_failed=1
    fi

    if [ "$gate_failed" -eq 0 ]; then
        gates_ok=$((gates_ok + 1))
    else
        gates_failed=$((gates_failed + 1))
    fi
    return 0
}

# ---------------------------------------------------------------------------
# Every gate, in order. No step is optional and none is skipped: a failing
# gate is recorded and the run continues, so one transcript shows the state of
# the whole evidence set.
# ---------------------------------------------------------------------------
run_gate check-crate-examples bash tools/check-crate-examples.sh
run_gate run-tutorials bash tools/run-tutorials.sh
run_gate check-doc-examples bash tools/check-doc-examples.sh
run_gate check-docs bash tools/check-docs.sh --symbols --claims --readme
run_gate check-reference-pages bash tools/check-reference-pages.sh

commit=$(git -C "$repo_root" rev-parse --short HEAD 2>/dev/null || true)
if [ -z "$commit" ]; then
    commit=unknown
fi
os=$(uname -s 2>/dev/null || printf unknown)
if [ -z "$os" ]; then
    os=unknown
fi

if [ "$gates_failed" -gt 0 ]; then
    printf 'docs-report: FAIL (%d of %d gate(s) failed)\n' "$gates_failed" "$gates_run"
fi

printf 'DOCREPORT total gates=%d ok=%d failed=%d commit=%s os=%s\n' \
    "$gates_run" "$gates_ok" "$gates_failed" "$commit" "$os"

if [ "$gates_failed" -gt 0 ]; then
    exit 1
fi
exit 0
