#!/usr/bin/env python3
"""Mutate the agent contract gate to prove it fails on stale instructions."""
from __future__ import annotations

import importlib.util
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
GATE = ROOT / "tools/harness/check-agent-contracts.py"
FILES = ("lovelace-lang/src/registry.rs", "docs/skills.md", "AGENTS.md",
         ".github/workflows/ci.yml")


def seed(destination: Path) -> None:
    for item in FILES:
        target = destination / item
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / item, target)
    shutil.copytree(ROOT / ".dsh/skills", destination / ".dsh/skills")


def run_gate(root: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(GATE), "--root", str(root), "--static-only"],
        capture_output=True, text=True, timeout=30, check=False,
    )


def change(root: Path, file: str, before: str, after: str) -> None:
    path = root / file
    source = path.read_text(encoding="utf-8")
    if before not in source:
        raise AssertionError(f"mutation anchor missing: {file}: {before}")
    path.write_text(source.replace(before, after, 1), encoding="utf-8")


def main() -> int:
    spec = importlib.util.spec_from_file_location("agent_contracts", GATE)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    assert module.exact_rank([[1, 2], [2, 4]]) == 1
    assert module.exact_rank([[1, 2, 3], [2, 4, 6], [3, 6, 9]]) == 1
    assert module.exact_rank([[1, 0], [0, 1]]) == 2
    mutants = (
        ("retired-field", ".dsh/skills/add-builtin/SKILL.md",
         "`module`, `category`", "`plugin`, `category`"),
        ("wrong-inventory-count", "docs/skills.md",
         "Eight task skills", "Nine task skills"),
        ("missing-skill", "docs/skills.md",
         "../.dsh/skills/add-builtin/SKILL.md",
         "../.dsh/skills/nonexistent/SKILL.md"),
        ("missing-module-metadata", "lovelace-lang/src/registry.rs",
         'module: "Core"', 'module: "Missing"'),
        ("missing-verify", ".dsh/skills/add-builtin/SKILL.md",
         "## Verify", "## NotVerified"),
        ("agent-index-drift", "AGENTS.md",
         "(.dsh/skills/add-builtin/SKILL.md)",
         "(.dsh/skills/does-not-exist/SKILL.md)"),
        ("retired-plugin-directive", ".dsh/skills/add-builtin/SKILL.md",
         'Use `module: "Core"`', 'Use `plugin: None`'),
        ("frontmatter-drift", ".dsh/skills/add-builtin/SKILL.md",
         "name: add-builtin", "name: add-builtin-obsolete"),
        ("missing-when-to-use", ".dsh/skills/add-builtin/SKILL.md",
         "whenToUse:", "obsoleteUseField:"),
        ("missing-registry-kind", "lovelace-lang/src/registry.rs",
         'return_kind: Some("Natural | Integer | Real | Symbolic | Vector"),',
         'return_kind: None,'),
        ("missing-registry-struct-field", "lovelace-lang/src/registry.rs",
         "pub module: &'static str,", "pub plugin: &'static str,"),
    )
    with tempfile.TemporaryDirectory(prefix="rustlace-agent-contracts-") as temp:
        base = Path(temp)
        baseline = base / "baseline"
        seed(baseline)
        baseline_result = run_gate(baseline)
        if baseline_result.returncode != 0:
            print(f"baseline FAIL: {baseline_result.stderr}", file=sys.stderr)
            return 1
        for label, file, before, after in mutants:
            target = base / label
            seed(target)
            change(target, file, before, after)
            result = run_gate(target)
            if result.returncode == 0 or "AGENTCONTRACT FAIL" not in result.stderr:
                print(f"mutation did not fail: {label}: {result.stdout} {result.stderr}",
                      file=sys.stderr)
                return 1
    print(f"AGENTCONTRACTMUTATION total={len(mutants)} passed={len(mutants)} failed=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
