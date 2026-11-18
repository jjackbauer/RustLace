#!/usr/bin/env bash
# Keep both language and extension fenced runners in the evidence transcript.
set -uo pipefail
root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd) || exit 2
cd -- "$root" || exit 2
ok=0
if bash docs/language/examples/run-examples.sh; then
    ok=$((ok + 1))
else
    printf 'doc-family-examples: language examples failed\n' >&2
fi
if bash docs/extensions/examples/run-examples.sh; then
    ok=$((ok + 1))
else
    printf 'doc-family-examples: extension examples failed\n' >&2
fi
if [[ $ok -eq 2 ]]; then
    printf 'DOCREPORT gate=doc-family-examples status=ok families=2 failed=0\n'
else
    printf 'DOCREPORT gate=doc-family-examples status=fail families=2 failed=%d\n' "$((2-ok))"
    exit 1
fi
