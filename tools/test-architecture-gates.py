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

    rat_representation = (
        "\nuse lovelace_int::Int;\nuse lovelace_nat::Nat;\n\n"
        "pub struct Rat { pub num: Int, pub den: Nat }\n\n"
        "impl Rat {\n"
        "    pub fn add(&self, other: &Rat) -> Rat { other.clone() }\n"
        "    pub fn sub(&self, other: &Rat) -> Rat { other.clone() }\n"
        "    pub fn mul(&self, other: &Rat) -> Rat { other.clone() }\n"
        "    pub fn div(&self, other: &Rat) -> Rat { other.clone() }\n"
        "}\n"
    )

    def duplicate_in_owner_crate(scenario: Scenario) -> None:
        # lovelace-lang owns numeric promotion, not the Rat representation. A whole-crate numeric
        # owner exemption let this pass; abstraction-specific ownership must reject it.
        scenario.append("lovelace-lang/src/lib.rs", rat_representation)

    def duplicate_rat_in_owner_crate(scenario: Scenario) -> None:
        # lovelace-real owns the Real abstraction, not the Rat representation.
        scenario.append("lovelace-real/src/lib.rs", rat_representation)

    def duplicate_real_in_owner_crate(scenario: Scenario) -> None:
        # Even the owning crate may declare a reserved name once: a second Real declaration is a
        # duplicate representation of the same abstraction.
        scenario.append(
            "lovelace-real/src/lib.rs",
            "\npub struct Real { pub value: u64 }\n",
        )

    def split_impl(scenario: Scenario) -> None:
        # Four arithmetic methods on a renamed representation, two per impl block. The per-block
        # maximum is 2; the summed count is 4 and must reach the arithmetic-impl rule.
        scenario.append(
            "lovelace-systems/src/lib.rs",
            "\nuse lovelace_int::Int;\nuse lovelace_nat::Nat;\n\n"
            "pub struct SplitPair { pub num: Int, pub den: Nat }\n\n"
            "impl SplitPair {\n"
            "    pub fn add(&self, other: &SplitPair) -> SplitPair { other.clone() }\n"
            "    pub fn sub(&self, other: &SplitPair) -> SplitPair { other.clone() }\n"
            "}\n\n"
            "impl SplitPair {\n"
            "    pub fn mul(&self, other: &SplitPair) -> SplitPair { other.clone() }\n"
            "    pub fn div(&self, other: &SplitPair) -> SplitPair { other.clone() }\n"
            "}\n",
        )

    def generic_arithmetic_trait(scenario: Scenario) -> None:
        # `impl<T> std::ops::Add for Wrapper<T>` and `... for &Wrapper<T>`: the parser must strip
        # the leading generics and resolve both targets to Wrapper, not to T.
        scenario.append(
            "lovelace-systems/src/lib.rs",
            "\npub struct Wrapper<T>(pub T);\n\n"
            "impl<T> std::ops::Add for Wrapper<T> {\n"
            "    type Output = Wrapper<T>;\n"
            "    fn add(self, other: Self) -> Self { other }\n"
            "}\n\n"
            "impl<T> std::ops::Add for &Wrapper<T> {\n"
            "    type Output = Wrapper<T>;\n"
            "    fn add(self, other: Self) -> Self { other }\n"
            "}\n",
        )

    def free_function_kernel(scenario: Scenario) -> None:
        # A competing rational kernel expressed as free functions instead of inherent methods.
        scenario.append(
            "lovelace-systems/src/lib.rs",
            "\npub fn rat_add(left: u64, right: u64) -> u64 { left + right }\n"
            "pub fn fraction_mul(left: u64, right: u64) -> u64 { left * right }\n"
            "pub fn rational_div(left: u64, right: u64) -> u64 { left / right }\n"
            "pub fn q_gcd(left: u64, right: u64) -> u64 { left }\n",
        )

    def renamed_representation(scenario: Scenario) -> None:
        # Quantity is not a reserved name, left/right is not a numeric field pair, and each impl
        # block holds two arithmetic methods: only the summed method count can see this shape.
        scenario.append(
            "lovelace-systems/src/lib.rs",
            "\nuse lovelace_int::Int;\nuse lovelace_nat::Nat;\n\n"
            "pub struct Quantity { pub left: Int, pub right: Nat }\n\n"
            "impl Quantity {\n"
            "    pub fn add(&self, other: &Quantity) -> Quantity { other.clone() }\n"
            "    pub fn sub(&self, other: &Quantity) -> Quantity { other.clone() }\n"
            "}\n\n"
            "impl Quantity {\n"
            "    pub fn mul(&self, other: &Quantity) -> Quantity { other.clone() }\n"
            "    pub fn div(&self, other: &Quantity) -> Quantity { other.clone() }\n"
            "}\n",
        )

    def partial_duplicate_kernel(scenario: Scenario) -> None:
        # The second-review Finding 2 shape: `n`/`d` are not a recognized numeric field pair and
        # only two arithmetic methods are implemented, so neither the named-pair rule nor the
        # four-method threshold may be what rejects it. Numeric storage plus two arithmetic
        # methods is itself the structural signal.
        scenario.append(
            "lovelace-systems/src/lib.rs",
            "\nuse lovelace_int::Int;\nuse lovelace_nat::Nat;\n\n"
            "pub struct RatioValue { pub n: Int, pub d: Nat }\n\n"
            "impl RatioValue {\n"
            "    pub fn add(&self, other: &RatioValue) -> RatioValue { other.clone() }\n"
            "    pub fn div(&self, other: &RatioValue) -> RatioValue { other.clone() }\n"
            "}\n",
        )

    def broad_exception_scope(scenario: Scenario) -> None:
        # A complete, otherwise-valid exception whose scope is `lovelace-*`. The old checker only
        # rejected the literal "*", so this waived a real violation and the gate exited 0.
        scenario.write(
            "docs/adr/999-exception-scope-probe.md",
            "# 999 - exception scope probe\n\n"
            "Fixture-only decision record used to prove that a broad architecture-exception scope\n"
            "is rejected even when every required field is present.\n",
        )
        scenario.append("lovelace-systems/src/lib.rs", "\npub struct Rat;\n")
        policy = scenario.policy()
        policy["exceptions"] = [
            {
                "id": "EX-BROAD-1",
                "rule": "duplicate-numeric-type",
                "adr": "docs/adr/999-exception-scope-probe.md",
                "rationale": "Temporary probe of exception scope matching in the review fixture.",
                "reviewer": "review-rnl-001",
                "removalPlan": "Remove this exception and the probe type after review.",
                "scope": ["lovelace-*"],
            }
        ]
        scenario.write_policy(policy)

    def unresolved_arithmetic_impl(scenario: Scenario) -> None:
        # The arithmetic trait name is present but the impl target is the generic parameter T, so
        # the analysis cannot decide whose representation this is. That must be review-required,
        # never a silent pass.
        scenario.append(
            "lovelace-systems/src/lib.rs",
            "\npub struct Placeholder<T>(pub T);\n\n"
            "impl<T> Add for T {\n"
            "    type Output = T;\n"
            "    fn add(self, other: Self) -> Self { other }\n"
            "}\n",
        )

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
    expect(
        "duplicate-in-owner-crate",
        1,
        "duplicate-numeric-type",
        duplicate_in_owner_crate,
    )
    expect(
        "duplicate-rat-in-owner-crate",
        1,
        "duplicate-numeric-type",
        duplicate_rat_in_owner_crate,
    )
    expect(
        "duplicate-real-in-owner-crate",
        1,
        "duplicate-numeric-type",
        duplicate_real_in_owner_crate,
    )
    expect("split-impl", 1, "duplicate-arithmetic-impl", split_impl)
    expect(
        "generic-arithmetic-trait",
        1,
        "duplicate-arithmetic-trait",
        generic_arithmetic_trait,
    )
    expect("free-function-kernel", 1, "free-function-kernel", free_function_kernel)
    expect(
        "renamed-representation",
        1,
        "duplicate-numeric-representation",
        renamed_representation,
    )
    expect(
        "partial-duplicate-kernel",
        1,
        "duplicate-numeric-representation",
        partial_duplicate_kernel,
    )
    expect(
        "broad-exception-scope", 1, "unauthorized-exception", broad_exception_scope
    )
    expect(
        "unresolved-arithmetic-impl",
        1,
        "review-required",
        unresolved_arithmetic_impl,
    )

    if executed < 21:
        print(f"FAIL: only {executed} mutation scenarios executed", file=sys.stderr)
        return 1
    if failures:
        print(f"ARCH GATES FAILED: {len(failures)} scenario(s): {', '.join(failures)}", file=sys.stderr)
        return 1
    print(f"ARCH GATES PASSED: {executed} scenarios")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
