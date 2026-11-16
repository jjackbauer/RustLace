#!/usr/bin/env bash
# test-docs-gates.sh - the mutation self-test for RustLace's documentation gates.
#
# "A gate that cannot be shown to fail is not a gate." For every documentation
# gate, this script builds a defect in a scratch tree, runs the REAL checker
# against it, and requires BOTH a non-zero exit AND a diagnostic that names the
# defective file and the problem. A mutation that still passes fails this
# script; so does a clean fixture that cannot pass in the first place, because
# then the mutation's failure would prove nothing.
#
# What is mutated (one PASS line per mutation):
#   check-docs.sh (links, heading anchors, code references)
#     1. a broken relative link
#     2. a path-shaped inline code reference to a nonexistent file
#     3. a heading anchor that matches no heading on the same page (README.md)
#     4. a heading anchor that matches no heading in the destination page
#     5. a nonexistent path linked from a root page (README.md)
#     6. a broken link in an agent instruction (.dsh/skills/**/SKILL.md)
#   check-docs.sh --symbols
#     7. a .rs:NNN citation outside docs/adr/
#     8. a dead snake_case symbol citation
#   check-docs.sh --claims
#     9. a prohibited absolutist numerical phrase
#    10. a tutorial without its "## Precision" section
#   check-docs.sh --readme
#    11. a README without its "## Current limitations" section
#   check-reference-pages.sh
#    12. a listed reference page missing a required section
#    13. a listed reference page with an out-of-order section
#    14. a scoped-tree page that is neither manifested nor declared non-reference
#    15. a manifested page that does not exist on disk
#    16. a required section heading with no content before the next heading
#    17. a declared non-reference page with no reason
#   docs/language/examples/run-examples.sh
#    18. an undeclared page in the runner manifest
#    19. a declared page missing on disk
#    20. zero example pairs in the whole run
#    21. an unpaired ```lovelace block
#   docs/extensions/examples/run-examples.sh
#    22. an undeclared page in the runner manifest
#    23. a declared page missing on disk
#    24. zero example pairs in the whole run
#    25. an unpaired ```lovelace block
#   the CI wiring guard
#    26. a mandatory docs-job step deleted from a copy of ci.yml
#   tools/docs-report.sh
#    27. a gate that exits 0 without printing its DOCREPORT line
#
# The script also asserts that .github/workflows/ci.yml still invokes every
# mandatory gate as a step of the docs job, so the self-test cannot stay green
# after a mode or a runner is quietly dropped from CI. The assertion is a
# SUBSET: each required command must be present, but the job may gain steps
# and no exact step count is required. It reads the docs job block only, so a
# command mentioned elsewhere in the file does not satisfy it.
#
# Nothing in the working tree is modified: every fixture lives under a mktemp
# directory removed on exit. Each scratch checker is a byte-for-byte copy of
# the real one (verified with cmp), so the test cannot pass by testing a stub.
#
# Usage (from anywhere; the repository root is derived from this file):
#   bash tools/test-docs-gates.sh
#   bash tools/test-docs-gates.sh --check-ci FILE   # the wiring guard alone
#                                                   # (used by mutation 26)
#
# Requires bash, cp, cmp, awk, grep, find, mktemp and, for the two example
# runners, a built lovelace-run under ${CARGO_TARGET_DIR:-<repo>/target}.
#
# Exit status:
#   0  every mutation failed with its intended diagnostic, and CI is wired
#   1  a mutation passed, or a diagnostic was wrong, or CI is not wired
#   2  setup failure (no scratch directory, checker copy differs, no lovelace-run)

set -u

LC_ALL=C
export LC_ALL

script_dir=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd) || exit 2
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd) || exit 2
cd -- "$repo_root" || exit 2

work=$(mktemp -d "${TMPDIR:-/tmp}/test-docs-gates.XXXXXX") || {
    printf 'test-docs-gates: could not create a scratch directory under %s\n' "${TMPDIR:-/tmp}" >&2
    exit 2
}
trap 'rm -rf -- "$work"' EXIT HUP INT TERM

mutations=0
passed=0
failed=0
mutation_failed=0

fail() {
    # $1 case or area, $2 problem
    printf 'FAIL %s: %s\n' "$1" "$2" >&2
    failed=$((failed + 1))
}

# A mutation that did not fail is a broken gate, not a broken harness: it is
# both a problem and a mutation verdict, so the final guard can tell "every
# mutation was judged" apart from "a script path forgot to judge one".
fail_mutation() {
    printf 'FAIL %s: %s\n' "$1" "$2" >&2
    failed=$((failed + 1))
    mutation_failed=$((mutation_failed + 1))
}

# copy_checker SRC DST - the scratch checker must be the real checker, byte for
# byte. A copy that differs means the self-test would be testing something else.
copy_checker() {
    local src=$1 dst=$2
    mkdir -p -- "$(dirname -- "$dst")" || exit 2
    cp -- "$src" "$dst" || exit 2
    if ! cmp -s -- "$src" "$dst"; then
        printf 'test-docs-gates: scratch copy of %s differs from the real checker\n' "$src" >&2
        exit 2
    fi
}

# expect_success LABEL LOGFILE CMD... - the clean fixture must pass before the
# mutation is built; otherwise "it failed after the mutation" proves nothing.
expect_success() {
    local label=$1 log=$2 rc=0
    shift 2
    if "$@" >"$log" 2>&1; then rc=0; else rc=$?; fi
    if [ "$rc" -ne 0 ]; then
        fail "baseline $label" "clean fixture exited $rc; the mutation cannot be attributed to the defect"
        sed 's/^/    /' "$log" >&2
        return 1
    fi
    printf 'CTRL %s: clean fixture passed\n' "$label"
    return 0
}

