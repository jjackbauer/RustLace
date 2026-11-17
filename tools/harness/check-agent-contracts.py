#!/usr/bin/env python3
"""Fail-closed agent instruction and core builtin contract checks.

Static checks need only Python's standard library. With --runner, execute
public Lovelace calls against the actual binary and independently verify
matrix rank using Fraction elimination (never the Rust implementation).
"""
from __future__ import annotations

import argparse
from fractions import Fraction
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
NUMBER_WORDS = {"zero": 0, "one": 1, "two": 2, "three": 3, "four": 4,
                "five": 5, "six": 6, "seven": 7, "eight": 8, "nine": 9,
                "ten": 10, "eleven": 11, "twelve": 12}
CASES = (
    ("abs", "abs(-3)", "Integer", "3"),
    ("abs", 'abs(symbol("x"))', "Symbolic", None),
    ("abs", "abs(fft([0, 1, 0, 0]))", "Vector", None),
    ("mean", "mean([2, 4])", "Natural", "3"),
    ("sum", "sum([[1, 2], [3, 4]], 0)", "Vector", None),
    ("matmul", "matmul([[1, 2], [3, 4]], [[5, 6], [7, 8]])",
     "Array", None),
)
RANK_MATRICES = (
    [[0, 0], [0, 0]],
    [[1, 2], [2, 4]],
    [[1, 2, 3], [2, 4, 6], [3, 6, 9]],
    [[1, 2, 3], [2, 4, 6]],
    [[1, 0, 0], [0, 1, 0]],
    [[1, 2], [3, 4]],
    [[0, 1], [0, 0]],
    [[-2, 4, 6], [1, -2, -3]],
)


def fail(message: str) -> None:
    raise ValueError(message)


def load_registry(root: Path) -> tuple[set[str], dict[str, dict[str, str]]]:
    source = (root / "lovelace-lang/src/registry.rs").read_text(encoding="utf-8")
    match = re.search(r"pub struct BuiltinInfo\s*\{(.*?)\n\}", source, re.S)
    if not match:
        fail("missing BuiltinInfo declaration")
    fields = set(re.findall(r"pub\s+(\w+)\s*:", match.group(1)))
    for required in ("module", "name", "return_kind", "min_arity", "parameter_count"):
        if required not in fields:
            fail(f"BuiltinInfo is missing required field {required}")
    if "plugin" in fields:
        fail("BuiltinInfo reintroduced the retired plugin descriptor")
    registry: dict[str, dict[str, str]] = {}
    for raw in re.findall(r"\n    BuiltinInfo\s*\{(.*?)\n    \},", source, re.S):
        entry = {}
        for field in ("name", "module", "return_kind"):
            found = re.search(rf"\b{field}:\s*(?:Some\()?\"([^\"]+)\"", raw)
            if found:
                entry[field] = found.group(1)
        if entry.get("name"):
            if entry["name"] in registry:
                fail(f"duplicate builtin name: {entry['name']}")
            registry[entry["name"]] = entry
    if len(registry) < 44:
        fail(f"registry scan incomplete: only {len(registry)} descriptors")
    core = [name for name, entry in registry.items() if entry.get("module") == "Core"]
    if len(core) != 44:
        fail(f"expected 44 Core descriptors, got {len(core)}")
    return fields, registry


def check_skills(root: Path, fields: set[str]) -> int:
    skill_root = root / ".dsh/skills"
    paths = sorted(skill_root.glob("*/SKILL.md"))
    if not paths:
        fail("no agent skill files scanned")
    index = (root / "docs/skills.md").read_text(encoding="utf-8")
    agents = (root / "AGENTS.md").read_text(encoding="utf-8")
    index_names = set(re.findall(r"\.\./\.dsh/skills/([\w-]+)/SKILL\.md", index))
    agent_names = set(re.findall(r"\(\.dsh/skills/([\w-]+)/SKILL\.md\)", agents))
    actual = {path.parent.name for path in paths}
    if index_names != actual:
        fail(f"docs/skills.md inventory drift: missing={actual - index_names}, extra={index_names - actual}")
    if agent_names != actual:
        fail(f"AGENTS.md skill links drift: missing={actual - agent_names}, extra={agent_names - actual}")
    count = re.search(r"^(\w+) task skills\b", index, re.I | re.M)
    if not count or NUMBER_WORDS.get(count.group(1).lower()) != len(actual):
        fail(f"docs/skills.md stated skill count is not {len(actual)}")
    for path in paths:
        text = path.read_text(encoding="utf-8")
        if not re.search(rf"(?m)^name:\s*{re.escape(path.parent.name)}\s*$", text):
            fail(f"{path}: frontmatter name does not match directory")
        if not re.search(r"(?m)^whenToUse:\s*\S", text):
            fail(f"{path}: missing whenToUse")
        if "## Verify" not in text:
            fail(f"{path}: missing Verify section")
    builtin = (skill_root / "add-builtin/SKILL.md").read_text(encoding="utf-8")
    if re.search(r"\bplugin\s*:\s*(?:None|Some\()", builtin):
        fail("add-builtin skill instructs agents to use retired plugin descriptor")
    field_list = re.search(r"Note the fields:\s*(.*?)\.", builtin, re.S)
    if not field_list:
        fail("add-builtin skill does not declare registry field expectations")
    named = re.findall(r"`([a-z][a-z_]+)`", field_list.group(1))
    missing = set(named) - fields
    if missing:
        fail(f"add-builtin references missing BuiltinInfo fields: {sorted(missing)}")
    return len(paths)


