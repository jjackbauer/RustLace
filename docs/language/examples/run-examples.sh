#!/usr/bin/env bash
# Runs every ```lovelace block in the docs/language pages through
# `lovelace-run --file <tmp> --text` and compares the normalised result with
# the ```text block that follows it.
#
# Pairing contract: a ```lovelace block must be immediately followed (blank
# lines allowed) by its ```text block, and a ```text block immediately
# following another fence must be the output of a ```lovelace block. Either
# half of a pair going missing is a failure, not a skipped example.
#
# It then verifies the builtin catalogue in docs/language/builtins.md against
# the live runtime (`lovelace-run --functions --function-limit 500`): every
# documented name, its declared category, arity and parameter names, and the
# presence of every runtime builtin on the page. A documented builtin the
# runtime does not publish, a row under the wrong category, and a runtime
# builtin the page omits all fail the run with a docs/language/builtins.md
# line diagnostic. The check is pure bash, so it runs unchanged on Linux and
# on Windows (Git Bash); run-examples.ps1 stays in the tree for a Windows
# PowerShell host, but this script is the one CI executes on both.
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
# Exit status: 0 when every example matches, 1 on an example mismatch, a
# missing or unlisted page, a malformed or unpaired fence pair, or when no
# example pairs were found at all.
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

# ---------------------------------------------------------------------------
# Helpers for the catalogue check. The registry listing is compact JSON with a
# fixed shape (`name`, `category`, `minArity`, `parameterCount`, `variadic`,
# `parameters`), so it is read with parameter expansion rather than a JSON
# dependency: the runner must work with the tools Git Bash and ubuntu-latest
# both carry, and jq is not one of them.
# ---------------------------------------------------------------------------

