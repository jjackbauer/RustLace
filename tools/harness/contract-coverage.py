#!/usr/bin/env python3
"""Expose the *tested surface* of the agent-contract gate, not an assurance score.

This reads current public registry descriptors and the actual CASES/REFUSALS in
check-agent-contracts.py. It does not execute a function or certify correctness.
Use the live contract gate for the selected executions.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "tools/harness/check-agent-contracts.py"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--summary", action="store_true")
    opts = parser.parse_args()
    try:
        spec = importlib.util.spec_from_file_location("agent_contract_gate", CHECKER)
        if not spec or not spec.loader:
            raise ValueError("cannot load agent contract checker")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        _, registry = module.load_registry(opts.root)
        selected = {name for name, *_ in module.CASES} | {"matrix_rank"}
        if not selected or not selected <= set(registry):
            raise ValueError(f"selected tests lack registry descriptors: {sorted(selected - set(registry))}")
        matrix_rank_cases = len(module.RANK_MATRICES)
        refusal_cases = len(module.REFUSALS)
        if matrix_rank_cases == 0 or refusal_cases == 0:
            raise ValueError("rank oracle/refusal fixtures unexpectedly empty")
        functions = [
            {
                "name": name,
                "module": descriptor["module"],
                "selectedForRuntimeCheck": name in selected,
                "hasIndependentMathOracle": name == "matrix_rank",
            }
            for name, descriptor in sorted(registry.items())
        ]
        coverage = {
            "status": "fixture-scope-only-not-execution-certificate",
            "registered": len(registry),
            "selectedForRuntimeCheck": len(selected),
            "notSelectedForRuntimeCheck": len(registry) - len(selected),
            "withIndependentMathOracle": 1,
            "selectedCallCases": len(module.CASES) + matrix_rank_cases + refusal_cases,
            "functions": functions,
        }
        if opts.summary:
            print("CONTRACTCOVERAGE " + " ".join(
                f"{key}={coverage[key]}" for key in
                ("registered", "selectedForRuntimeCheck", "notSelectedForRuntimeCheck",
                 "withIndependentMathOracle", "selectedCallCases")
            ))
        else:
            print(json.dumps(coverage, indent=2, sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError) as exc:
        print(f"CONTRACTCOVERAGE FAIL: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