# expect_failure LABEL SUBJECT MESSAGE LOGFILE CMD... - the mutated fixture must
# exit non-zero AND print a diagnostic naming SUBJECT and stating MESSAGE.
expect_failure() {
    local label=$1 subject=$2 message=$3 log=$4 rc=0
    shift 4
    if "$@" >"$log" 2>&1; then rc=0; else rc=$?; fi
    mutations=$((mutations + 1))

    if [ "$rc" -eq 0 ]; then
        fail_mutation "$label" "the mutated fixture PASSED (exit 0); a mutation that does not fail is not a gate"
        sed 's/^/    /' "$log" >&2
        return 1
    fi
    if [ -n "$subject" ] && ! grep -Fq -- "$subject" "$log"; then
        fail_mutation "$label" "exit $rc, but the diagnostic does not name '$subject'"
        sed 's/^/    /' "$log" >&2
        return 1
    fi
    if ! grep -Fq -- "$message" "$log"; then
        fail_mutation "$label" "exit $rc, but the diagnostic lacks '$message'"
        sed 's/^/    /' "$log" >&2
        return 1
    fi

    passed=$((passed + 1))
    printf 'PASS %s (exit %d)\n' "$label" "$rc"
    return 0
}

# ---------------------------------------------------------------------------
# CI wiring: the self-test is only meaningful for the gates CI actually runs.
# The guard reads the docs job block and asserts that every mandatory command
# is still a step of it. The assertion is deliberately a SUBSET - adding a
# step is allowed, deleting one of these is not - so another agent can extend
# the job without breaking the self-test. Invoked on a path so the mutation
# case can run it against a copy of ci.yml with one step removed.
# ---------------------------------------------------------------------------
check_ci_wiring() {
    local ci=$1 block=$work/ci-docs-job.txt docs_line=1
    local literal missing=0

    if [ ! -f "$ci" ]; then
        fail "ci wiring" "missing $ci"
        return 1
    fi

    docs_line=$(awk '/^  docs:$/ { print NR; exit }' "$ci")
    if [ -z "$docs_line" ]; then
        docs_line=1
    fi

    awk '
        { sub(/\r$/, "") }
        /^  docs:$/ { inside = 1; print; next }
        inside && /^  [A-Za-z0-9_-]+:$/ { exit }
        inside { print }
    ' "$ci" >"$block"

    if [ ! -s "$block" ]; then
        fail "ci wiring" "$ci:$docs_line: ci.yml has no '  docs:' job block"
        return 1
    fi

    # Every mandatory step of the docs job, by command. check-crate-examples.sh
    # and run-tutorials.sh are the two steps the old guard missed; both example
    # runners, all three check-docs modes, the reference-page gate, the
    # machine-readable evidence report and this self-test are the rest.
    for literal in \
        'bash tools/check-crate-examples.sh' \
        'bash tools/run-tutorials.sh' \
        'bash docs/language/examples/run-examples.sh' \
        'bash docs/extensions/examples/run-examples.sh' \
        'bash tools/check-docs.sh --symbols' \
        'bash tools/check-docs.sh --claims' \
        'bash tools/check-docs.sh --readme' \
        'bash tools/check-reference-pages.sh' \
        'bash tools/docs-report.sh' \
        'bash tools/test-docs-gates.sh'
    do
        if ! grep -Fq -- "$literal" "$block"; then
            fail "ci wiring" "$ci:$docs_line: the docs job does not run: $literal"
            missing=1
        fi
    done

    if ! grep -Fq -- 'os: [ubuntu-latest, windows-latest]' "$ci"; then
        fail "ci wiring" "$ci:$docs_line: the docs job does not run on ubuntu-latest and windows-latest"
        missing=1
    fi
    if grep -Fq -- '|| true' "$block"; then
        fail "ci wiring" "$ci:$docs_line: the docs job contains '|| true', which lets a gate fail silently"
        missing=1
    fi
    if grep -Fq -- 'continue-on-error' "$block"; then
        fail "ci wiring" "$ci:$docs_line: the docs job contains continue-on-error"
        missing=1
    fi
    if grep -Eq '^[[:space:]]+if:' "$block"; then
        fail "ci wiring" "$ci:$docs_line: the docs job contains a conditional step (if:)"
        missing=1
    fi

    if [ "$missing" -eq 0 ]; then
        printf 'WIRE ci.yml: the docs job runs the crate-example and tutorial gates, both example runners, the three check-docs modes, the reference-page gate, the evidence report and this mutation self-test on the Ubuntu+Windows matrix; no conditional skip\n'
    fi
    return "$missing"
}

# --check-ci FILE is the guard's own entry point: it lets the mutation case run
# the real guard against a defective workflow copy without touching ci.yml.
if [ "${1:-}" = "--check-ci" ]; then
    if [ "$#" -ne 2 ]; then
        printf 'test-docs-gates: --check-ci takes exactly one file\n' >&2
        exit 2
    fi
    check_ci_wiring "$2"
    if [ "$failed" -gt 0 ]; then
        exit 1
    fi
    exit 0
fi
if [ "$#" -gt 0 ]; then
    printf 'test-docs-gates: unknown argument: %s\n' "$1" >&2
    printf 'usage: bash tools/test-docs-gates.sh [--check-ci FILE]\n' >&2
    exit 2
fi