trim() {
    local text=$1
    text=${text#"${text%%[![:space:]]*}"}
    text=${text%"${text##*[![:space:]]}"}
    printf '%s' "$text"
}

json_string() {
    local record=$1 key=$2 rest
    [[ $record == *"\"$key\":\""* ]] || return 1
    rest=${record#*"\"$key\":\""}
    printf '%s' "${rest%%\"*}"
}

json_number() {
    local record=$1 key=$2 rest
    [[ $record == *"\"$key\":"* ]] || return 1
    rest=${record#*"\"$key\":"}
    rest=${rest%%[!0-9]*}
    [[ -n $rest ]] || return 1
    printf '%s' "$rest"
}

json_boolean() {
    local record=$1 key=$2
    if [[ $record == *"\"$key\":true"* ]]; then
        printf 'true'
    elif [[ $record == *"\"$key\":false"* ]]; then
        printf 'false'
    else
        return 1
    fi
}

# The registry's arity, rendered the way the page renders it: "1" for an exact
# count, "0-1" for an optional parameter, "2+" for a variadic minimum.
arity_text() {
    local minimum=$1 declared=$2 variadic=$3
    if [[ $variadic == true ]]; then
        printf '%s+' "$minimum"
    elif [[ $minimum == "$declared" ]]; then
        printf '%s' "$minimum"
    else
        printf '%s-%s' "$minimum" "$declared"
    fi
}

# The declared parameter names, rendered the way the page renders them.
parameters_text() {
    local record=$1 inside names
    [[ $record == *'"parameters":['* ]] || return 1
    inside=${record#*'"parameters":['}
    inside=${inside%%]*}
    if [[ -z $inside ]]; then
        printf '&mdash;'
        return 0
    fi
    names=${inside//\"/}
    printf '%s' "${names//,/, }"
}

# The page manifest, validated in both directions against the directory:
# every page listed here must exist, and every docs/language/*.md file must be
# listed here. A new page fails the run until it is added to the manifest
# instead of being silently skipped by a stale array; a removed page fails
# instead of shrinking the checked set. No page is excluded: a page with no
# examples is allowed, as long as the run as a whole finds some.
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

page_listed() {
    local wanted=$1 candidate
    for candidate in "${pages[@]}"; do
        [[ $candidate == "$wanted" ]] && return 0
    done
    return 1
}

for page in "${pages[@]}"; do
    if [[ ! -f $docs_dir/$page || ! -r $docs_dir/$page ]]; then
        die "declared page is missing or unreadable: docs/language/$page"
    fi
done

shopt -s nullglob
inventory=("$docs_dir"/*.md)
shopt -u nullglob
if (( ${#inventory[@]} == 0 )); then
    die "no Markdown pages found under docs/language"
fi
for page_path in "${inventory[@]}"; do
    page_name=${page_path##*/}
    if ! page_listed "$page_name"; then
        die "docs/language/$page_name is not in the page manifest; add it to 'pages' (or exclude it with a stated reason) so it cannot be silently skipped"
    fi
done

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

    # Split the page into fenced blocks first: pairing is a property of the
    # block sequence, not of a cursor that consumes a block as it pairs it.
    block_lang=()
    block_content=()
    block_open=()
    block_close=()
    in_fence=0
    lang=
    buffer=
    sep=
    open_line=0
    for (( n = 0; n < line_count; n++ )); do
        line=${page_lines[n]}
        if (( !in_fence )); then
            if [[ $line =~ ^${FENCE}[[:space:]]*([^[:space:]]*)[[:space:]]*$ ]]; then
                in_fence=1
                lang=$(printf '%s' "${BASH_REMATCH[1]}" | tr '[:upper:]' '[:lower:]')
                buffer=
                sep=
                open_line=$((n + 1))
            fi
        else
            if [[ $line =~ ^${FENCE}[[:space:]]*$ ]]; then
                in_fence=0
                block_lang+=("$lang")
                block_content+=("$buffer")
                block_open+=("$open_line")
                block_close+=("$((n + 1))")
            else
                buffer+=$sep$line
                sep=$'\n'
            fi
        fi
    done
    if (( in_fence )); then
        die "docs/language/$page:$open_line: unterminated '$lang' fence"
    fi

    # True when only blank lines separate block $1's closing fence from block
    # $2's opening fence. An example is a pair of adjacent blocks; prose
    # between a script and its output is not a documented output.
    blocks_adjacent() {
        local left=$1 right=$2 n
        for (( n = block_close[left]; n < block_open[right] - 1; n++ )); do
            [[ ${page_lines[n]} =~ ^[[:space:]]*$ ]] || return 1
        done
        return 0
    }

    block_count=${#block_lang[@]}
    passed_here=0
    for (( idx = 0; idx < block_count; idx++ )); do
        # A text block is only an expected output when it directly follows a
        # lovelace block. One that directly follows another fence of a
        # different language has lost its script and must not be skipped.
        if [[ ${block_lang[idx]} == text ]]; then
            if (( idx > 0 )) && blocks_adjacent $((idx - 1)) "$idx" &&
                [[ ${block_lang[idx - 1]} != lovelace ]]; then
                die "docs/language/$page:${block_open[idx]}: text block is not the output of a preceding lovelace block (preceded by '${block_lang[idx - 1]}')"
            fi
            continue
        fi
        if [[ ${block_lang[idx]} != lovelace ]]; then
            continue
        fi

        fence_line=${block_open[idx]}
        if (( idx + 1 >= block_count )); then
            die "docs/language/$page:$fence_line: lovelace block has no following text block"
        fi
        if [[ ${block_lang[idx + 1]} != text ]]; then
            die "docs/language/$page:$fence_line: expected a \`\`\`text block, found '${block_lang[idx + 1]}'"
        fi
        if ! blocks_adjacent "$idx" $((idx + 1)); then
            die "docs/language/$page:$fence_line: lovelace block is not immediately followed by its \`\`\`text block"
        fi

        source=${block_content[idx]}
        expected=${block_content[idx + 1]}

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
    done

    printf '%s: %d example(s) passed\n' "$page" "$passed_here"
done

if (( total == 0 )); then
    die 'no ```lovelace/```text example pairs found under docs/language; refusing to pass on zero work'
fi

# ---------------------------------------------------------------------------
# The catalogue check (RQ-014).
#
# The examples prove the prose and the snippets; they do not prove the tables
# in builtins.md. This check reads the live registry through
# `--functions --function-limit 500` and compares the JSON with the page:
#   - every runtime builtin must be documented, under the category the registry
#     publishes, with the arity and parameter names the registry declares;
#   - every documented row must be published by the runtime, so a renamed or
#     deleted builtin is caught as a documented name the registry does not know;
#   - the "Registry at a glance" totals and each "Full catalogue" heading count
#     must agree with the runtime counts.
# A mismatch is a documentation defect, not a skipped check: the run exits 1
# with a docs/language/builtins.md:<line> diagnostic naming the row.
# ---------------------------------------------------------------------------

catalogue_path=$docs_dir/builtins.md
if [[ ! -f $catalogue_path || ! -r $catalogue_path ]]; then
    die "missing or unreadable catalogue page: docs/language/builtins.md"
fi

catalogue_lines=()
while IFS= read -r line || [[ -n $line ]]; do
    catalogue_lines+=("${line%$'\r'}")
done <"$catalogue_path"

problems=()
add_problem() {
    problems+=("$1")
}

# The runtime registry. A non-zero exit, a missing functions array or a
# malformed record is fatal: an unread listing must never read as a pass.
listing_file=$scratch/functions.json
if ! "$exe" --functions --function-limit 500 >"$listing_file" 2>"$scratch/functions.err"; then
    printf 'error: lovelace-run --functions --function-limit 500 failed:\n' >&2
    sed 's/^/  /' "$scratch/functions.err" >&2
    exit 1
fi
listing=$(cat "$listing_file")
if [[ $listing != *'"ok":true'* ]]; then
    die 'malformed --functions JSON: no "ok":true field'
fi
runtime_total=$(json_number "$listing" total) || die 'malformed --functions JSON: no "total" field'
runtime_returned=$(json_number "$listing" returned) || die 'malformed --functions JSON: no "returned" field'
runtime_truncated=$(json_boolean "$listing" truncated) || die 'malformed --functions JSON: no "truncated" field'
record_text=${listing#*'"functions":['}
if [[ $record_text == "$listing" ]]; then
    die 'malformed --functions JSON: no "functions" array'
fi
case $record_text in
    *'],"mathIrVersion":'*) record_text=${record_text%%'],"mathIrVersion":'*} ;;
    *) die 'malformed --functions JSON: unterminated "functions" array' ;;
esac
if [[ -z $record_text ]]; then
    die 'lovelace-run --functions published no builtins; refusing to pass on zero work'
fi
record_text=${record_text//'},{'/$'\n'}

# Every associative array is initialised with `=()`. Under `set -u` a merely
# declared associative array reports "unbound variable" for `${#a[@]}`, which
# would turn the zero-work guard below into an error message instead of the
# diagnostic it owes the reader.
declare -A runtime_category=()
declare -A runtime_arity=()
declare -A runtime_parameters=()
declare -A runtime_count=()
registry_count=0
category_count=0
while IFS= read -r record; do
    [[ -n $record ]] || continue
    name=$(json_string "$record" name) || die "malformed --functions record without a name: $record"
    category=$(json_string "$record" category) ||
        die "malformed --functions record for '$name': no category"
    minimum=$(json_number "$record" minArity) ||
        die "malformed --functions record for '$name': no minArity"
    declared=$(json_number "$record" parameterCount) ||
        die "malformed --functions record for '$name': no parameterCount"
    variadic=$(json_boolean "$record" variadic) ||
        die "malformed --functions record for '$name': no variadic flag"
    parameters=$(parameters_text "$record") ||
        die "malformed --functions record for '$name': no parameters array"
    if [[ -n ${runtime_category[$name]:-} ]]; then
        die "lovelace-run --functions published '$name' twice"
    fi
    runtime_category[$name]=$category
    runtime_arity[$name]=$(arity_text "$minimum" "$declared" "$variadic")
    runtime_parameters[$name]=$parameters
    runtime_count[$category]=$(( ${runtime_count[$category]:-0} + 1 ))
    registry_count=$((registry_count + 1))
done <<<"$record_text"
category_count=${#runtime_count[@]}

if (( runtime_returned != registry_count )); then
    add_problem "lovelace-run --functions: \"returned\" says $runtime_returned but the listing carried $registry_count records"
fi
if (( runtime_total != registry_count )); then
    add_problem "lovelace-run --functions: \"total\" says $runtime_total but the listing carried $registry_count records"
fi
if [[ $runtime_truncated != false ]]; then
    add_problem "lovelace-run --functions: --function-limit 500 truncated the listing at $registry_count of $runtime_total builtins; raise the limit in this runner"
fi

# The page: the "Registry at a glance" counts and the "Full catalogue" rows.
# A level-1..3 heading closes the current region; a level-4 heading inside the
# full catalogue opens a category section.
declare -A doc_category=()
declare -A doc_arity=()
declare -A doc_parameters=()
declare -A doc_line=()
declare -A section_declared=()
declare -A section_line=()
declare -A section_rows=()
declare -A glance_count=()
declare -A glance_line=()
glance_heading_line=0
glance_total=
glance_total_line=0
full_heading_line=0
mode=none
section=
# The patterns are held in variables: bash's conditional lexer mishandles an
# escaped space inside an inline `=~` pattern, and a quoted pattern would turn
# the metacharacters literal. A variable is a plain ERE and parses cleanly.
catalogue_total_re='^\| \*\*Total\*\* \| \*\*([0-9]+)\*\*'
catalogue_glance_re='^\| ([A-Za-z][^|]*) \| ([0-9]+) \|'
catalogue_section_re='^#### ([^(]+) \(([0-9]+) builtins?\)[[:space:]]*$'
catalogue_row_re='^\| `([^`]+)` \| ([^|]*) \| ([^|]*) \|[[:space:]]*$'
catalogue_line_count=${#catalogue_lines[@]}
for (( n = 0; n < catalogue_line_count; n++ )); do
    line=${catalogue_lines[n]}
    lineno=$((n + 1))
    case $line in
        '### Registry at a glance'*)
            mode=glance
            glance_heading_line=$lineno
            continue
            ;;
        '### Full catalogue'*)
            mode=full
            full_heading_line=$lineno
            section=
            continue
            ;;
    esac
    if [[ $line =~ ^#{1,3}[[:space:]] ]]; then
        mode=none
        continue
    fi
    case $mode in
        glance)
            if [[ $line =~ $catalogue_total_re ]]; then
                glance_total=${BASH_REMATCH[1]}
                glance_total_line=$lineno
                continue
            fi
            if [[ $line =~ $catalogue_glance_re ]]; then
                key=$(trim "${BASH_REMATCH[1]}")
                glance_count[$key]=${BASH_REMATCH[2]}
                glance_line[$key]=$lineno
            fi
            ;;
        full)
            if [[ $line =~ $catalogue_section_re ]]; then
                section=$(trim "${BASH_REMATCH[1]}")
                section_declared[$section]=${BASH_REMATCH[2]}
                section_line[$section]=$lineno
                section_rows[$section]=0
                continue
            fi
            if [[ $line =~ $catalogue_row_re ]]; then
                name=$(trim "${BASH_REMATCH[1]}")
                row_arity=$(trim "${BASH_REMATCH[2]}")
                row_parameters=$(trim "${BASH_REMATCH[3]}")
                if [[ -z $section ]]; then
                    add_problem "docs/language/builtins.md:$lineno: catalogue row '$name' sits under no section heading"
                    continue
                fi
                if [[ -n ${doc_line[$name]:-} ]]; then
                    add_problem "docs/language/builtins.md:$lineno: catalogue row '$name' appears twice (first at line ${doc_line[$name]})"
                    continue
                fi
                doc_category[$name]=$section
                doc_arity[$name]=$row_arity
                doc_parameters[$name]=$row_parameters
                doc_line[$name]=$lineno
                section_rows[$section]=$(( ${section_rows[$section]:-0} + 1 ))
            fi
            ;;
    esac
done

if (( ${#doc_line[@]} == 0 )); then
    die 'the builtin catalogue lists no Name/Arity/Parameters rows under "### Full catalogue"; refusing to pass on zero work'
fi

# Every runtime builtin, in a stable order so a failing run reads the same way
# twice: documented, in the right category, with the declared arity/parameters.
while IFS= read -r name; do
    category=${runtime_category[$name]}
    if [[ -z ${doc_line[$name]:-} ]]; then
        line=${section_line[$category]:-${full_heading_line:-1}}
        add_problem "docs/language/builtins.md:$line: the full catalogue omits runtime builtin '$name' (category '$category')"
        continue
    fi
    if [[ ${doc_category[$name]} != "$category" ]]; then
        add_problem "docs/language/builtins.md:${doc_line[$name]}: '$name' is listed under '${doc_category[$name]}'; the runtime registry says '$category'"
    fi
    if [[ ${doc_arity[$name]} != "${runtime_arity[$name]}" ]]; then
        add_problem "docs/language/builtins.md:${doc_line[$name]}: '$name' lists arity '${doc_arity[$name]}'; the registry declares '${runtime_arity[$name]}'"
    fi
    if [[ ${doc_parameters[$name]} != "${runtime_parameters[$name]}" ]]; then
        add_problem "docs/language/builtins.md:${doc_line[$name]}: '$name' lists parameters '${doc_parameters[$name]}'; the registry declares '${runtime_parameters[$name]}'"
    fi
done < <(printf '%s\n' "${!runtime_category[@]}" | LC_ALL=C sort)

# Every documented row must still exist at runtime: this is the check that
# catches a builtin renamed or removed in the code while the page kept the old
# name.
while IFS= read -r name; do
    if [[ -z ${runtime_category[$name]:-} ]]; then
        add_problem "docs/language/builtins.md:${doc_line[$name]}: documented builtin '$name' is not published by lovelace-run --functions (runtime total $runtime_total)"
    fi
done < <(printf '%s\n' "${!doc_line[@]}" | LC_ALL=C sort)

# The glance table, in both directions.
if [[ -z $glance_total ]]; then
    add_problem "docs/language/builtins.md:${glance_heading_line:-1}: the \"Registry at a glance\" table has no **Total** row"
else
    if (( glance_total != runtime_total )); then
        add_problem "docs/language/builtins.md:${glance_total_line}: the glance Total says $glance_total but --functions reports total $runtime_total"
    fi
fi
glance_sum=0
glance_keys=$(printf '%s\n' "${!glance_count[@]}" | LC_ALL=C sort)
while IFS= read -r key; do
    [[ -n $key ]] || continue
    glance_sum=$((glance_sum + glance_count[$key]))
    if [[ -z ${runtime_count[$key]:-} ]]; then
        add_problem "docs/language/builtins.md:${glance_line[$key]}: the glance table names category '$key', which the registry does not publish"
    elif (( glance_count[$key] != runtime_count[$key] )); then
        add_problem "docs/language/builtins.md:${glance_line[$key]}: the glance table says '$key' has ${glance_count[$key]}; the registry has ${runtime_count[$key]}"
    fi
done <<<"$glance_keys"
if (( glance_sum != runtime_total )); then
    add_problem "docs/language/builtins.md:${glance_heading_line:-1}: the glance categories sum to $glance_sum but --functions reports total $runtime_total"
fi
while IFS= read -r category; do
    if [[ -z ${glance_count[$category]:-} ]]; then
        add_problem "docs/language/builtins.md:${glance_heading_line:-1}: the glance table omits category '$category' (${runtime_count[$category]} builtins)"
    fi
done < <(printf '%s\n' "${!runtime_count[@]}" | LC_ALL=C sort)

# The full-catalogue section headings and their row counts.
while IFS= read -r category; do
    if [[ -z ${section_declared[$category]:-} ]]; then
        add_problem "docs/language/builtins.md:${full_heading_line:-1}: the full catalogue has no '$category' section"
        continue
    fi
    if (( section_declared[$category] != runtime_count[$category] )); then
        add_problem "docs/language/builtins.md:${section_line[$category]}: the '$category' heading says ${section_declared[$category]} builtins; the registry has ${runtime_count[$category]}"
    fi
    if (( ${section_rows[$category]:-0} != section_declared[$category] )); then
        add_problem "docs/language/builtins.md:${section_line[$category]}: the '$category' section declares ${section_declared[$category]} rows but lists ${section_rows[$category]:-0}"
    fi
done < <(printf '%s\n' "${!runtime_count[@]}" | LC_ALL=C sort)
while IFS= read -r category; do
    [[ -n $category ]] || continue
    if [[ -z ${runtime_count[$category]:-} ]]; then
        add_problem "docs/language/builtins.md:${section_line[$category]}: the full catalogue names category '$category', which the registry does not publish"
    fi
done < <(printf '%s\n' "${!section_declared[@]}" | LC_ALL=C sort)

if (( ${#problems[@]} > 0 )); then
    printf '\ncatalogue: %d mismatch(es) between docs/language/builtins.md and lovelace-run --functions --function-limit 500\n' \
        "${#problems[@]}" >&2
    for problem in "${problems[@]}"; do
        printf '  %s\n' "$problem" >&2
    done
    exit 1
fi
printf 'catalogue: %d builtins in %d categories match lovelace-run --functions --function-limit 500\n' \
    "$runtime_total" "$category_count"

if (( failed > 0 )); then
    printf '\n%d of %d example(s) failed\n' "$failed" "$total"
    exit 1
fi

printf '\nAll %d examples passed.\n' "$total"
exit 0
