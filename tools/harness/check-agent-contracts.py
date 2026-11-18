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
                "ten": 10, "eleven": 11, "twelve": 12, "fifteen": 15,
                "seventeen": 17}
CASES = (
    ("abs", "abs(-3)", "Integer", "3"),
    ("abs", 'abs(symbol("x"))', "Symbolic", None),
    ("abs", "abs(fft([0, 1, 0, 0]))", "Vector", None),
    ("mean", "mean([2, 4])", "Natural", "3"),
    ("mean", "mean([[[1,2],[3,4]],[[5,6],[7,8]]], 0)", "Array", None),
    ("norm", "norm([[3,4],[0,5]], 0)", "Vector", None),
    ("norm", "norm([[[3,4],[0,5]],[[0,3],[4,0]]], 0)", "Array", None),
    ("sum", "sum([[1, 2], [3, 4]], 0)", "Vector", None),
    ("matmul", "matmul([[1, 2], [3, 4]], [[5, 6], [7, 8]])",
     "Array", None),
)
REFUSALS = (
    ('matrix_rank([[symbol("x"),1],[evalf(sqrt(2),10),1]])', "Real"),
    ('matrix_rank([[evalf(sqrt(2),10),1],[symbol("x"),1]])', "Real"),
    ('matrix_rank([[symbol("x"),1],["bad",1]])', "Text"),
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
    if not registry:
        fail("registry scan returned no descriptors")
    allowed_modules = {"Core", "DSP", "Symbolics", "MathIR", "Systems"}
    for name, entry in registry.items():
        if entry.get("module") not in allowed_modules:
            fail(f"{name}: unknown static module {entry.get('module')!r}")
        if entry.get("module") == "Core" and not entry.get("return_kind"):
            fail(f"{name}: Core descriptor lacks a nonempty return_kind")
    if not any(entry["module"] == "Core" for entry in registry.values()):
        fail("registry scan found no Core descriptors")
    # The documented catalogue is a second inventory: a builtin cannot be
    # removed from code (or added without docs) and keep passing static-only.
    catalogue = (root / "docs/language/builtins.md").read_text(encoding="utf-8")
    documented = re.findall(r"(?m)^\\| \\x60([a-z][a-z0-9_]*)\\x60 \\|", catalogue)
    if len(documented) != len(set(documented)):
        fail("builtin catalogue contains duplicate function rows")
    if set(documented) != set(registry):
        fail(f"builtin catalogue mismatch: docs-only={sorted(set(documented)-set(registry))}, "
             f"registry-only={sorted(set(registry)-set(documented))}")
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
    workflow = (root / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    if "\\n  docs:\\n" not in workflow or "\\n  architecture-policy:" not in workflow:
        fail("cannot identify the documentation CI job")
    doc_job = workflow.split("\\n  docs:\\n", 1)[1].split("\\n  architecture-policy:", 1)[0]
    ci_steps = len(re.findall(r"(?m)^      - name: ", doc_job))
    advertised = re.search(r"(?m)^\\| Documentation examples .*?\\| (\\w+) steps", agents)
    if not ci_steps or not advertised or NUMBER_WORDS.get(advertised.group(1).lower()) != ci_steps:
        fail(f"AGENTS.md documentation CI step count drift: actual={ci_steps}")
    for command in ("tools/harness/check-agent-contracts.py",
                    "tools/harness/test-agent-contracts.py"):
        if command not in doc_job or command not in agents:
            fail(f"agent instructions or documentation CI omit required gate {command}")
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


def invoke(runner: Path, arguments: list[str], expected_exit: int) -> dict:
    try:
        proc = subprocess.run([str(runner), *arguments], text=True,
                              capture_output=True, timeout=30, check=False)
    except (OSError, subprocess.TimeoutExpired) as err:
        fail(f"runner unavailable: {err}")
    if proc.returncode != expected_exit:
        fail(f"runner {arguments[:3]}: exit {proc.returncode}, expected {expected_exit}; "
             f"stderr={proc.stderr[:300]}, stdout={proc.stdout[:300]}")
    try:
        return json.loads(proc.stdout)
    except (ValueError, TypeError) as err:
        fail(f"invalid runner envelope: {err}")


def live_registry(runner: Path, static: dict[str, dict[str, str]]) -> dict[str, dict]:
    # --functions is the compiled authority for names; per-function queries
    # provide full return kinds (the listing intentionally omits prose).
    listing = invoke(runner, ["--functions", "--function-limit",
                              str(max(len(static) + 1, 1000))], 0)
    entries = listing.get("functions")
    if not isinstance(entries, list) or listing.get("truncated") is not False:
        fail("compiled function listing missing or truncated")
    if listing.get("total") != len(entries) or listing.get("returned") != len(entries):
        fail("compiled registry listing counts are inconsistent")
    names = [item["name"] for item in entries]
    if len(names) != len(set(names)) or set(names) != set(static):
        fail(f"source/compiled registry mismatch: source-only={sorted(set(static)-set(names))}, "
             f"compiled-only={sorted(set(names)-set(static))}")
    selected = {name for name, *_ in CASES} | {"matrix_rank"}
    contracts: dict[str, dict] = {}
    for name in sorted(selected):
        descriptor = invoke(runner, ["--function", name], 0).get("function")
        if not isinstance(descriptor, dict) or descriptor.get("name") != name:
            fail(f"compiled descriptor missing: {name}")
        if descriptor.get("module") != static[name].get("module"):
            fail(f"{name}: compiled module mismatches static registry")
        if descriptor.get("returnKind") != static[name].get("return_kind"):
            fail(f"{name}: compiled returnKind mismatches static registry")
        contracts[name] = descriptor
    # These operations have fixed public output kinds; over-advertising an
    # unreachable kind is also contract drift, not merely a harmless superset.
    for name, allowed in (("matmul", {"Array"}), ("matrix_rank", {"Integer"})):
        kinds = {value.strip() for value in contracts[name]["returnKind"].split("|")}
        if kinds != allowed:
            fail(f"{name}: returnKind over-advertises unreachable kinds: {kinds - allowed}")
    return contracts


def run_refusal(runner: Path, script: str, arrived_kind: str) -> None:
    envelope = invoke(runner, ["--eval", script, "--omit-functions",
                               "--omit-variables"], 1)
    if (envelope.get("code"), envelope.get("category")) != (
        "InvalidArgument", "TypeMismatch"
    ):
        fail(f"matrix_rank refusal misclassified for {script}")
    signature = envelope.get("signature")
    if (not isinstance(signature, dict)
        or signature.get("function") != "matrix_rank"
        or signature.get("position") != 1
        or signature.get("parameter") != "a"
        or signature.get("arrivedKind") != arrived_kind):
        fail(f"matrix_rank refusal missing/mismatched signature for {script}")
    if "exact numeric or symbolic entries" not in envelope.get("message", ""):
        fail(f"matrix_rank refusal message drift for {script}")


def run_case(runner: Path, name: str, source: str, kind: str,
             expected: str | None, registry: dict[str, dict[str, str]]) -> None:
    descriptor = registry.get(name)
    if descriptor is None:
        fail(f"missing registry descriptor {name}")
    allowed = set(map(str.strip, descriptor.get("returnKind", "").split("|")))
    if kind not in allowed:
        fail(f"{name} registry returnKind {allowed} excludes valid {kind}")
    envelope = invoke(runner, ["--eval", source, "--omit-functions", "--omit-variables"], 0)
    result = envelope.get("result")
    if not isinstance(result, dict):
        fail(f"{name}: missing runner result")
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
            contracts = live_registry(runner, registry)
            for name, source, kind, expected in CASES:
                run_case(runner, name, source, kind, expected, contracts)
                checked += 1
            for matrix in RANK_MATRICES:
                literal = "[" + ", ".join(str(row).replace(" ", "") for row in matrix) + "]"
                run_case(runner, "matrix_rank", f"matrix_rank({literal})",
                         "Integer", str(exact_rank(matrix)), contracts)
                checked += 1
            for script, arrived_kind in REFUSALS:
                run_refusal(runner, script, arrived_kind)
                checked += 1
        core_count = sum(1 for descriptor in registry.values() if descriptor["module"] == "Core")
        print(f"AGENTCONTRACT skills={skills} core={core_count} executed={checked} failed=0")
        return 0
    except (ValueError, OSError) as err:
        print(f"AGENTCONTRACT FAIL: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
