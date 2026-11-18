#!/usr/bin/env bash
# Aggregate the existing independent agent contract and mutation gates.
set -uo pipefail
root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd) || exit 2
cd -- "$root" || exit 2
ok=0
if python tools/harness/check-agent-contracts.py; then
    ok=$((ok + 1))
else
    printf 'agent-contracts: live/static contract check failed\n' >&2
fi
if python tools/harness/test-agent-contracts.py; then
    ok=$((ok + 1))
else
    printf 'agent-contracts: mutation check failed\n' >&2
fi
if python tools/harness/contract-coverage.py --summary; then
    ok=$((ok + 1))
else
    printf 'agent-contracts: coverage inventory failed\n' >&2
fi
if [[ $ok -eq 3 ]]; then
    printf 'DOCREPORT gate=agent-contracts status=ok checks=3 failed=0\n'
else
    printf 'DOCREPORT gate=agent-contracts status=fail checks=3 failed=%d\n' "$((3-ok))"
    exit 1
fi
