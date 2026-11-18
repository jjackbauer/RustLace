#!/usr/bin/env bash
# Include the documentation checker mutation suite in the aggregate evidence.
# The mutation fixtures stub the aggregate command, avoiding recursive execution.
set -uo pipefail
root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd) || exit 2
cd -- "$root" || exit 2
if bash tools/test-docs-gates.sh; then
    printf 'DOCREPORT gate=doc-gates-mutants status=ok checks=1 failed=0\n'
else
    printf 'DOCREPORT gate=doc-gates-mutants status=fail checks=1 failed=1\n'
    exit 1
fi
