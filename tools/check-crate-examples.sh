#!/usr/bin/env bash
# check-crate-examples.sh - compile and run the Rust fences in docs/crates/*.md.
#
# Usage: bash tools/check-crate-examples.sh [--keep]
#
# Every fenced ```rust block in every page under docs/crates/ is extracted,
# wrapped in fn main(), assembled into one throwaway Cargo package with path
# dependencies on the workspace crates, compiled, and executed. A compile
# error or a failed assertion is printed as page:line, mapped back from the
# generated file to the Markdown page, and the script exits non-zero. This
# gate is independent of the link and heading checks: an example whose API
# has drifted is a build failure here even when check-docs.sh passes.
#
# A fence that is intentionally illustrative and cannot compile must be
# annotated on the line immediately above its opening fence:
#
#     <!-- not-compiled: <reason> -->
#     ```rust
#
# Every exclusion is reported by page:line and reason, so the exemption list
# is auditable. An annotation that is not immediately above a rust fence, or
# that carries an empty reason, is itself a failure: an exemption cannot rot
# silently.
#
# Exit status:
#   0  every included fence compiled and ran, every exclusion was reported
#   1  a compile error, a failed assertion, or an annotation problem
#   2  bad usage, a missing docs/crates tree, or no rust fence at all
#      (zero work is not a pass)
#
# The script needs bash, awk, sed, grep, mktemp and a cargo it locates itself:
# $CARGO, then cargo on PATH, then $HOME/.cargo/bin/cargo, then
# %USERPROFILE%/.cargo/bin/cargo.exe under Git Bash. It uses only workspace
# path dependencies: no network access, no external crates. It runs from any
# working directory and tolerates CRLF Markdown.
#
# Every path written into a generated file (today only the temporary
# Cargo.toml, whose [workspace] table has no members) is converted with
# `cygpath -w` when cygpath is on PATH. Git Bash, MSYS and Cygwin translate a
# native child's argv and path-like environment variables, but never the
# contents of a file the child reads, so a POSIX path in the manifest is
# opened by Windows cargo as C:\d\... and fails. See toml_path below.

set -u

usage() {
    printf 'usage: bash tools/check-crate-examples.sh [--keep]\n'
    printf '  compiles and runs every ```rust fence under docs/crates/\n'
    printf '  --keep   keep the generated crate and print its path\n'
}

keep=0
for arg in "$@"; do
    case "$arg" in
        -h | --help)
            usage
            exit 0
            ;;
        --keep) keep=1 ;;
        *)
            printf 'check-crate-examples: unknown argument: %s\n' "$arg" >&2
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

pages_dir=docs/crates
if [ ! -d "$pages_dir" ]; then
    printf 'check-crate-examples: no %s/ directory under %s\n' "$pages_dir" "$repo_root" >&2
    exit 2
fi

