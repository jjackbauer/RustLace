#!/usr/bin/env bash
# run-tutorials.sh - execute the four RustLace tutorials and check what they claim.
#
# The tutorials are executable contracts, not prose with samples. For every page
# under docs/tutorials/ named in the manifest below this script:
#
#   1. requires the page's "## Verification" section to use the project-root
#      `cargo run -p lovelace-run -- ...` command and to name the checked-in
#      example script under docs/tutorials/examples/;
#   2. requires the checked-in example script to be byte-for-byte the page's
#      "## Source" fence (line endings normalised only);
#   3. executes that script through lovelace-run and compares the result with
#      the page's "## Output" fence. The three text tutorials are compared as
#      typed transcripts; the reproducibility tutorial is compared field by
#      field through the JSON envelope. Only host information is normalised:
#      CRLF, trailing spaces, the absolute temporary directory in a plot path.
#      Numbers are never rounded, truncated or otherwise normalised;
#   4. for the linear-algebra page, recomputes det(A) and A * sol = b with
#      64-bit integer arithmetic that is independent of the engine, so a page
#      and an engine that are wrong in the same way still fail;
#   5. rejects the documenting host's old build path (/tmp/rl-target), a shell
#      here-document, or a Unix-only command on any page, so RQ-008 cannot
#      quietly regress.
#
# The expected set is asserted BY NAME and in both directions: a missing page
# or example is a failure, and so is an unlisted page, because either way one
# of the four tutorials would be skipped. There is no subset mode for the same
# reason.
#
# Usage (from the repository root, or any directory):
#   bash tools/run-tutorials.sh
#
# Runner lookup, in order:
#   $LOVELACE_RUN if set, then ${CARGO_TARGET_DIR:-<repo>/target}/{debug,release}/
#   lovelace-run[.exe], and finally `cargo run -q -p lovelace-run --` when cargo
#   is on PATH. The docs CI job runs `cargo build -p lovelace-run` first, so the
#   built binary is normally used on both Ubuntu and Windows (Git Bash).
#
# Nothing is written inside the repository: the run uses a mktemp directory that
# is removed on exit, and the plot directory it passes to the CLI is created by
# the CLI itself (proving --plot-dir is created on demand).
#
# Exit status: 0 every tutorial matches; 1 any tutorial, field, oracle or
# layout check failed; 2 setup failure (no runner, no scratch directory, bad
# usage, or an empty manifest).
set -u

LC_ALL=C
export LC_ALL

script_dir=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd) || exit 2
repo=$(CDPATH= cd -- "$script_dir/.." && pwd) || exit 2
cd -- "$repo" || exit 2

usage() {
    printf 'usage: bash tools/run-tutorials.sh\n'
    printf '  runs exactly the four tutorials named in this script and compares each\n'
    printf '  recorded output with the executed example; fails if one is missing\n'
}

for arg in "$@"; do
    case "$arg" in
        -h | --help)
            usage
            exit 0
            ;;
        *)
            printf 'run-tutorials: unknown argument: %s\n' "$arg" >&2
            usage >&2
            exit 2
            ;;
    esac
done

tutorials=(linear-algebra symbolic signal-processing reproducible)
expected_count=4
if [ "${#tutorials[@]}" -ne "$expected_count" ]; then
    printf 'run-tutorials: the manifest declares %d names, expected %d\n' \
        "${#tutorials[@]}" "$expected_count" >&2
    exit 2
fi

failures=0
oracles=0
fences_checked=0

fail() {
    # $1 tutorial or area, $2 message
    printf 'FAIL %s: %s\n' "$1" "$2" >&2
    failures=$((failures + 1))
}

setup_fail() {
    printf 'run-tutorials: %s\n' "$*" >&2
    exit 2
}

scratch=$(mktemp -d "${TMPDIR:-/tmp}/run-tutorials.XXXXXX") || \
    setup_fail "could not create a scratch directory under ${TMPDIR:-/tmp}"
trap 'rm -rf -- "$scratch"' EXIT HUP INT TERM

