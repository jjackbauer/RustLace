#!/usr/bin/env python3
"""Mutation self-test for the architecture checker (RNL-CI-003/004/010).

Each scenario copies `tools/architecture-fixtures/base` to an isolated `.tmp/arch-gates/<name>`
tree, injects one forbidden change, runs the real checker and requires the expected nonzero exit
and diagnostic. The valid fixtures must pass. A scenario that stops failing fails this suite, so
the architecture gate cannot quietly become a no-op.

Usage:
    python tools/test-architecture-gates.py
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASE = ROOT / "tools" / "architecture-fixtures" / "base"
CHECK = ROOT / "tools" / "check-architecture.py"
WORK = ROOT / ".tmp" / "arch-gates"


class Scenario:
    def __init__(self, name: str) -> None:
        self.name = name
        self.directory = WORK / name
        if self.directory.exists():
            shutil.rmtree(self.directory)
        shutil.copytree(BASE, self.directory)

    def write(self, relative: str, text: str) -> None:
        path = self.directory / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8", newline="\n")

    def append(self, relative: str, text: str) -> None:
        path = self.directory / relative
        existing = path.read_text(encoding="utf-8") if path.exists() else ""
        path.write_text(existing + text, encoding="utf-8", newline="\n")

    def metadata(self) -> dict:
        return json.loads((self.directory / "metadata.json").read_text(encoding="utf-8"))

    def write_metadata(self, data: dict) -> None:
        (self.directory / "metadata.json").write_text(
            json.dumps(data, indent=2) + "\n", encoding="utf-8", newline="\n"
        )

    def policy(self) -> dict:
        return json.loads(
            (self.directory / "tools" / "architecture-policy.json").read_text(encoding="utf-8")
        )

    def write_policy(self, policy: dict) -> None:
        (self.directory / "tools" / "architecture-policy.json").write_text(
            json.dumps(policy, indent=2) + "\n", encoding="utf-8", newline="\n"
        )

    def run(self) -> tuple[int, str]:
        completed = subprocess.run(
            [
                sys.executable,
                str(CHECK),
                "--root",
                str(self.directory),
                "--metadata",
                str(self.directory / "metadata.json"),
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        return completed.returncode, completed.stdout + completed.stderr


def dependency_edit(scenario: Scenario, crate: str, dependency: str) -> None:
    data = scenario.metadata()
    for package in data["packages"]:
        if package["name"] == crate:
            package["dependencies"].append({"name": dependency, "kind": None})
    scenario.write_metadata(data)


def main() -> int:
    if WORK.exists():
        shutil.rmtree(WORK)
    WORK.mkdir(parents=True)
    failures: list[str] = []
    executed = 0

    def expect(
        name: str,
        expected_code: int,
        needle: str | None,
        mutate,
    ) -> None:
        nonlocal executed
        executed += 1
        scenario = Scenario(name)
        mutate(scenario)
        code, output = scenario.run()
        okay = code == expected_code and (needle is None or needle in output)
        if okay:
            print(f"PASS {name}: exit {code}" + (f", {needle}" if needle else ""))
        else:
            failures.append(name)
            print(f"FAIL {name}: expected exit {expected_code} and {needle!r}, got {code}")
            print(output)
            print("---")

    def valid(scenario: Scenario) -> None:
        # The base fixture already carries a domain formula, a typed unit, a canonical alias and
        # legal dependency edges; this asserts the legitimate case is accepted.
        pass

    def duplicate_rational(scenario: Scenario) -> None:
        scenario.write(
            "lovelace-systems/src/numeric.rs",
            "use lovelace_int::Int;\nuse lovelace_nat::Nat;\n\n"
            "pub struct Rat { pub num: Int, pub den: Nat }\n\n"
            "impl Rat {\n"
            "    pub fn add(&self, other: &Rat) -> Rat { other.clone() }\n"
            "    pub fn sub(&self, other: &Rat) -> Rat { other.clone() }\n"
            "    pub fn mul(&self, other: &Rat) -> Rat { other.clone() }\n"
            "    pub fn div(&self, other: &Rat) -> Rat { other.clone() }\n"
            "}\n",
        )

    def duplicate_trait(scenario: Scenario) -> None:
        scenario.append(
            "lovelace-systems/src/lib.rs",
            "\npub struct Cost(pub Q);\n\n"
            "impl std::ops::Add for Cost {\n"
            "    type Output = Cost;\n"
            "    fn add(self, other: Self) -> Self { Cost(self.0.add(&other.0)) }\n"
            "}\n",
        )

    def undeclared_numeric(scenario: Scenario) -> None:
        scenario.append(
            "lovelace-systems/src/lib.rs",
            "\nuse lovelace_int::Int;\nuse lovelace_nat::Nat;\n\n"
            "pub struct ScalarPair { pub top: Int, pub bottom: Nat }\n\n"
            "impl ScalarPair {\n"
            "    pub fn add(&self, other: &ScalarPair) -> ScalarPair { other.clone() }\n"
            "    pub fn sub(&self, other: &ScalarPair) -> ScalarPair { other.clone() }\n"
            "    pub fn mul(&self, other: &ScalarPair) -> ScalarPair { other.clone() }\n"
            "    pub fn div(&self, other: &ScalarPair) -> ScalarPair { other.clone() }\n"
            "}\n",
        )

    def forbidden_dependency(scenario: Scenario) -> None:
        dependency_edit(scenario, "lovelace-systems", "lovelace-lang")

    def cycle(scenario: Scenario) -> None:
        dependency_edit(scenario, "lovelace-systems", "lovelace-lang")

    def unauthorized_exception(scenario: Scenario) -> None:
        policy = scenario.policy()
        policy["exceptions"] = [
            {
                "id": "EX-1",
                "rule": "forbidden-dependency",
                "scope": ["lovelace-systems/*"],
                "rationale": "we need this one",
            }
        ]
        scenario.write_policy(policy)

    def invalid_policy(scenario: Scenario) -> None:
        scenario.write("tools/architecture-policy.json", "{ not json\n")

    def empty_workspace(scenario: Scenario) -> None:
        scenario.write_metadata({"packages": [], "workspace_members": []})

    def zero_scan(scenario: Scenario) -> None:
        for path in scenario.directory.rglob("*.rs"):
            path.unlink()

    def non_delegating_specialization(scenario: Scenario) -> None:
        text = (scenario.directory / "lovelace-systems" / "src" / "lib.rs").read_text(
            encoding="utf-8"
        )
        text = text.replace(
            "ServiceTime(self.0.add(&other.0))", "ServiceTime(self.0.clone())"
        )
        scenario.write("lovelace-systems/src/lib.rs", text)

    expect("valid", 0, "violations=0", valid)
    expect("duplicate-rational", 1, "duplicate-numeric-type", duplicate_rational)
    expect("duplicate-trait", 1, "duplicate-arithmetic-trait", duplicate_trait)
    expect("undeclared-numeric", 1, "duplicate-numeric-representation", undeclared_numeric)
    expect("forbidden-dependency", 1, "forbidden-dependency", forbidden_dependency)
    expect("dependency-cycle", 1, "dependency-cycle", cycle)
    expect("unauthorized-exception", 1, "unauthorized-exception", unauthorized_exception)
    expect("invalid-policy", 2, "CONFIG ERROR", invalid_policy)
    expect("empty-workspace", 2, "CONFIG ERROR", empty_workspace)
    expect("zero-scan", 2, "CONFIG ERROR", zero_scan)
    expect(
        "non-delegating-specialization",
        1,
        "specialization-not-delegating",
        non_delegating_specialization,
    )

    if executed < 11:
        print(f"FAIL: only {executed} mutation scenarios executed", file=sys.stderr)
        return 1
    if failures:
        print(f"ARCH GATES FAILED: {len(failures)} scenario(s): {', '.join(failures)}", file=sys.stderr)
        return 1
    print(f"ARCH GATES PASSED: {executed} scenarios")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