pages=()
for candidate in "$pages_dir"/*.md; do
    [ -f "$candidate" ] && pages+=("$candidate")
done
if [ "${#pages[@]}" -eq 0 ]; then
    printf 'check-crate-examples: no Markdown pages under %s/\n' "$pages_dir" >&2
    exit 2
fi

# ---------------------------------------------------------------------------
# Locate cargo (OQ-002). A shell can run this gate without rustup's bin
# directory on PATH: a bare WSL shell, or Git Bash started from a host that
# installed Rust for another shell. The resolution order is fixed and every
# place tried is reported when none works. An absent compiler is exit 2,
# never a skipped example.
#   1. $CARGO, when it is set and executable
#   2. cargo on PATH
#   3. $HOME/.cargo/bin/cargo (the rustup default on Unix)
#   4. $USERPROFILE/.cargo/bin/cargo.exe (the rustup default in Git Bash)
# ---------------------------------------------------------------------------
cargo_bin=
cargo_source=

if [ -n "${CARGO:-}" ]; then
    if [ -x "$CARGO" ] || command -v "$CARGO" >/dev/null 2>&1; then
        cargo_bin=$CARGO
        cargo_source='$CARGO'
    else
        printf 'check-crate-examples: CARGO=%s is not executable; ignoring it\n' "$CARGO" >&2
    fi
fi

if [ -z "$cargo_bin" ]; then
    if candidate=$(command -v cargo 2>/dev/null) && [ -n "$candidate" ]; then
        cargo_bin=$candidate
        cargo_source='PATH'
    fi
fi

if [ -z "$cargo_bin" ] && [ -n "${HOME:-}" ] && [ -x "$HOME/.cargo/bin/cargo" ]; then
    cargo_bin=$HOME/.cargo/bin/cargo
    cargo_source='$HOME/.cargo/bin/cargo'
fi

if [ -z "$cargo_bin" ] && [ -n "${USERPROFILE:-}" ]; then
    cargo_win=$USERPROFILE/.cargo/bin/cargo.exe
    # Git Bash reports USERPROFILE as a native Windows path; cygpath gives the
    # POSIX path its test builtin and exec accept. Without cygpath the joined
    # path is still valid for a real Git Bash, which understands both shapes.
    if command -v cygpath >/dev/null 2>&1; then
        cargo_win=$(cygpath -u -- "$cargo_win" 2>/dev/null || printf '%s' "$cargo_win")
    fi
    if [ -x "$cargo_win" ]; then
        cargo_bin=$cargo_win
        cargo_source='%USERPROFILE%/.cargo/bin/cargo.exe'
    fi
fi

if [ -z "$cargo_bin" ]; then
    printf 'check-crate-examples: cargo not found; tried, in order:\n' >&2
    printf '  1. CARGO=%s\n' "${CARGO:-(unset)}" >&2
    printf '  2. cargo on PATH\n' >&2
    printf '  3. %s/.cargo/bin/cargo\n' "${HOME:-<HOME unset>}" >&2
    printf '  4. %s/.cargo/bin/cargo.exe (Git Bash)\n' "${USERPROFILE:-<USERPROFILE unset>}" >&2
    printf 'check-crate-examples: set CARGO to the cargo executable, put cargo on PATH,\n' >&2
    printf 'check-crate-examples: or install Rust from https://rustup.rs\n' >&2
    exit 2
fi

printf 'check-crate-examples: cargo: %s (from %s)\n' "$cargo_bin" "$cargo_source"

# mktemp lives in the OS temp directory, never in the repository, so a run
# leaves no artifact behind unless --keep asks for one.
tmp=$(mktemp -d "${TMPDIR:-/tmp}/rl-crate-examples.XXXXXX") || exit 2
cleanup() {
    if [ "$keep" -eq 1 ]; then
        printf 'check-crate-examples: kept temporary crate at %s\n' "$tmp"
    else
        rm -rf -- "$tmp"
    fi
}
trap cleanup EXIT HUP INT TERM

mkdir -p "$tmp/fences" "$tmp/src/bin"
manifest=$tmp/fences.tsv
problems=$tmp/problems.tsv
: > "$manifest"
: > "$problems"

# ---------------------------------------------------------------------------
# Parse every page. The awk program reports one manifest record per rust fence
# and writes the fence body to its own file. It strips a CR so a Windows
# checkout behaves like an LF one.
# ---------------------------------------------------------------------------
for page in "${pages[@]}"; do
    base=$(basename -- "$page" .md)
    awk -v page="$page" \
        -v base="$base" \
        -v outdir="$tmp/fences" \
        -v manifest="$manifest" \
        -v problems="$problems" '
        function bodyfile(n) {
            return outdir "/" base "__" n ".rs"
        }
        BEGIN {
            in_rust = 0
            in_other = 0
            pending = 0
        }
        {
            raw = $0
            sub(/\r$/, "", raw)
            n = NR

            if (in_rust) {
                if (raw ~ /^[ \t]*```[ \t]*$/) {
                    close(body)
                    printf "%s\t%d\t%d\t%d\t%s\t%s\n", page, start, n, excluded, (reason == "" ? "-" : reason), body >> manifest
                    in_rust = 0
                    next
                }
                print raw > body
                next
            }
            if (in_other) {
                if (raw ~ /^[ \t]*```[ \t]*$/) {
                    in_other = 0
                }
                next
            }
            if (pending) {
                if (raw ~ /^[ \t]*```rust[ \t]*$/) {
                    in_rust = 1
                    start = n
                    excluded = 1
                    body = bodyfile(n)
                    printf "" > body
                    pending = 0
                    next
                }
                printf "UNATTACHED\t%s\t%d\t%s\n", page, ann_line, reason >> problems
                pending = 0
                # Fall through: the current line may still open another fence.
            }
            if (raw ~ /^[ \t]*<!--[ \t]*not-compiled:/) {
                reason = raw
                sub(/^[ \t]*<!--[ \t]*not-compiled:[ \t]*/, "", reason)
                sub(/[ \t]*-->[ \t]*$/, "", reason)
                if (reason == "") {
                    printf "EMPTYREASON\t%s\t%d\n", page, n >> problems
                    next
                }
                gsub(/\t/, " ", reason)
                pending = 1
                ann_line = n
                next
            }
            if (raw ~ /^[ \t]*```rust[ \t]*$/) {
                in_rust = 1
                start = n
                excluded = 0
                reason = ""
                body = bodyfile(n)
                printf "" > body
                next
            }
            if (raw ~ /^[ \t]*```/) {
                in_other = 1
                other_start = n
                next
            }
        }
        END {
            if (in_rust) {
                printf "UNCLOSED\t%s\t%d\n", page, start >> problems
            }
            if (in_other) {
                printf "UNCLOSED\t%s\t%d\n", page, other_start >> problems
            }
            if (pending) {
                printf "UNATTACHED\t%s\t%d\t%s\n", page, ann_line, reason >> problems
            }
        }
    ' "$page"