# ---------------------------------------------------------------------------
# Platform shape. Git Bash on windows-latest reports MINGW*, and a Windows
# build of lovelace-run has the .exe suffix; no other platform may fall back
# to it. MSYS translates POSIX paths for native executables inconsistently, so
# every path handed to lovelace-run goes through cygpath when it exists.
# ---------------------------------------------------------------------------
case "$(uname -s 2>/dev/null || printf unknown)" in
    MINGW* | MSYS* | CYGWIN*) exe_suffix=.exe ;;
    *) exe_suffix= ;;
esac

native_path() {
    if command -v cygpath >/dev/null 2>&1; then
        cygpath -w -- "$1"
    else
        printf '%s' "$1"
    fi
}

section_text() {
    # $1 page, $2 exact heading: the section's prose and fences, no heading.
    awk -v heading="$2" '
        { sub(/\r$/, "") }
        $0 == heading { inside = 1; next }
        inside && /^## / { exit }
        inside { print }
    ' "$1"
}

target_dir=${CARGO_TARGET_DIR:-$repo/target}
case "$target_dir" in
    /* | [A-Za-z]:[\\/]*) : ;;
    *) target_dir=$repo/$target_dir ;;
esac

runner=()
runner_kind=
if [ -n "${LOVELACE_RUN:-}" ]; then
    [ -f "$LOVELACE_RUN" ] || setup_fail "LOVELACE_RUN is set but is not a file: $LOVELACE_RUN"
    runner=("$LOVELACE_RUN")
    runner_kind="LOVELACE_RUN=$LOVELACE_RUN"
else
    for profile in debug release; do
        candidate=$target_dir/$profile/lovelace-run$exe_suffix
        if [ -f "$candidate" ]; then
            runner=("$candidate")
            runner_kind=$candidate
            break
        fi
    done
fi
if [ "${#runner[@]}" -eq 0 ]; then
    if command -v cargo >/dev/null 2>&1; then
        runner=(cargo run -q -p lovelace-run --)
        runner_kind='cargo run -p lovelace-run'
    else
        setup_fail "no lovelace-run under '$target_dir/{debug,release}' and cargo is not on PATH; build it with: cargo build -p lovelace-run"
    fi
fi

printf 'run-tutorials: expecting %d tutorials: %s\n' "$expected_count" "${tutorials[*]}"
printf 'run-tutorials: runner: %s\n' "$runner_kind"

# ---------------------------------------------------------------------------
# Manifest, in both directions. README.md is the only page under docs/tutorials
# that is not a tutorial: it is documentation about them and is checked for the
# host-path and shell-shape rules below, but it has no Source/Output pair.
# ---------------------------------------------------------------------------
shopt -s nullglob
page_paths=("$repo"/docs/tutorials/*.md)
shopt -u nullglob
if [ "${#page_paths[@]}" -eq 0 ]; then
    setup_fail "no Markdown pages under docs/tutorials"
fi

for name in "${tutorials[@]}"; do
    if [ ! -f "$repo/docs/tutorials/$name.md" ]; then
        fail inventory "expected tutorial '$name' has no page at docs/tutorials/$name.md"
    fi
done

for path in "${page_paths[@]}"; do
    base=${path##*/}
    [ "$base" = "README.md" ] && continue
    listed=0
    for name in "${tutorials[@]}"; do
        if [ "$base" = "$name.md" ]; then
            listed=1
            break
        fi
    done
    if [ "$listed" -eq 0 ]; then
        fail inventory "docs/tutorials/$base is not in the expected set of four; add it to the manifest or remove the page, so no tutorial is silently skipped"
    fi
done