def exact_rank(matrix: list[list[int]]) -> int:
    """Reference rank over rational numbers, independent of the Rust evaluator."""
    data = [[Fraction(value) for value in row] for row in matrix]
    rows = len(data)
    columns = len(data[0])
    if any(len(row) != columns for row in data):
        fail("malformed oracle matrix")
    rank = 0
    for column in range(columns):
        pivot = next((r for r in range(rank, rows) if data[r][column]), None)
        if pivot is None:
            continue
        data[rank], data[pivot] = data[pivot], data[rank]
        divisor = data[rank][column]
        for r in range(rank + 1, rows):
            coefficient = data[r][column] / divisor
            for c in range(column, columns):
                data[r][c] -= coefficient * data[rank][c]
        rank += 1
        if rank == rows:
            break
    return rank


def run_case(runner: Path, name: str, source: str, kind: str,
             expected: str | None, registry: dict[str, dict[str, str]]) -> None:
    descriptor = registry.get(name)
    if descriptor is None:
        fail(f"missing registry descriptor {name}")
    allowed = set(map(str.strip, descriptor.get("return_kind", "").split("|")))
    if kind not in allowed:
        fail(f"{name} registry returnKind {allowed} excludes valid {kind}")
    command = [str(runner), "--eval", source, "--omit-functions", "--omit-variables"]
    try:
        process = subprocess.run(command, text=True, capture_output=True, timeout=30, check=False)
    except (OSError, subprocess.TimeoutExpired) as err:
        fail(f"{name}: runner unavailable: {err}")
    if process.returncode != 0:
        fail(f"{name}: exit={process.returncode}, stderr={process.stderr}, stdout={process.stdout[:300]}")
    try:
        envelope = json.loads(process.stdout)
        result = envelope["result"]
    except (ValueError, KeyError, TypeError) as err:
        fail(f"{name}: invalid runner envelope: {err}")
    if result.get("kind") != kind:
        fail(f"{name}: expected kind {kind}, got {result.get('kind')}")
    if expected is not None and result.get("display") != expected:
        fail(f"{name}: expected {expected}, got {result.get('display')}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--runner", type=Path, help="built lovelace-run; required unless --static-only")
    parser.add_argument("--static-only", action="store_true")
    options = parser.parse_args()
    try:
        fields, registry = load_registry(options.root)
        skills = check_skills(options.root, fields)
        checked = 0
        if not options.static_only:
            runner = options.runner
            if runner is None:
                runner = options.root / "target/debug/lovelace-run"
                if not runner.is_file():
                    runner = options.root / "target/debug/lovelace-run.exe"
            runner = runner.resolve()
            if not runner.is_file():
                fail(f"runner missing: {runner}")
            for name, source, kind, expected in CASES:
                run_case(runner, name, source, kind, expected, registry)
                checked += 1
            for matrix in RANK_MATRICES:
                literal = "[" + ", ".join(str(row).replace(" ", "") for row in matrix) + "]"
                run_case(runner, "matrix_rank", f"matrix_rank({literal})",
                         "Integer", str(exact_rank(matrix)), registry)
                checked += 1
        print(f"AGENTCONTRACT skills={skills} core=44 executed={checked} failed=0")
        return 0
    except (ValueError, OSError) as err:
        print(f"AGENTCONTRACT FAIL: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