# ---------------------------------------------------------------------------
# Scratch fixtures
# ---------------------------------------------------------------------------

# A minimal current documentation tree that passes every check-docs.sh mode:
# one page per scanned class (docs/, root page, agent instruction) with a
# resolving link, a resolving heading anchor and a live code reference, and a
# placeholder source root so the symbol index is non-empty.
make_check_docs_tree() {
    local root=$1
    mkdir -p -- "$root/tools" "$root/docs" "$root/lovelace-lang/src" \
        "$root/.github" "$root/.dsh/skills/scratch" || exit 2
    copy_checker "$repo_root/tools/check-docs.sh" "$root/tools/check-docs.sh"
    cat >"$root/lovelace-lang/src/lib.rs" <<'RUST'
pub fn engine_open() {}
pub fn engine_close() {}
RUST
    cat >"$root/docs/index.md" <<'MD'
# Scratch documentation index

Self-test fixture. The engine entry point is `engine_open()`.

See [the page template](template.md#template) and the implementation in
`lovelace-lang/src/lib.rs`.
MD
    cat >"$root/docs/template.md" <<'MD'
# Template

Scratch page.
MD
    cat >"$root/README.md" <<'MD'
# Scratch README

A root page for the documentation-gate self-test.

## Notes

See [the notes](#notes) and [the page template](docs/template.md#template).
MD
    cat >"$root/.github/copilot-instructions.md" <<'MD'
# Scratch agent instructions

See [the page template](../docs/template.md#template) and `engine_open()`.
MD
    cat >"$root/.dsh/skills/scratch/SKILL.md" <<'MD'
# Scratch skill

See [the page template](../../../docs/template.md#template) and `engine_open()`.
MD
}

# A README that satisfies all four --readme requirements, in order.
write_readme() {
    local path=$1
    cat >"$path" <<'MD'
# Scratch README

This scratch tree computes arbitrary-precision arithmetic and arrays for the documentation-gate self-test.

## Current limitations

None; this is a fixture.

```console
$ lovelace-run --file example.ls --text
```

## The crates

Scratch.
MD
}

# A manifest, one conformant reference page, the three scoped trees the gate
# requires, and one declared non-reference page with its reason.
make_reference_tree() {
    local root=$1
    mkdir -p -- "$root/tools" "$root/docs/reference" "$root/docs/language" \
        "$root/docs/extensions" "$root/docs/crates" || exit 2
    copy_checker "$repo_root/tools/check-reference-pages.sh" "$root/tools/check-reference-pages.sh"
    cat >"$root/docs/reference-pages.md" <<'MD'
# Scratch reference pages

## Manifest

| Page | Surface | Why it is in scope |
|---|---|---|
| `docs/reference/thing.md` | thing | Self-test fixture. |

## Not in scope

| Page | Why it is not in scope |
|---|---|
| `docs/language/guide.md` | Self-test fixture: a narrative guide, not a callable surface. |
MD
    cat >"$root/docs/language/guide.md" <<'MD'
# guide

Scratch narrative page; declared non-reference by the scratch manifest.
MD
    cat >"$root/docs/reference/thing.md" <<'MD'
# thing

## Purpose

Scratch.

## Inputs

None.

## Outputs

None.

## Precision behaviour

None.

## Errors

None.

## Limitations

None known.

## Examples

None.
MD
}

language_pages='README.md lexical-structure.md values-and-types.md expressions-and-operators.md statements-and-control-flow.md functions-and-scope.md builtins.md errors-and-diagnostics.md precision-and-formatting.md plots.md'

# write_language_catalogue PAGE - generate a builtins.md catalogue from the
# runner under test. The language runner now cross-checks
# docs/language/builtins.md against `lovelace-run --functions` (RQ-014), so a
# stub page would make every fixture fail for a reason unrelated to the
# mutation. Generating the page from the same binary keeps the fixture honest,
# and the generated page carries no lovelace examples, so the zero-example
# mutation still observes an empty set.
write_language_catalogue() {
    local page=$1 json record name category minimum declared variadic parameters arity key count total
    local records=$work/language-catalogue-records.tsv
    local categories=$work/language-catalogue-categories.txt

    "$runner_exe" --functions --function-limit 500 >"$work/functions.json" || {
        printf 'test-docs-gates: lovelace-run --functions failed; cannot build the catalogue fixture\n' >&2
        exit 2
    }
    json=$(cat "$work/functions.json")
    case $json in
        *'"ok":true'*) ;;
        *)
            printf 'test-docs-gates: --functions JSON has no "ok":true\n' >&2
            exit 2
            ;;
    esac
    total=${json#*'"total":'}
    total=${total%%[!0-9]*}
    if [ -z "$total" ]; then
        printf 'test-docs-gates: --functions JSON has no "total"\n' >&2
        exit 2
    fi

    record_text=${json#*'"functions":['}
    case $record_text in
        *'],"mathIrVersion":'*) record_text=${record_text%%'],"mathIrVersion":'*} ;;
        *)
            printf 'test-docs-gates: --functions JSON has no terminated "functions" array\n' >&2
            exit 2
            ;;
    esac
    record_text=${record_text//'},{'/$'\n'}

    : >"$records"
    while IFS= read -r record; do
        [ -n "$record" ] || continue
        name=${record#*'"name":"'}
        name=${name%%'"'*}
        category=${record#*'"category":"'}
        category=${category%%'"'*}
        minimum=${record#*'"minArity":'}
        minimum=${minimum%%[!0-9]*}
        declared=${record#*'"parameterCount":'}
        declared=${declared%%[!0-9]*}
        variadic=false
        case $record in
            *'"variadic":true'*) variadic=true ;;
        esac
        parameters=${record#*'"parameters":['}
        parameters=${parameters%%]*}
        if [ -z "$parameters" ]; then
            parameters='&mdash;'
        else
            parameters=${parameters//\"/}
            parameters=${parameters//,/, }
        fi
        if [ "$variadic" = true ]; then
            arity="$minimum+"
        elif [ "$minimum" = "$declared" ]; then
            arity=$minimum
        else
            arity="$minimum-$declared"
        fi
        printf '%s\t| `%s` | %s | %s |\n' \
            "$category" "$name" "$arity" "$parameters" >>"$records"
    done <<<"$record_text"

    if [ ! -s "$records" ]; then
        printf 'test-docs-gates: --functions published no builtins; cannot build the catalogue fixture\n' >&2
        exit 2
    fi
    cut -f1 "$records" | LC_ALL=C sort -u >"$categories"

    {
        printf '# Builtins\n\n'
        printf 'Scratch fixture generated from `lovelace-run --functions` for the documentation-gate self-test.\n'
        printf 'It carries no lovelace example blocks on purpose.\n\n'
        printf '### Registry at a glance\n\n| Category | Builtins |\n|---|---|\n'
        while IFS= read -r key; do
            [ -n "$key" ] || continue
            count=$(awk -F'\t' -v wanted="$key" '$1 == wanted { n++ } END { print n + 0 }' "$records")
            printf '| %s | %d |\n' "$key" "$count"
        done <"$categories"
        printf '| **Total** | **%s** |\n\n' "$total"
        printf '### Full catalogue\n'
        while IFS= read -r key; do
            [ -n "$key" ] || continue
            count=$(awk -F'\t' -v wanted="$key" '$1 == wanted { n++ } END { print n + 0 }' "$records")
            printf '\n#### %s (%d builtins)\n\n| Name | Arity | Parameters |\n|---|---|---|\n' \
                "$key" "$count"
            awk -F'\t' -v wanted="$key" '$1 == wanted { print $2 }' "$records"
        done <"$categories"
    } >"$page"
}

# The language runner's declared page set, every page present, one generated
# builtin catalogue and one valid lovelace/text pair so the clean run executes
# an example and passes.
make_language_tree() {
    local root=$1 page
    mkdir -p -- "$root/docs/language/examples" || exit 2
    copy_checker "$repo_root/docs/language/examples/run-examples.sh" \
        "$root/docs/language/examples/run-examples.sh"
    for page in $language_pages; do
        [ "$page" = builtins.md ] && continue
        printf '# %s\n\nScratch page for the documentation-gate self-test.\n' "$page" \
            >"$root/docs/language/$page"
    done
    write_language_catalogue "$root/docs/language/builtins.md"
    cat >>"$root/docs/language/README.md" <<'MD'

```lovelace
1 + 1
```
```text
= 2 (Natural)
  _ = 2
```
MD
}

# The extensions runner's declared page set, with one valid pair on dsp.md.
make_extensions_tree() {
    local root=$1 page
    mkdir -p -- "$root/docs/extensions/examples" || exit 2
    copy_checker "$repo_root/docs/extensions/examples/run-examples.sh" \
        "$root/docs/extensions/examples/run-examples.sh"
    for page in README.md dsp.md mathir.md symbolics.md; do
        printf '# %s\n\nScratch page for the documentation-gate self-test.\n' "$page" \
            >"$root/docs/extensions/$page"
    done
    cat >>"$root/docs/extensions/dsp.md" <<'MD'

```lovelace
1 + 1
```
```text
= 2 (Natural)
```
MD
}

# write_report_stub ROOT NAME LINE - one fake gate for the docs-report fixture:
# it prints LINE on stdout and exits 0. The aggregator's own accounting is what
# this fixture tests; the real gates have their own mutations above.
write_report_stub() {
    local root=$1 name=$2 line=$3
    cat >"$root/tools/$name.sh" <<STUB
#!/usr/bin/env bash
printf '%s\n' '$line'
exit 0
STUB
    chmod +x "$root/tools/$name.sh" || exit 2
}

# A scratch tools/ tree for docs-report.sh: the REAL aggregator (copy_checker
# proves the copy is byte-identical) over stub gates that each print one valid
# DOCREPORT line, so the fixture has a clean baseline and a controllable defect.
make_docs_report_tree() {
    local root=$1
    mkdir -p -- "$root/tools" || exit 2
    copy_checker "$repo_root/tools/docs-report.sh" "$root/tools/docs-report.sh"
    write_report_stub "$root" check-crate-examples \
        'DOCREPORT gate=check-crate-examples status=ok pages=1 fences=1 compiled=1 excluded=0 failures=0'
    write_report_stub "$root" run-tutorials \
        'DOCREPORT gate=run-tutorials status=ok tutorials=4 fences=8 oracles=1 failures=0'
    write_report_stub "$root" check-docs \
        'DOCREPORT gate=check-docs status=ok pages=1 links=1 anchors=1 refs=1 citations=0 symbols=0 claims=0 tutorials=0 sections=0 readme=0 modes=stub failures=0'
    write_report_stub "$root" check-reference-pages \
        'DOCREPORT gate=check-reference-pages status=ok pages=1 manifested=1 non_reference=0 unclassified=0 sections=7 failed_pages=0 failures=0'
}

# ---------------------------------------------------------------------------
# Locate the built runner: the example-runner mutations must fail their own
# manifest/pairing checks, not "lovelace-run not found". Build it first.
# ---------------------------------------------------------------------------
runner_target_dir=${CARGO_TARGET_DIR:-$repo_root/target}
case $runner_target_dir in
    /* | [A-Za-z]:[\\/]*) : ;;
    *) runner_target_dir=$repo_root/$runner_target_dir ;;
esac

exe_suffix=
case "$(uname -s 2>/dev/null || true)" in
    MINGW* | MSYS* | CYGWIN*) exe_suffix=.exe ;;
esac

runner_exe=
for profile in debug release; do
    candidate=$runner_target_dir/$profile/lovelace-run$exe_suffix
    if [ -f "$candidate" ]; then
        runner_exe=$candidate
        break
    fi
done
if [ -z "$runner_exe" ]; then
    printf 'test-docs-gates: lovelace-run not found under %s/{debug,release%s}\n' \
        "$runner_target_dir" "$exe_suffix" >&2
    printf '  Build it first with: cargo build -p lovelace-run\n' >&2
    exit 2
fi
if [ -z "$exe_suffix" ] && [ ! -x "$runner_exe" ]; then
    printf 'test-docs-gates: lovelace-run is not executable: %s\n' "$runner_exe" >&2
    exit 2
fi
printf 'SETUP lovelace-run: %s\n' "$runner_exe"
printf 'SETUP scratch: %s\n' "$work"

# ---------------------------------------------------------------------------
# 1-2. check-docs.sh, base pass: links and path-shaped code references
# ---------------------------------------------------------------------------
{
    root=$work/check-docs-broken-link/repo
    make_check_docs_tree "$root"
    expect_success "check-docs base: broken-link fixture" "$work/check-docs-broken-link/baseline.log" \
        bash "$root/tools/check-docs.sh"
    printf '\nA broken [link](missing-page.md) is a documentation defect.\n' >>"$root/docs/index.md"
    expect_failure "check-docs: broken relative link" "docs/index.md" \
        "broken link 'missing-page.md'" "$work/check-docs-broken-link/mutated.log" \
        bash "$root/tools/check-docs.sh"
}

{
    root=$work/check-docs-missing-ref/repo
    make_check_docs_tree "$root"
    expect_success "check-docs base: missing-code-reference fixture" \
        "$work/check-docs-missing-ref/baseline.log" bash "$root/tools/check-docs.sh"
    printf '\nThe old kernel lived in `lovelace-lang/src/absent.rs`.\n' >>"$root/docs/index.md"
    expect_failure "check-docs: path-shaped code reference to a nonexistent file" \
        "docs/index.md" "does not name an existing file" \
        "$work/check-docs-missing-ref/mutated.log" bash "$root/tools/check-docs.sh"
}

# ---------------------------------------------------------------------------
# 3-4. check-docs.sh --symbols
# ---------------------------------------------------------------------------
{
    root=$work/check-docs-rs-line/repo
    make_check_docs_tree "$root"
    expect_success "check-docs --symbols: .rs:NNN fixture" "$work/check-docs-rs-line/baseline.log" \
        bash "$root/tools/check-docs.sh" --symbols
    printf '\nThe entry point used to be `lovelace-lang/src/lib.rs:12`.\n' >>"$root/docs/index.md"
    expect_failure "check-docs --symbols: .rs:NNN citation outside docs/adr/" \
        "docs/index.md" "is not allowed outside docs/adr/" \
        "$work/check-docs-rs-line/mutated.log" bash "$root/tools/check-docs.sh" --symbols
}

{
    root=$work/check-docs-dead-symbol/repo
    make_check_docs_tree "$root"
    expect_success "check-docs --symbols: live-symbol fixture" \
        "$work/check-docs-dead-symbol/baseline.log" bash "$root/tools/check-docs.sh" --symbols
    printf '\nThe kernel calls `absent_function()` before returning.\n' >>"$root/docs/index.md"
    expect_failure "check-docs --symbols: dead symbol citation" "docs/index.md" \
        "is not defined in lovelace-*/src" "$work/check-docs-dead-symbol/mutated.log" \
        bash "$root/tools/check-docs.sh" --symbols
}

# ---------------------------------------------------------------------------
# 5-6. check-docs.sh --claims
# ---------------------------------------------------------------------------
{
    root=$work/check-docs-claim/repo
    make_check_docs_tree "$root"
    mkdir -p -- "$root/docs/tutorials" || exit 2
    expect_success "check-docs --claims: phrase fixture" "$work/check-docs-claim/baseline.log" \
        bash "$root/tools/check-docs.sh" --claims
    printf '\nThe engine reports exact 30-place results.\n' >>"$root/docs/index.md"
    expect_failure "check-docs --claims: prohibited absolutist phrase" "docs/index.md" \
        "prohibited numerical claim 'exact 30-place'" "$work/check-docs-claim/mutated.log" \
        bash "$root/tools/check-docs.sh" --claims
}

{
    root=$work/check-docs-tutorial/repo
    make_check_docs_tree "$root"
    mkdir -p -- "$root/docs/tutorials" || exit 2
    expect_success "check-docs --claims: tutorial fixture" \
        "$work/check-docs-tutorial/baseline.log" bash "$root/tools/check-docs.sh" --claims
    cat >"$root/docs/tutorials/first-steps.md" <<'MD'
# First steps

A tutorial without its precision section.
MD
    expect_failure "check-docs --claims: tutorial missing '## Precision'" \
        "docs/tutorials/first-steps.md" "missing '## Precision' section" \
        "$work/check-docs-tutorial/mutated.log" bash "$root/tools/check-docs.sh" --claims
}

# ---------------------------------------------------------------------------
# 7. check-docs.sh --readme
# ---------------------------------------------------------------------------
{
    root=$work/check-docs-readme/repo
    make_check_docs_tree "$root"
    write_readme "$root/README.md"
    expect_success "check-docs --readme: README fixture" "$work/check-docs-readme/baseline.log" \
        bash "$root/tools/check-docs.sh" --readme
    awk '$0 != "## Current limitations" { print }' "$root/README.md" >"$root/README.md.defect" &&
        mv -- "$root/README.md.defect" "$root/README.md"
    expect_failure "check-docs --readme: README missing '## Current limitations'" \
        "README.md" "missing '## Current limitations' section" \
        "$work/check-docs-readme/mutated.log" bash "$root/tools/check-docs.sh" --readme
}

# ---------------------------------------------------------------------------
# 3-6. check-docs.sh: heading fragments, root pages and agent instructions.
# The clean fixture already carries resolving anchors in all three scanned
# classes, so the baselines prove the anchor pass actually runs.
# ---------------------------------------------------------------------------
{
    root=$work/check-docs-readme-anchor/repo
    make_check_docs_tree "$root"
    expect_success "check-docs: README anchor fixture" \
        "$work/check-docs-readme-anchor/baseline.log" bash "$root/tools/check-docs.sh"
    printf '\nA dead cross-reference: [missing section](#a-section-that-does-not-exist).\n' \
        >>"$root/README.md"
    expect_failure "check-docs: broken heading anchor in README.md" \
        "README.md" "broken heading anchor '#a-section-that-does-not-exist'" \
        "$work/check-docs-readme-anchor/mutated.log" bash "$root/tools/check-docs.sh"
}

{
    root=$work/check-docs-cross-anchor/repo
    make_check_docs_tree "$root"
    expect_success "check-docs: cross-file anchor fixture" \
        "$work/check-docs-cross-anchor/baseline.log" bash "$root/tools/check-docs.sh"
    printf '\nA dead cross-file reference: [absent heading](template.md#absent-heading).\n' \
        >>"$root/docs/index.md"
    expect_failure "check-docs: heading anchor missing in the destination page" \
        "docs/index.md" "broken heading anchor '#absent-heading'" \
        "$work/check-docs-cross-anchor/mutated.log" bash "$root/tools/check-docs.sh"
}

{
    root=$work/check-docs-root-path/repo
    make_check_docs_tree "$root"
    expect_success "check-docs: root-page path fixture" \
        "$work/check-docs-root-path/baseline.log" bash "$root/tools/check-docs.sh"
    printf '\nSee [the missing guide](docs/absent-guide.md).\n' >>"$root/README.md"
    expect_failure "check-docs: nonexistent path linked from a root page" \
        "README.md" "broken link 'docs/absent-guide.md'" \
        "$work/check-docs-root-path/mutated.log" bash "$root/tools/check-docs.sh"
}

{
    root=$work/check-docs-agent-link/repo
    make_check_docs_tree "$root"
    expect_success "check-docs: agent-instruction fixture" \
        "$work/check-docs-agent-link/baseline.log" bash "$root/tools/check-docs.sh"
    printf '\nSee [the missing page](../../../docs/absent.md).\n' \
        >>"$root/.dsh/skills/scratch/SKILL.md"
    expect_failure "check-docs: broken link in an agent instruction" \
        ".dsh/skills/scratch/SKILL.md" "broken link '../../../docs/absent.md'" \
        "$work/check-docs-agent-link/mutated.log" bash "$root/tools/check-docs.sh"
}

# ---------------------------------------------------------------------------
# 12-17. check-reference-pages.sh
# ---------------------------------------------------------------------------
{
    root=$work/reference-missing-section/repo
    make_reference_tree "$root"
    expect_success "check-reference-pages: conformant fixture" \
        "$work/reference-missing-section/baseline.log" bash "$root/tools/check-reference-pages.sh"
    awk '
        /^## Limitations$/ { skip = 1; next }
        skip && /^## / { skip = 0 }
        skip { next }
        { print }
    ' "$root/docs/reference/thing.md" >"$root/docs/reference/thing.defect" &&
        mv -- "$root/docs/reference/thing.defect" "$root/docs/reference/thing.md"
    expect_failure "check-reference-pages: missing required section" \
        "docs/reference/thing.md" "missing required section 'Limitations'" \
        "$work/reference-missing-section/mutated.log" bash "$root/tools/check-reference-pages.sh"
}

{
    root=$work/reference-out-of-order/repo
    make_reference_tree "$root"
    expect_success "check-reference-pages: ordered fixture" \
        "$work/reference-out-of-order/baseline.log" bash "$root/tools/check-reference-pages.sh"
    cat >"$root/docs/reference/thing.md" <<'MD'
# thing

## Purpose

Scratch.

## Outputs

None.

## Inputs

None.

## Precision behaviour

None.

## Errors

None.

## Limitations

None known.

## Examples

None.
MD
    expect_failure "check-reference-pages: out-of-order section" \
        "docs/reference/thing.md" "out of order; it must come after Outputs" \
        "$work/reference-out-of-order/mutated.log" bash "$root/tools/check-reference-pages.sh"
}

{
    root=$work/reference-unclassified/repo
    make_reference_tree "$root"
    expect_success "check-reference-pages: classified-tree fixture" \
        "$work/reference-unclassified/baseline.log" bash "$root/tools/check-reference-pages.sh"
    # A conformant reference page that nobody listed must still fail: the
    # manifest, not the page's shape, is what decides scope.
    cat >"$root/docs/extensions/new-surface.md" <<'MD'
# new-surface

## Purpose

Scratch.

## Inputs

None.

## Outputs

None.

## Precision behaviour

None.

## Errors

None.

## Limitations

None known.

## Examples

None.
MD
    expect_failure "check-reference-pages: unlisted new scoped-tree page" \
        "docs/extensions/new-surface.md" "page is not classified in docs/reference-pages.md" \
        "$work/reference-unclassified/mutated.log" bash "$root/tools/check-reference-pages.sh"
}

{
    root=$work/reference-listed-missing/repo
    make_reference_tree "$root"
    expect_success "check-reference-pages: listed-present fixture" \
        "$work/reference-listed-missing/baseline.log" bash "$root/tools/check-reference-pages.sh"
    rm -f -- "$root/docs/reference/thing.md"
    expect_failure "check-reference-pages: manifested page missing on disk" \
        "docs/reference-pages.md" "listed reference page 'docs/reference/thing.md' does not exist" \
        "$work/reference-listed-missing/mutated.log" bash "$root/tools/check-reference-pages.sh"
}

{
    root=$work/reference-empty-section/repo
    make_reference_tree "$root"
    expect_success "check-reference-pages: filled-section fixture" \
        "$work/reference-empty-section/baseline.log" bash "$root/tools/check-reference-pages.sh"
    # Keep every required heading, but leave Limitations with no body.
    cat >"$root/docs/reference/thing.md" <<'MD'
# thing

## Purpose

Scratch.

## Inputs

None.

## Outputs

None.

## Precision behaviour

None.

## Errors

None.

## Limitations

## Examples

None.
MD
    expect_failure "check-reference-pages: empty required section" \
        "docs/reference/thing.md" "required section 'Limitations' is empty" \
        "$work/reference-empty-section/mutated.log" bash "$root/tools/check-reference-pages.sh"
}

{
    root=$work/reference-no-reason/repo
    make_reference_tree "$root"
    expect_success "check-reference-pages: reasoned-exemption fixture" \
        "$work/reference-no-reason/baseline.log" bash "$root/tools/check-reference-pages.sh"
    awk '
        /`docs\/language\/guide\.md`/ {
            print "| `docs/language/guide.md` |  |"
            next
        }
        { print }
    ' "$root/docs/reference-pages.md" >"$root/docs/reference-pages.defect" &&
        mv -- "$root/docs/reference-pages.defect" "$root/docs/reference-pages.md"
    expect_failure "check-reference-pages: declared non-reference page without a reason" \
        "docs/reference-pages.md" "has no reason" \
        "$work/reference-no-reason/mutated.log" bash "$root/tools/check-reference-pages.sh"
}

# ---------------------------------------------------------------------------
# 18-21. docs/language/examples/run-examples.sh
# ---------------------------------------------------------------------------
{
    root=$work/language-undeclared/repo
    make_language_tree "$root"
    expect_success "language runner: declared-set fixture" \
        "$work/language-undeclared/baseline.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/language/examples/run-examples.sh"
    cat >"$root/docs/language/new-page.md" <<'MD'
# New page

```lovelace
1 + 1
```
```text
= 2 (Natural)
  _ = 2
```
MD
    expect_failure "language runner: undeclared page in the manifest" \
        "docs/language/new-page.md" "is not in the page manifest" \
        "$work/language-undeclared/mutated.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/language/examples/run-examples.sh"
}

{
    root=$work/language-declared-missing/repo
    make_language_tree "$root"
    expect_success "language runner: complete-page-set fixture" \
        "$work/language-declared-missing/baseline.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/language/examples/run-examples.sh"
    rm -f -- "$root/docs/language/plots.md"
    expect_failure "language runner: declared page missing on disk" \
        "docs/language/plots.md" "declared page is missing or unreadable" \
        "$work/language-declared-missing/mutated.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/language/examples/run-examples.sh"
}

{
    root=$work/language-zero-examples/repo
    make_language_tree "$root"
    expect_success "language runner: one-example fixture" \
        "$work/language-zero-examples/baseline.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/language/examples/run-examples.sh"
    printf '# README.md\n\nScratch page for the documentation-gate self-test.\n' \
        >"$root/docs/language/README.md"
    expect_failure "language runner: zero example pairs" \
        "README.md" "refusing to pass on zero work" \
        "$work/language-zero-examples/mutated.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/language/examples/run-examples.sh"
}

{
    root=$work/language-unpaired/repo
    make_language_tree "$root"
    expect_success "language runner: paired fixture" \
        "$work/language-unpaired/baseline.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/language/examples/run-examples.sh"
    cat >>"$root/docs/language/README.md" <<'MD'

```lovelace
1 + 1
```
MD
    expect_failure "language runner: unpaired lovelace block" \
        "docs/language/README.md" "lovelace block has no following text block" \
        "$work/language-unpaired/mutated.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/language/examples/run-examples.sh"
}

# ---------------------------------------------------------------------------
# 14-17. docs/extensions/examples/run-examples.sh
# ---------------------------------------------------------------------------
{
    root=$work/extensions-undeclared/repo
    make_extensions_tree "$root"
    expect_success "extensions runner: declared-set fixture" \
        "$work/extensions-undeclared/baseline.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/extensions/examples/run-examples.sh"
    cat >"$root/docs/extensions/new-page.md" <<'MD'
# New page

```lovelace
1 + 1
```
```text
= 2 (Natural)
```
MD
    expect_failure "extensions runner: undeclared page in the manifest" \
        "docs/extensions/new-page.md" "is not in the page manifest" \
        "$work/extensions-undeclared/mutated.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/extensions/examples/run-examples.sh"
}

{
    root=$work/extensions-declared-missing/repo
    make_extensions_tree "$root"
    expect_success "extensions runner: complete-page-set fixture" \
        "$work/extensions-declared-missing/baseline.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/extensions/examples/run-examples.sh"
    rm -f -- "$root/docs/extensions/symbolics.md"
    expect_failure "extensions runner: declared page missing on disk" \
        "docs/extensions/symbolics.md" "declared page is missing or unreadable" \
        "$work/extensions-declared-missing/mutated.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/extensions/examples/run-examples.sh"
}

{
    root=$work/extensions-zero-examples/repo
    make_extensions_tree "$root"
    expect_success "extensions runner: one-example fixture" \
        "$work/extensions-zero-examples/baseline.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/extensions/examples/run-examples.sh"
    printf '# dsp.md\n\nScratch page for the documentation-gate self-test.\n' \
        >"$root/docs/extensions/dsp.md"
    # Tree-level defect: no single page is at fault, so the diagnostic names the
    # page set (4 page(s)) rather than one file.
    expect_failure "extensions runner: zero example pairs" \
        "4 page(s)" "No examples were found in 4 page(s); nothing was verified." \
        "$work/extensions-zero-examples/mutated.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/extensions/examples/run-examples.sh"
}

{
    root=$work/extensions-unpaired/repo
    make_extensions_tree "$root"
    expect_success "extensions runner: paired fixture" \
        "$work/extensions-unpaired/baseline.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/extensions/examples/run-examples.sh"
    cat >>"$root/docs/extensions/README.md" <<'MD'

```lovelace
1 + 1
```
MD
    expect_failure "extensions runner: unpaired lovelace block" \
        "README.md" "no immediately following text block" \
        "$work/extensions-unpaired/mutated.log" \
        env CARGO_TARGET_DIR="$runner_target_dir" bash "$root/docs/extensions/examples/run-examples.sh"
}

# ---------------------------------------------------------------------------
# 26. CI wiring: delete one mandatory step from a copy of ci.yml and prove the
# guard fails. The real workflow is never modified, and the baseline run of the
# same guard against the real file proves the deleted step is the only change.
# ---------------------------------------------------------------------------
{
    root=$work/ci-step-removed
    mkdir -p -- "$root" || exit 2
    ci_source=$repo_root/.github/workflows/ci.yml

    awk '
        /^[[:space:]]*- name: Compile and run the crate-page examples[[:space:]]*$/ { skip = 1; next }
        skip && /^[[:space:]]*- (name|uses):/ { skip = 0 }
        skip { next }
        { print }
    ' "$ci_source" >"$root/ci-step-deleted.yml"
    if cmp -s -- "$ci_source" "$root/ci-step-deleted.yml"; then
        fail "ci wiring mutation" "the deleted-step fixture is identical to ci.yml; the step name did not match"
    fi

    expect_success "ci wiring: current workflow" "$root/baseline.log" \
        bash "$repo_root/tools/test-docs-gates.sh" --check-ci "$ci_source"
    expect_failure "ci wiring: mandatory docs step deleted" \
        "bash tools/check-crate-examples.sh" "the docs job does not run" \
        "$root/mutated.log" \
        bash "$repo_root/tools/test-docs-gates.sh" --check-ci "$root/ci-step-deleted.yml"
}

# ---------------------------------------------------------------------------
# 27. docs-report.sh: a gate that exits 0 but prints no DOCREPORT line is a
# failure. A silent pass is not evidence, and the total must record it.
# ---------------------------------------------------------------------------
{
    root=$work/docs-report-missing-summary
    make_docs_report_tree "$root"
    expect_success "docs-report: complete summaries" "$root/baseline.log" \
        bash "$root/tools/docs-report.sh"
    if ! grep -Fq -- 'DOCREPORT total gates=4 ok=4 failed=0' "$root/baseline.log"; then
        fail "docs-report: clean baseline total" "the total line is not gates=4 ok=4 failed=0"
    fi

    write_report_stub "$root" run-tutorials 'run-tutorials: OK (this gate forgot its DOCREPORT line)'
    expect_failure "docs-report: gate exits 0 with no DOCREPORT line" \
        "run-tutorials" "summary=missing" \
        "$root/mutated.log" bash "$root/tools/docs-report.sh"
    if ! grep -Fq -- 'DOCREPORT total gates=4 ok=3 failed=1' "$root/mutated.log"; then
        fail "docs-report: total after a missing summary" "the total line is not gates=4 ok=3 failed=1"
    fi
}

check_ci_wiring "$repo_root/.github/workflows/ci.yml"

# ---------------------------------------------------------------------------
# Verdict
# ---------------------------------------------------------------------------
printf '\ntest-docs-gates: %d mutation(s), %d passed, %d failed\n' \
    "$mutations" "$passed" "$failed"

if [ "$((passed + mutation_failed))" -ne "$mutations" ]; then
    printf 'test-docs-gates: FAIL (%d of %d mutations produced no verdict)\n' \
        "$((mutations - passed - mutation_failed))" "$mutations" >&2
    exit 1
fi
if [ "$failed" -gt 0 ]; then
    printf 'test-docs-gates: FAIL (%d problem(s))\n' "$failed" >&2
    exit 1
fi
printf 'test-docs-gates: OK (%d mutations, every gate failed for its intended reason)\n' \
    "$mutations"
exit 0