# ---------------------------------------------------------------------------
# Static shape of the pages: the RQ-008 regressions this script exists to make
# impossible to reintroduce.
# ---------------------------------------------------------------------------
for path in "${page_paths[@]}"; do
    base=${path##*/}
    page=docs/tutorials/$base

    if grep -qF -- '/tmp/rl-target' "$path"; then
        fail "$page" "names the documenting host's build path /tmp/rl-target"
    elif grep -qF -- '/tmp' "$path"; then
        fail "$page" "contains a Unix-only temporary path; use the OS temporary directory or --plot-dir"
    fi
    if grep -qF -- '<<' "$path"; then
        fail "$page" "contains a shell here-document; the workflow must run unchanged in bash, PowerShell and Command Prompt"
    fi
    if [ "$base" != "README.md" ] && [ "$base" != "reproducible.md" ] &&
        grep -qF -- 'python3' "$path"; then
        fail "$page" "names python3; Python is needed only where the page states it as an explicit prerequisite"
    fi
done

if ! grep -qF -- 'ConvertFrom-Json' "$repo/docs/tutorials/reproducible.md"; then
    fail reproducible "the reproducibility page must keep its no-Python Windows route (ConvertFrom-Json)"
fi
if ! grep -qF -- 'python3' "$repo/docs/tutorials/reproducible.md"; then
    fail reproducible "the reproducibility page must keep a field-selection route"
fi

# Every tutorial must document the actual portable command, from the project
# root, naming the script this runner executes. A page that still says "run the
# binary at <some host path>" fails here even though its prose is otherwise
# well formed.
for name in "${tutorials[@]}"; do
    page_file=$repo/docs/tutorials/$name.md
    [ -f "$page_file" ] || continue
    verification=$(section_text "$page_file" '## Verification')
    if [ -z "$verification" ]; then
        fail "$name" "the page has no ## Verification section"
        continue
    fi
    case "$verification" in
        *'cargo run -p lovelace-run --'*) ;;
        *) fail "$name" "the ## Verification section does not use the project-root cargo run command" ;;
    esac
    case "$verification" in
        *"docs/tutorials/examples/$name.ls"*) ;;
        *) fail "$name" "the ## Verification section does not name docs/tutorials/examples/$name.ls" ;;
    esac
done