done

# ---------------------------------------------------------------------------
# Annotation problems are conformance failures, not setup errors.
# ---------------------------------------------------------------------------
fences=$(wc -l < "$manifest" | tr -d ' ')
if [ -s "$problems" ]; then
    while IFS=$'\t' read -r kind page line reason; do
        case "$kind" in
            UNATTACHED)
                printf 'FAIL %s:%s: not-compiled annotation is not immediately above a ```rust fence (reason: %s)\n' \
                    "$page" "$line" "$reason" >&2
                ;;
            EMPTYREASON)
                printf 'FAIL %s:%s: not-compiled annotation has an empty reason\n' "$page" "$line" >&2
                ;;
            UNCLOSED)
                printf 'FAIL %s:%s: fence is never closed\n' "$page" "$line" >&2
                ;;
            *)
                printf 'FAIL %s:%s: %s\n' "$page" "$line" "$kind" >&2
                ;;
        esac
    done < "$problems"
    printf 'check-crate-examples: annotation audit failed\n' >&2
    annotation_problems=$(wc -l < "$problems" | tr -d ' ')
    printf 'DOCREPORT gate=check-crate-examples status=fail pages=%d fences=%d compiled=0 excluded=0 failures=%d\n' \
        "${#pages[@]}" "$fences" "$annotation_problems"
    exit 1
fi

if [ "$fences" -eq 0 ]; then
    printf 'check-crate-examples: no ```rust fence under %s/; refusing to pass on zero work\n' "$pages_dir" >&2
    exit 2
fi

# ---------------------------------------------------------------------------
# Build the throwaway crate metadata. Path dependencies are the workspace
# crates the fences are allowed to exercise; every path must exist.
# ---------------------------------------------------------------------------
deps="lovelace-array lovelace-int lovelace-json lovelace-lang lovelace-nat lovelace-real lovelace-symbolics"
for dep in $deps; do
    if [ ! -f "$dep/Cargo.toml" ]; then
        printf 'check-crate-examples: workspace crate %s/Cargo.toml is missing\n' "$dep" >&2
        exit 2
    fi
done

# ---------------------------------------------------------------------------
# Native path shape for the generated manifest. The temporary Cargo.toml is
# read by the native cargo, not by this shell: a Git Bash, MSYS or Cygwin
# shell reports the repository as /d/a/RustLace/RustLace, and it converts
# that spelling for a native child's argv and path-like environment
# variables, but never for the contents of a file the child reads. Written
# unchanged, the path is opened by Windows cargo as
# C:\d\a\RustLace\RustLace\lovelace-array, which does not exist (os error 3).
# Converting with `cygpath -w` whenever it exists, and using the absolute
# path unchanged elsewhere (Linux, macOS, WSL), makes the manifest native on
# every host. The failure branch below explains the shape if it ever recurs.
# ---------------------------------------------------------------------------
cygpath_bin=
if command -v cygpath >/dev/null 2>&1; then
    cygpath_bin=$(command -v cygpath)
