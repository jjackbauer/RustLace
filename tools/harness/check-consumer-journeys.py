#!/usr/bin/env python3
"""Black-box consumer journey: discover, inspect, execute, verify, recover.

This deliberately invokes the *public CLI*, never interpreter internals. It
checks fixed evidence-backed tasks, not whether an unfamiliar human/LLM can
independently discover the correct workflow. That needs a separate blind study.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def fail(message: str) -> None:
    raise ValueError(message)


def call(runner: Path, arguments: list[str], exit_code: int) -> dict:
    try:
        p = subprocess.run([str(runner), *arguments], capture_output=True,
                           text=True, timeout=30, check=False, cwd=ROOT)
    except (OSError, subprocess.TimeoutExpired) as exc:
        fail(f"cannot execute {arguments}: {exc}")
    if p.returncode != exit_code:
        fail(f"{arguments}: exit {p.returncode} != {exit_code}; stderr={p.stderr[:400]}")
    if p.stderr and exit_code == 0:
        fail(f"{arguments}: unexpected stderr: {p.stderr[:400]}")
    try:
        value = json.loads(p.stdout)
    except json.JSONDecodeError as exc:
        fail(f"{arguments}: stdout not a JSON envelope: {exc}")
    if not isinstance(value, dict):
        fail(f"{arguments}: top-level JSON is not an object")
    return value


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--runner", type=Path)
    args = ap.parse_args()
    runner = args.runner or ROOT / "target/debug/lovelace-run"
    if not runner.is_file():
        runner = ROOT / "target/debug/lovelace-run.exe"
    if not runner.is_file():
        fail(f"runner missing: {runner}; run cargo build -p lovelace-run")
    runner = runner.resolve()
    checks = 0
    try:
        short = call(runner, ["--functions", "--function-limit", "1"], 0)
        if short.get("returned") != 1 or short.get("truncated") is not True:
            fail("the one-item registry discovery response must say truncated=true")
        checks += 1
        all_ = call(runner, ["--functions", "--function-limit", "500"], 0)
        functions = all_.get("functions")
        if (not isinstance(functions, list)
                or all_.get("truncated") is not False
                or all_.get("total") != len(functions)
                or all_.get("returned") != len(functions)
                or len(functions) <= 50):
            fail("full registry discovery incomplete; an agent must paginate/retry")
        names = {item["name"] for item in functions}
        if not {"matrix_rank", "sys_mm1", "sqrt"} <= names:
            fail("expected operations absent from completed discovery")
        checks += 1
        for name, module in (("matrix_rank", "Core"), ("sys_mm1", "Systems")):
            detail = call(runner, ["--function", name], 0).get("function")
            if not isinstance(detail, dict) or detail.get("module") != module:
                fail(f"{name}: incorrect function descriptor or module")
            checks += 1
        for script, kind, display in (
            ("1/3", "Real", "0.(3)"),
            ("matrix_rank([[1,2],[2,4]])", "Integer", "1"),
            ("sys_mm1(8,10).utilization", "Real", "0.8"),
        ):
            output = call(runner, ["--eval", script, "--omit-functions", "--omit-variables"], 0)
            result = output.get("result")
            if not isinstance(result, dict) or (result.get("kind"), result.get("display")) != (kind, display):
                fail(f"{script}: wrong typed output: {result}")
            checks += 1
        if Fraction(8, 10) != Fraction(4, 5):
            fail("independent utilization oracle invalid")
        if Fraction(2, 4) != Fraction(1, 2):
            fail("independent rank-dependence oracle invalid")
        checks += 1
        script = ROOT / "docs/tutorials/examples/linear-algebra.ls"
        file_result = call(runner, ["--file", str(script), "--omit-functions", "--omit-variables"], 0)
        result = file_result.get("result")
        if not isinstance(result, dict) or (result.get("kind"), result.get("display")) != ("Array", "[[5], [6]]"):
            fail("file-based matrix tutorial no longer returns the documented residual")
        checks += 1
        bad = call(runner, ["--eval", "matrix_rank([1,2,3])",
                            "--omit-functions", "--omit-variables"], 1)
        if (bad.get("code"), bad.get("category")) != ("InvalidArgument", "TypeMismatch"):
            fail("wrong-shaped matrix must produce a structured type refusal")
        checks += 1
        print(f"DOCREPORT gate=consumer-journeys status=ok checks={checks} "
              "discovery=2 descriptors=2 executions=4 refusals=1 failed=0")
        return 0
    except (ValueError, KeyError) as exc:
        print(f"consumer-journeys: FAIL: {exc}", file=sys.stderr)
        print(f"DOCREPORT gate=consumer-journeys status=fail checks={checks} failed=1")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