# ---------------------------------------------------------------------------
# Fenced-block extraction. The page structure is fixed by the tutorial
# template: "## Source" is followed by a ```lovelace fence, "## Output" by a
# ```text fence. A missing section, a missing fence or the wrong language is a
# failure, never an empty comparison that passes.
# ---------------------------------------------------------------------------
extract_block() {
    # $1 page, $2 exact heading, $3 fence language. Exit 0 with the body on
    # stdout; 3 when the section ends before a fence; 4 on the wrong language;
    # 5 on an unterminated fence.
    awk -v heading="$2" -v lang="$3" '
        { sub(/\r$/, "") }
        !in_section && $0 == heading { in_section = 1; next }
        in_section && !in_block && /^## / { exit 3 }
        in_section && !in_block && /^[ \t]*```/ {
            marker = $0
            sub(/^[ \t]*/, "", marker)
            sub(/[ \t]*$/, "", marker)
            if (marker != "```" lang) exit 4
            in_block = 1
            next
        }
        in_block {
            if ($0 ~ /^[ \t]*```[ \t]*$/) { found = 1; exit 0 }
            print
        }
        END { if (in_block && !found) exit 5 }
    ' "$1"
}

normalise_text() {
    # Only host-shape normalisation: drop CR from a Windows checkout and
    # trailing spaces, and trim blank lines at the edges. Internal whitespace
    # and every digit are preserved, so a wrong value cannot be normalised away.
    awk '
        { sub(/\r$/, ""); sub(/[ \t]+$/, ""); line[NR] = $0 }
        END {
            first = 1
            last = NR
            while (first <= last && line[first] == "") first++
            while (last >= first && line[last] == "") last--
            for (i = first; i <= last; i++) print line[i]
        }'
}

# ---------------------------------------------------------------------------
# JSON field extraction for the reproducibility envelope. A small JSON walker
# is used instead of python3/jq because the docs job guarantees bash, awk, sed
# and grep on both platforms; it emits "path<TAB>value" for the wanted scalar
# leaves. Malformed JSON is a failure, not an empty lookup.
# ---------------------------------------------------------------------------
json_emit() {
    # $1 space-separated wanted paths, $2 JSON file
    awk -v wanted="$1" '
        function skip_ws(   c) {
            while (i <= n) {
                c = substr(s, i, 1)
                if (c == " " || c == "\t" || c == "\n" || c == "\r") i++
                else break
            }
        }
        function walker_fail(msg) {
            printf "run-tutorials: JSON parse error near byte %d: %s\n", i, msg > "/dev/stderr"
            exit 2
        }
        function emit(path, value) {
            if (index(" " wanted " ", " " path " ") > 0)
                printf "%s\t%s\n", path, value
        }
        function parse_string(   out, c, esc) {
            i++
            out = ""
            while (i <= n) {
                c = substr(s, i, 1)
                if (c == "\"") { i++; return out }
                if (c == "\\") {
                    esc = substr(s, i + 1, 1)
                    if (esc == "\"" || esc == "\\" || esc == "/") out = out esc
                    else if (esc == "n") out = out "\n"
                    else if (esc == "t") out = out "\t"
                    else if (esc == "r") out = out "\r"
                    else if (esc == "u") { out = out substr(s, i, 6); i += 6; continue }
                    else out = out esc
                    i += 2
                    continue
                }
                out = out c
                i++
            }
            walker_fail("unterminated string")
        }
        function parse_scalar(path,   start, c) {
            start = i
            while (i <= n) {
                c = substr(s, i, 1)
                if (c == "," || c == "}" || c == "]" || c == " " || c == "\t" || c == "\n" || c == "\r") break
                i++
            }
            if (i == start) walker_fail("empty scalar")
            emit(path, substr(s, start, i - start))
        }
        function parse_object(path,   key, c) {
            i++
            skip_ws()
            if (substr(s, i, 1) == "}") { i++; return }
            while (1) {
                skip_ws()
                key = parse_string()
                skip_ws()
                if (substr(s, i, 1) != ":") walker_fail("expected a colon after an object key")
                i++
                parse_value(path "." key)
                skip_ws()
                c = substr(s, i, 1)
                if (c == ",") { i++; continue }
                if (c == "}") { i++; return }
                walker_fail("expected a comma or closing brace in an object")
            }
        }
        function parse_array(path,   idx, c) {
            i++
            skip_ws()
            if (substr(s, i, 1) == "]") { i++; return }
            idx = 0
            while (1) {
                parse_value(path "[" idx "]")
                idx++
                skip_ws()
                c = substr(s, i, 1)
                if (c == ",") { i++; continue }
                if (c == "]") { i++; return }
                walker_fail("expected a comma or closing bracket in an array")
            }
        }
        function parse_value(path,   c) {
            skip_ws()
            c = substr(s, i, 1)
            if (c == "{") { parse_object(path); return }
            if (c == "[") { parse_array(path); return }
            if (c == "\"") { emit(path, parse_string()); return }
            parse_scalar(path)
        }
        { s = s $0 "\n" }
        END {
            n = length(s)
            i = 1
            parse_value("")
            skip_ws()
            if (i <= n) walker_fail("trailing data after the document")
        }
    ' "$2"
}

json_scalar() {
    # $1 fields file from json_emit, $2 path
    awk -v want="$2" '
        BEGIN { FS = "\t" }
        $1 == want {
            value = $0
            sub(/^[^\t]*\t/, "", value)
            print value
            found = 1
            exit
        }
        END { exit(found ? 0 : 1) }
    ' "$1"
}

# ---------------------------------------------------------------------------
# The independent linear-algebra oracle. It reads the observed typed output,
# takes A, b, sol, gap and det from it, and checks with 64-bit integer
# arithmetic (bash) that det = ad - bc, that A * sol = b exactly at the
# decimal scale of sol, and that gap is exactly zero. It does not call the
# engine, and it does not read the page's expected values, so a wrong engine
# and a wrong transcript cannot agree their way to a pass.
# ---------------------------------------------------------------------------
run_linear_algebra_oracle() {
    local observed=$1 values det scale factor x_int y_int
    local a11 a12 a21 a22 b1 b2 g1 g2 d_obs x y

    values=$(awk '
        BEGIN { bad = 0 }
        function trim(v) { sub(/^[ \t]+/, "", v); sub(/[ \t]+$/, "", v); return v }
        function decimals(v) { return (index(v, ".") > 0) ? length(v) - index(v, ".") : 0 }
        function scaled(v, scale,   neg, dot, whole, frac, k) {
            neg = 0
            if (substr(v, 1, 1) == "-") { neg = 1; v = substr(v, 2) }
            dot = index(v, ".")
            if (dot > 0) { whole = substr(v, 1, dot - 1); frac = substr(v, dot + 1) }
            else { whole = v; frac = "" }
            if (whole == "") whole = "0"
            while (length(frac) < scale) frac = frac "0"
            frac = substr(frac, 1, scale)
            k = whole frac
            sub(/^0+/, "", k)
            if (k == "") k = "0"
            return (neg ? "-" : "") k
        }
        /^[ \t]*A = \[\[/ {
            s = $0
            sub(/^[ \t]*A = \[\[/, "", s)
            sub(/\]\][ \t]*$/, "", s)
            if (split(s, rows, /\], \[/) != 2) { bad = 1; exit }
            if (split(rows[1], r1, ",") != 2) { bad = 1; exit }
            if (split(rows[2], r2, ",") != 2) { bad = 1; exit }
            a11 = trim(r1[1]); a12 = trim(r1[2])
            a21 = trim(r2[1]); a22 = trim(r2[2])
            have_a = 1
        }
        /^[ \t]*b = \[\[/ {
            s = $0
            sub(/^[ \t]*b = \[\[/, "", s)
            sub(/\]\][ \t]*$/, "", s)
            if (split(s, rows, /\], \[/) != 2) { bad = 1; exit }
            b1 = trim(rows[1]); b2 = trim(rows[2])
            have_b = 1
        }
        /^[ \t]*sol = \[\[/ {
            s = $0
            sub(/^[ \t]*sol = \[\[/, "", s)
            sub(/\]\][ \t]*$/, "", s)
            if (split(s, rows, /\], \[/) != 2) { bad = 1; exit }
            x = trim(rows[1]); y = trim(rows[2])
            have_sol = 1
        }
        /^[ \t]*gap = \[\[/ {
            s = $0
            sub(/^[ \t]*gap = \[\[/, "", s)
            sub(/\]\][ \t]*$/, "", s)
            if (split(s, rows, /\], \[/) != 2) { bad = 1; exit }
            g1 = trim(rows[1]); g2 = trim(rows[2])
            have_gap = 1
        }
        /^[ \t]*d = / {
            s = $0
            sub(/^[ \t]*d = /, "", s)
            d_obs = trim(s)
            have_d = 1
        }
        END {
            if (bad) exit 4
            if (!have_a || !have_b || !have_sol || !have_gap || !have_d) exit 5
            scale = decimals(x)
            if (decimals(y) > scale) scale = decimals(y)
            x_int = scaled(x, scale)
            y_int = scaled(y, scale)
            printf "%s %s %s %s %s %s %s %s %s %d %s %s\n", \
                a11, a12, a21, a22, b1, b2, g1, g2, d_obs, scale, x_int, y_int
        }
    ' "$observed") || {
        fail linear-algebra "oracle: could not read A, b, sol, gap and d from the observed transcript"
        return 1
    }

    read -r a11 a12 a21 a22 b1 b2 g1 g2 d_obs scale x_int y_int <<<"$values"

    if [ -z "${d_obs:-}" ]; then
        fail linear-algebra "oracle: extracted an empty determinant"
        return 1
    fi

    det=$((a11 * a22 - a12 * a21))
    factor=$((10 ** scale))
    oracles=$((oracles + 1))

    if [ "$det" -ne "$d_obs" ]; then
        fail linear-algebra "oracle: det(A) is $det, but the transcript reports d = $d_obs"
        return 1
    fi
    if [ $((a11 * x_int + a12 * y_int)) -ne $((b1 * factor)) ]; then
        fail linear-algebra "oracle: row 1 of A * sol is not b at scale 10^$scale (sol = [$x_int, $y_int] / 10^$scale)"
        return 1
    fi
    if [ $((a21 * x_int + a22 * y_int)) -ne $((b2 * factor)) ]; then
        fail linear-algebra "oracle: row 2 of A * sol is not b at scale 10^$scale (sol = [$x_int, $y_int] / 10^$scale)"
        return 1
    fi
    if [ "$g1" -ne 0 ] || [ "$g2" -ne 0 ]; then
        fail linear-algebra "oracle: gap is not the exact zero vector (observed [$g1, $g2])"
        return 1
    fi

    printf 'ORACLE linear-algebra: det(A) = %d; A * sol = b exactly at scale 10^%d; gap = [0, 0]\n' \
        "$det" "$scale"
    return 0
}

# ---------------------------------------------------------------------------
# Run one tutorial. $1 is the manifest name.
# ---------------------------------------------------------------------------
run_text_tutorial() {
    local name=$1 page example status out err
    page=docs/tutorials/$name.md
    example=docs/tutorials/examples/$name.ls
    out=$scratch/$name.out
    err=$scratch/$name.err

    # Source fence -> scratch file, and the checked-in example must be the
    # same script. If either half is missing the tutorial is a failure, not a
    # skip.
    if extract_block "$repo/$page" '## Source' lovelace >"$scratch/$name.source"; then
        :
    else
        status=$?
        fail "$name" "the ## Source section has no readable lovelace fence (extract status $status)"
        return
    fi
    if [ ! -s "$scratch/$name.source" ]; then
        fail "$name" "the ## Source fence is empty"
        return
    fi
    if [ ! -f "$repo/$example" ]; then
        fail "$name" "missing checked-in script $example"
        return
    fi
    tr -d '\r' <"$repo/$example" >"$scratch/$name.example"
    if ! cmp -s "$scratch/$name.source" "$scratch/$name.example"; then
        fail "$name" "the ## Source fence and $example differ; a reader copying the page would run a different script"
        return
    fi
    fences_checked=$((fences_checked + 1))

    if extract_block "$repo/$page" '## Output' text >"$scratch/$name.expected"; then
        :
    else
        status=$?
        fail "$name" "the ## Output section has no readable text fence (extract status $status)"
        return
    fi
    if [ ! -s "$scratch/$name.expected" ]; then
        fail "$name" "the ## Output fence is empty"
        return
    fi

    if "${runner[@]}" --file "$(native_path "$scratch/$name.example")" --text \
        >"$out" 2>"$err"; then
        :
    else
        status=$?
        fail "$name" "lovelace-run exited $status: $(sed -n '1p' "$err")"
        return
    fi

    normalise_text <"$scratch/$name.expected" >"$scratch/$name.expected.norm"
    normalise_text <"$out" >"$scratch/$name.observed.norm"

    if cmp -s "$scratch/$name.expected.norm" "$scratch/$name.observed.norm"; then
        fences_checked=$((fences_checked + 1))
        printf 'PASS %s: docs/tutorials/%s.md Source -> Output match\n' "$name" "$name"
    else
        printf '\nFAIL %s: output does not match the recorded transcript\n' "$name" >&2
        printf '  expected:\n' >&2
        sed 's/^/    /' "$scratch/$name.expected.norm" >&2
        printf '  observed:\n' >&2
        sed 's/^/    /' "$scratch/$name.observed.norm" >&2
        failures=$((failures + 1))
        return
    fi

    if [ "$name" = "linear-algebra" ]; then
        run_linear_algebra_oracle "$out" || return
    fi
}

run_reproducible_tutorial() {
    local name=reproducible page example out err status
    local fields expected_fields exp_plot obs_plot plot_base shell_plot first_line
    local wanted='.ok .result.display .result.kind .result.typed .result.structured.exact .artifacts[0].kind .artifacts[0].title .artifacts[0].path'
    page=docs/tutorials/$name.md
    example=docs/tutorials/examples/$name.ls
    out=$scratch/$name.out
    err=$scratch/$name.err
    fields=$scratch/$name.fields

    if extract_block "$repo/$page" '## Source' lovelace >"$scratch/$name.source"; then
        :
    else
        status=$?
        fail "$name" "the ## Source section has no readable lovelace fence (extract status $status)"
        return
    fi
    if [ ! -s "$scratch/$name.source" ]; then
        fail "$name" "the ## Source fence is empty"
        return
    fi
    if [ ! -f "$repo/$example" ]; then
        fail "$name" "missing checked-in script $example"
        return
    fi
    tr -d '\r' <"$repo/$example" >"$scratch/$name.example"
    if ! cmp -s "$scratch/$name.source" "$scratch/$name.example"; then
        fail "$name" "the ## Source fence and $example differ; a reader copying the page would run a different script"
        return
    fi
    fences_checked=$((fences_checked + 1))

    if extract_block "$repo/$page" '## Output' text >"$scratch/$name.expected"; then
        :
    else
        status=$?
        fail "$name" "the ## Output section has no readable text fence (extract status $status)"
        return
    fi
    if [ ! -s "$scratch/$name.expected" ]; then
        fail "$name" "the ## Output fence is empty"
        return
    fi

    # The plot directory does NOT exist yet: the CLI must create it, which the
    # directory assertion below records. It is inside the OS temp root, so no
    # repository path is used and both platforms exercise the same code.
    if "${runner[@]}" --file "$(native_path "$scratch/$name.example")" \
        --omit-functions --omit-variables \
        --plot-dir "$(native_path "$scratch/plots")" \
        >"$out" 2>"$err"; then
        :
    else
        status=$?
        fail "$name" "lovelace-run exited $status: $(sed -n '1p' "$err")"
        return
    fi

    if ! json_emit "$wanted" "$out" >"$fields"; then
        fail "$name" "the JSON envelope did not parse as JSON"
        return
    fi
    if ! json_emit '.display .kind .exact .plot' "$scratch/$name.expected" \
        >"$scratch/$name.expected.fields"; then
        fail "$name" "the recorded ## Output block is not the JSON object the page documents"
        return
    fi
    fences_checked=$((fences_checked + 1))

    if ! compare_json_field display "$scratch/$name.expected.fields" .display "$fields" .result.display; then
        return
    fi
    if ! compare_json_field kind "$scratch/$name.expected.fields" .kind "$fields" .result.kind; then
        return
    fi
    if ! compare_json_field exact "$scratch/$name.expected.fields" .exact "$fields" .result.structured.exact; then
        return
    fi

    local ok typed artifact_kind artifact_title
    ok=$(json_scalar "$fields" .ok) || {
        fail "$name" "the envelope has no ok field"
        return
    }
    typed=$(json_scalar "$fields" .result.typed) || {
        fail "$name" "the envelope has no result.typed field"
        return
    }
    artifact_kind=$(json_scalar "$fields" '.artifacts[0].kind') || {
        fail "$name" "the envelope has no artifacts[0].kind field"
        return
    }
    artifact_title=$(json_scalar "$fields" '.artifacts[0].title') || {
        fail "$name" "the envelope has no artifacts[0].title field"
        return
    }

    if [ "$ok" != "true" ]; then
        fail "$name" "the envelope reports ok=$ok"
        return
    fi
    if [ "$typed" != "5 (Real)" ]; then
        fail "$name" "result.typed is '$typed', expected '5 (Real)' (the typed value of det(A))"
        return
    fi
    if [ "$artifact_kind" != "plot" ] || [ "$artifact_title" != "squares" ]; then
        fail "$name" "the artifact is '$artifact_kind' titled '$artifact_title', expected a 'plot' titled 'squares'"
        return
    fi

    obs_plot=$(json_scalar "$fields" '.artifacts[0].path') || {
        fail "$name" "the envelope has no artifacts[0].path field"
        return
    }
    exp_plot=$(json_scalar "$scratch/$name.expected.fields" .plot) || {
        fail "$name" "the recorded output has no plot field"
        return
    }
    plot_base=${obs_plot##*\\}
    plot_base=${plot_base##*/}
    if [ -z "$plot_base" ]; then
        fail "$name" "the artifact path '$obs_plot' has no file name"
        return
    fi
    if [ "$exp_plot" != "<temp-dir>/$plot_base" ]; then
        fail "$name" "recorded plot value is '$exp_plot', expected '<temp-dir>/$plot_base'"
        return
    fi

    # The plot file must be the file the envelope names, outside the
    # repository, and a real SVG. The absolute directory is host information;
    # everything about the artifact's identity is checked.
    shell_plot=$obs_plot
    if command -v cygpath >/dev/null 2>&1; then
        shell_plot=$(cygpath -u -- "$obs_plot")
    else
        shell_plot=${obs_plot//\\//}
    fi
    case "$shell_plot" in
        "$repo"/*)
            fail "$name" "the plot was written inside the repository: $shell_plot"
            return
            ;;
    esac
    if [ ! -s "$shell_plot" ]; then
        fail "$name" "the envelope names '$obs_plot' but no non-empty file is there"
        return
    fi
    first_line=$(sed -n '1p' "$shell_plot")
    case "$first_line" in
        '<svg'*) ;;
        *)
            fail "$name" "the plot artifact does not start with an <svg> element"
            return
            ;;
    esac
    if [ ! -d "$scratch/plots" ]; then
        fail "$name" "--plot-dir was not created on demand"
        return
    fi

    printf 'PASS %s: docs/tutorials/%s.md fields display/kind/exact/plot match; plot artifact exists outside the repository\n' \
        "$name" "$name"
}

compare_json_field() {
    # $1 label, $2 expected fields file, $3 expected path, $4 observed fields
    # file, $5 observed path
    local label=$1 expf=$2 expp=$3 obsf=$4 obsp=$5 expected observed
    if ! expected=$(json_scalar "$expf" "$expp"); then
        fail reproducible "the recorded ## Output has no '$expp' field"
        return 1
    fi
    if ! observed=$(json_scalar "$obsf" "$obsp"); then
        fail reproducible "the envelope has no '$obsp' field"
        return 1
    fi
    if [ "$expected" != "$observed" ]; then
        fail reproducible "$label: recorded '$expected', observed '$observed'"
        return 1
    fi
    return 0
}

# ---------------------------------------------------------------------------
# The fixed set, always all four. A page or example that disappears fails
# above; a page that is added but unlisted fails above; nothing is skipped.
# ---------------------------------------------------------------------------
for name in "${tutorials[@]}"; do
    if [ ! -f "$repo/docs/tutorials/$name.md" ]; then
        continue
    fi
    if [ "$name" = "reproducible" ]; then
        run_reproducible_tutorial
    else
        run_text_tutorial "$name"
    fi
done

printf '\nrun-tutorials: %d tutorials checked (%s)\n' "$expected_count" "${tutorials[*]}"
printf 'run-tutorials: %d independent oracle(s)\n' "$oracles"

docreport_status=ok
if [ "$failures" -gt 0 ]; then
    printf 'run-tutorials: FAIL (%d failure(s))\n' "$failures"
    docreport_status=fail
else
    printf 'run-tutorials: OK\n'
fi

# The machine-readable evidence line (RQ-023), additive and last on stdout.
# `fences` counts the Source/Output fences verified in a pair: one by the
# checked-in-example comparison and one by the transcript comparison.
printf 'DOCREPORT gate=run-tutorials status=%s tutorials=%d fences=%d oracles=%d failures=%d\n' \
    "$docreport_status" "$expected_count" "$fences_checked" "$oracles" "$failures"

if [ "$docreport_status" = fail ]; then
    exit 1
fi
exit 0