fi

# toml_path ABSOLUTE_PATH - the value to put in a TOML basic string. On a
# host with cygpath the path is converted to its native spelling first, then
# escaped: the value is a TOML basic string, where a backslash introduces an
# escape, so the backslashes `cygpath -w` produces must be doubled. On every
# other host the absolute path is already native and passes through.
toml_path() {
    p=$1
    if [ -n "$cygpath_bin" ]; then
        p=$("$cygpath_bin" -w -- "$p") || {
            printf 'check-crate-examples: cygpath -w failed for %s\n' "$1" >&2
            return 1
        }
    fi
    printf '%s' "$p" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

manifest_file=$tmp/Cargo.toml
{
    printf '[package]\n'
    printf 'name = "rustlace-crate-examples-check"\n'
    printf 'version = "0.0.0"\n'
    printf 'edition = "2024"\n'
    printf 'publish = false\n\n'
    # No member paths: this table only stops the generated package from
    # joining the enclosing workspace.
    printf '[workspace]\n\n'
    printf '[dependencies]\n'
    for dep in $deps; do
        dep_path=$(toml_path "$repo_root/$dep") || exit 2
        printf '%s = { path = "%s" }\n' "$dep" "$dep_path"
    done
} > "$manifest_file"

# ---------------------------------------------------------------------------
# Generate one binary per included fence and a map from generated file to
# the Markdown page line, so diagnostics can be reported as page:line.
# ---------------------------------------------------------------------------
map=$tmp/map.tsv
run_list=$tmp/run.tsv
: > "$map"
: > "$run_list"
index=0
compiled=0
excluded=0

while IFS=$'\t' read -r page start end is_excluded reason body; do
    if [ "$is_excluded" -eq 1 ]; then
        excluded=$((excluded + 1))
        printf 'EXCLUDED %s:%s (reason: %s)\n' "$page" "$start" "$reason"
        continue
    fi
    index=$((index + 1))
    bin=$(printf 'fence_%02d' "$index")
    {
        printf '// generated by tools/check-crate-examples.sh from %s:%s\n' "$page" "$start"
        printf 'fn main() {\n'
        cat -- "$body"
        printf '}\n'
    } > "$tmp/src/bin/$bin.rs"
    printf '%s\t%s\t%s\n' "$bin.rs" "$page" "$start" >> "$map"
    printf '%s\t%s\t%s\n' "$bin" "$page" "$start" >> "$run_list"
    compiled=$((compiled + 1))
done < "$manifest"

if [ "$compiled" -eq 0 ]; then
    printf 'check-crate-examples: every rust fence is excluded; refusing to pass on zero compiled examples\n' >&2
    exit 2
fi

# ---------------------------------------------------------------------------
# Compile. Relative paths in rustc diagnostics are mapped back to the page.
# ---------------------------------------------------------------------------
target_dir=${CARGO_TARGET_DIR:-$tmp/target}
case "$target_dir" in
    /* | [A-Za-z]:[\\/]*) ;;
    *) target_dir=$tmp/$target_dir ;;
esac

map_diagnostics() {
    awk -v mapfile="$map" '
        BEGIN {
            FS = "\t"
            while ((getline line < mapfile) > 0) {
                split(line, a, "\t")
                page[a[1]] = a[2]
                start[a[1]] = a[3]
            }
        }
        function mdline(key, generated,   s) {
            s = start[key] + 0
            if (generated < 3) {
                return s
            }
            return s + generated - 2
        }
        {
            while (match($0, /src[\/\\]bin[\/\\]fence_[0-9][0-9]\.rs:[0-9]+(:[0-9]+)?/)) {
                match_text = substr($0, RSTART, RLENGTH)
                key = match_text
                sub(/^.*[\/\\]/, "", key)
                sub(/:.*$/, "", key)
                rest = substr(match_text, index(match_text, key) + length(key))
                sub(/^:/, "", rest)
                split(rest, parts, ":")
                generated = parts[1] + 0
                col = (parts[2] == "" ? "" : ":" parts[2])
                replacement = page[key] ":" mdline(key, generated) col
                $0 = substr($0, 1, RSTART - 1) replacement substr($0, RSTART + RLENGTH)
            }
            print
        }
    '
}

build_log=$tmp/build.log
(
    cd -- "$tmp" || exit 2
    CARGO_TARGET_DIR="$target_dir" "$cargo_bin" build --quiet --bins
) > "$build_log" 2>&1
build_code=$?
if [ "$build_code" -ne 0 ]; then
    map_diagnostics < "$build_log" >&2
    # A path the generated manifest records but the native cargo cannot open
    # is a different defect from an API drift, and cargo's "os error 3" does
    # not say that the missing file is a path this script wrote. Only a host
    # with cygpath can have had a POSIX path converted, so the explanation is
    # gated on it. This is additive: the existing messages and the DOCREPORT
    # line below are unchanged.
    if [ -n "$cygpath_bin" ] &&
        grep -qF 'failed to load manifest' "$build_log" &&
        grep -qE 'The system cannot find the path specified|os error 3|No such file or directory' "$build_log"; then
        printf '%s\n' \
            'check-crate-examples: the generated manifest records a path that the native cargo cannot open.' \
            'check-crate-examples: this host has cygpath, so the manifest must hold the native spelling that' \
            'check-crate-examples: `cygpath -w` returns. Git Bash translates argv for a native program, never' \
            'check-crate-examples: the contents of a file it reads, so a POSIX path here is opened as' \
            'check-crate-examples: C:\d\a\... and does not exist.' \
            "check-crate-examples: the manifest is $manifest_file and recorded:" >&2
        grep 'path = ' "$manifest_file" | sed 's/^/  /' >&2
        printf 'check-crate-examples: check the native spelling with: cygpath -w -- %s\n' "$repo_root" >&2
    fi
    printf 'check-crate-examples: the crate-page examples do not compile; a syntax/API change is a build failure\n' >&2
    printf 'DOCREPORT gate=check-crate-examples status=fail pages=%d fences=%d compiled=%d excluded=%d failures=1\n' \
        "${#pages[@]}" "$fences" "$compiled" "$excluded"
    exit 1
fi

# ---------------------------------------------------------------------------
# Run every binary. A panic is reported through the same line map.
# ---------------------------------------------------------------------------
failures=0
while IFS=$'\t' read -r bin page start; do
    exe=$target_dir/debug/$bin
    if [ ! -x "$exe" ] && [ -x "$exe.exe" ]; then
        exe=$exe.exe
    fi
    if [ ! -x "$exe" ]; then
        printf 'FAIL %s:%s: cargo did not produce an executable for %s\n' "$page" "$start" "$bin" >&2
        failures=$((failures + 1))
        continue
    fi
    output=$(RUST_BACKTRACE=0 "$exe" 2>&1)
    code=$?
    if [ "$code" -ne 0 ]; then
        mapped=$(printf '%s\n' "$output" | map_diagnostics)
        printf '%s\n' "$mapped" >&2
        fail_line=$start
        case "$mapped" in
            *"$page:"*)
                remainder=${mapped#*"$page:"}
                candidate=${remainder%%[!0-9]*}
                [ -n "$candidate" ] && fail_line=$candidate
                ;;
        esac
        printf 'FAIL %s:%s: fence did not run to completion (exit %d)\n' "$page" "$fail_line" "$code" >&2
        failures=$((failures + 1))
    else
        printf 'PASS %s:%s\n' "$page" "$start"
    fi
done < "$run_list"

docreport_status=ok
if [ "$failures" -ne 0 ]; then
    printf 'check-crate-examples: %d of %d included fence(s) failed\n' "$failures" "$compiled" >&2
    docreport_status=fail
else
    if [ "$excluded" -eq 0 ]; then
        printf 'EXCLUDED (none)\n'
    fi
    printf 'check-crate-examples: %d page(s), %d rust fence(s): %d compiled and ran, %d excluded\n' \
        "${#pages[@]}" "$fences" "$compiled" "$excluded"
    printf 'check-crate-examples: OK\n'
fi

# The machine-readable evidence line (RQ-023), additive and last on stdout.
printf 'DOCREPORT gate=check-crate-examples status=%s pages=%d fences=%d compiled=%d excluded=%d failures=%d\n' \
    "$docreport_status" "${#pages[@]}" "$fences" "$compiled" "$excluded" "$failures"

if [ "$docreport_status" = fail ]; then
    exit 1
fi
exit 0
