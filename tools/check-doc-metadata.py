#!/usr/bin/env python3
"""Check documentation inventories against source metadata, not copied counts.

Pure standard-library check. It deliberately inspects only *current assertions*:
dated observations in docs/testing.md and historical release notes are not current
contracts. Use --self-test to mutation-test the assertions and the checker itself.
"""
from __future__ import annotations

import argparse
from datetime import date
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
WORDS = {
    "zero": 0, "one": 1, "two": 2, "three": 3, "four": 4,
    "five": 5, "six": 6, "seven": 7, "eight": 8, "nine": 9,
    "ten": 10, "eleven": 11, "twelve": 12, "thirteen": 13,
    "fourteen": 14, "fifteen": 15, "sixteen": 16, "seventeen": 17,
    "eighteen": 18, "nineteen": 19, "twenty": 20,
}


class MetadataFailure(ValueError):
    pass


def fail(path: str, message: str) -> None:
    raise MetadataFailure(f"{path}: {message}")


def read(root: Path, path: str) -> str:
    try:
        return (root / path).read_text(encoding="utf-8")
    except OSError as exc:
        fail(path, str(exc))


def stated_count(path: str, content: str, pattern: str) -> int:
    match = re.search(pattern, content, re.I | re.M | re.S)
    if not match:
        fail(path, "missing canonical inventory statement")
    raw = match.group(1).lower()
    if raw.isdigit():
        return int(raw)
    if raw not in WORDS:
        fail(path, f"unrecognized count word {raw!r}")
    return WORDS[raw]


def scan_not_run_annotations(path: str, source: str) -> list[tuple[str, str]]:
    """Match the executable-example runner's line-oriented waiver grammar.

    The shell runner recognizes a line beginning with optional whitespace,
    '<!--', optional whitespace and 'not-run:'. It takes everything up to the
    first '-->' as the reason and tolerates trailing text after the close.
    A fenced code example showing the annotation is not an actual waiver.
    Return (reason, exact Lovelace script) for adjacent executable fences.
    """
    lines = source.replace("\r\n", "\n").split("\n")
    annotations = []
    in_fence = False
    for index, line in enumerate(lines):
        if in_fence:
            if re.fullmatch(r"\s*```\s*", line):
                in_fence = False
            continue
        opening = re.fullmatch(r"\s*```\s*([^\s]*)\s*", line)
        if opening:
            in_fence = True
            continue
        if not re.match(r"^\s*<!--\s*not-run:", line):
            continue
        # Mirror shell parameter expansion: reason=${line#*not-run:};
        # reason=${reason%%-->*}; trim whitespace. No [^<>] shortcut.
        reason = line.split("not-run:", 1)[1].split("-->", 1)[0].strip()
        if not reason:
            fail(path, f"empty not-run annotation at line {index + 1}")
        if index + 1 >= len(lines) or not re.fullmatch(
            r"\s*```\s*lovelace\s*", lines[index + 1], re.I
        ):
            fail(path, f"not-run annotation is not immediately above a Lovelace fence at line {index + 1}")
        closing = next(
            (end for end in range(index + 2, len(lines))
             if re.fullmatch(r"\s*```\s*", lines[end])),
            None,
        )
        if closing is None:
            fail(path, f"unclosed annotated Lovelace fence at line {index + 2}")
        annotations.append((reason, "\n".join(lines[index + 2:closing])))
    return annotations


def validate(root: Path) -> dict[str, int]:
    manifest = tomllib.loads(read(root, "Cargo.toml"))
    members = manifest["workspace"]["members"]
    if not members or len(members) != len(set(members)):
        fail("Cargo.toml", "empty or duplicate workspace members")
    member_set = set(members)
    readme = read(root, "README.md")
    build = read(root, "docs/build-and-run.md")
    agents = read(root, "AGENTS.md")
    skills_index = read(root, "docs/skills.md")

    member_count = stated_count(
        "README.md", readme,
        r"^\w+ crates:\s+(\w+) workspace members plus one standalone spike"
    )
    total_count = stated_count(
        "README.md", readme,
        r"^(\w+) crates:\s+\w+ workspace members plus one standalone spike"
    )
    if member_count != len(members) or total_count != len(members) + 1:
        fail("README.md", f"crate counts {total_count}/{member_count} vs Cargo members={len(members)}")
    build_count = stated_count(
        "docs/build-and-run.md", build, r"The workspace has (\w+) members"
    )
    if build_count != len(members):
        fail("docs/build-and-run.md", f"workspace count {build_count} vs {len(members)}")

    crate_section = re.search(r"(?ms)^## The crates\n(.*?)(?=^## |\Z)", readme)
    if not crate_section:
        fail("README.md", "missing crate table")
    listed = re.findall(r"(?m)^\| \x60(lovelace-[a-z-]+)\x60 \|", crate_section.group(1))
    expected = member_set | {"lovelace-desktop-spike"}
    if len(listed) != len(set(listed)) or set(listed) != expected:
        fail("README.md", f"crate table drift: missing={sorted(expected-set(listed))}, extra={sorted(set(listed)-expected)}")

    skill_paths = list((root / ".dsh/skills").glob("*/SKILL.md"))
    skills = len(skill_paths)
    if skills == 0:
        fail(".dsh/skills", "no skill files")
    if stated_count("README.md", readme, r"the (\w+) task skills for extending") != skills:
        fail("README.md", "task-skill count drift")
    if stated_count("docs/skills.md", skills_index, r"^(\w+) task skills") != skills:
        fail("docs/skills.md", "task-skill count drift")

    adrs = len(list((root / "docs/adr").glob("[0-9][0-9][0-9]-*.md")))
    if not adrs:
        fail("docs/adr", "no ADRs found")
    if stated_count("README.md", readme, r"(\w+) architecture decision records") != adrs:
        fail("README.md", f"ADR count drift (actual={adrs})")

    corpus_counts = {}
    for family in ("lang", "symbolics", "systems"):
        path = f"parity/corpus/{family}.json"
        try:
            doc = json.loads(read(root, path))
        except ValueError as exc:
            fail(path, f"invalid corpus JSON: {exc}")
        records = doc.get("examples")
        if not isinstance(records, list) or not records:
            fail(path, "examples array missing or empty")
        corpus_counts[family] = len(records)
        declared = re.search(
            rf"\x60parity/corpus/{family}\.json\x60 \((\d+)", readme
        )
        if not declared or int(declared.group(1)) != len(records):
            fail("README.md", f"{family} corpus count drift (actual={len(records)})")
    total = sum(corpus_counts.values())
    # The normative workspace contract and parity reference must not keep
    # historic family sizes as if they described current fixtures.
    for scoped in ("CONTRACT.md", "docs/crates/lovelace-corpus.md",
                   "docs/parity-with-studio.md"):
        scoped_text = read(root, scoped)
        for family, count in corpus_counts.items():
            marker = f"`parity/corpus/{family}.json`"
            if marker not in scoped_text:
                fail(scoped, f"missing {family} corpus from current inventory")
            # A reference page can mention the corpus earlier in a table
            # without making an inventory assertion. Inspect *all* explicit
            # count declarations instead of blindly taking the first mention.
            # This also catches a stale declaration appended alongside a
            # correct one, without treating an uncounted link as a claim.
            declarations = [
                int(value)
                for value in re.findall(
                    rf"{re.escape(marker)}[^\n]*?\b(\d+)\s+records\b",
                    scoped_text,
                )
            ]
            if not declarations or any(value != count for value in declarations):
                fail(scoped, f"{family} inventory drift "
                    f"(declared={declarations}, actual={count})")
    corpus_page = read(root, "docs/crates/lovelace-corpus.md")
    for path, text, pattern in (
        ("README.md", readme, rf"\bfor {total} records\b"),
        ("AGENTS.md", agents, rf"\b{total}-record documentation\b"),
        ("docs/crates/lovelace-corpus.md", corpus_page, rf"\b{total} records in total\b"),
    ):
        if not re.search(pattern, text):
            fail(path, f"missing current corpus total {total}")
    # Catch stale totals even if a new correct total was appended elsewhere.
    for path, text in (("README.md", readme), ("docs/crates/lovelace-corpus.md", corpus_page)):
        wrong = re.findall(r"\bcorpus:\s*(\d+) passed,\s*0 failed,\s*(\d+) total", text)
        if any(int(all_) != total or int(passed) > total for passed, all_ in wrong):
            fail(path, f"stale corpus transcript (current total={total})")
        # A filtered corpus invocation may correctly report fewer passes than
        # the total inventory. README's first-run transcript is unfiltered.
        if path == "README.md" and any(int(passed) != total for passed, _ in wrong):
            fail(path, f"unfiltered quick start must execute all {total} records")
    for path, text in (("README.md", readme), ("AGENTS.md", agents)):
        if re.search(r"\bthe one ignored test\b", text):
            fail(path, "historical ignored-test assertion presented as current")

    version_page = "docs/version-applicability.md"
    if not (root / version_page).is_file():
        fail(version_page, "missing version compatibility instructions")
    if "docs/agent-usage.md" not in readme or "docs/agent-usage.md" not in agents:
        fail("README.md / AGENTS.md", "missing consumer-agent entry-point link")
    tutorial_index = read(root, "docs/tutorials/README.md")
    if "checked-out development revision" not in tutorial_index:
        fail("docs/tutorials/README.md", "must explain development-branch applicability")
    tutorial_paths = sorted(
        path for path in (root / "docs/tutorials").glob("*.md")
        if path.name != "README.md"
    )
    if not tutorial_paths:
        fail("docs/tutorials", "no tutorials discovered")
    for tutorial_path in tutorial_paths:
        path = tutorial_path.relative_to(root).as_posix()
        text = read(root, path)
        if "checked-out RustLace development revision" not in text:
            fail(path, "tutorial does not declare its executable baseline")
        if f"({tutorial_path.name})" not in tutorial_index:
            fail("docs/tutorials/README.md", f"unlisted tutorial: {tutorial_path.name}")

    # The general example runner supports explicit not-run exemptions. CI must
    # not allow a documentation author to suppress a failing example with only
    # a free-text annotation. Pin each waiver to its exact source and expiry.
    try:
        waiver_records = json.loads(read(root, "docs/example-waivers.json"))["waivers"]
    except (ValueError, KeyError, TypeError) as exc:
        fail("docs/example-waivers.json", f"invalid waiver inventory: {exc}")
    if not isinstance(waiver_records, list):
        fail("docs/example-waivers.json", "waivers must be an array")
    allowed = {}
    for entry in waiver_records:
        if not isinstance(entry, dict):
            fail("docs/example-waivers.json", "waiver entries must be objects")
        keys = {"id", "path", "sha256", "justification", "approved_by", "expires"}
        if set(entry) != keys:
            fail("docs/example-waivers.json", f"waiver fields must be exactly {sorted(keys)}")
        ident = entry["id"]
        if not isinstance(ident, str) or not re.fullmatch(r"WAIVER-[A-Z0-9-]+", ident):
            fail("docs/example-waivers.json", "invalid waiver ID")
        if ident in allowed:
            fail("docs/example-waivers.json", f"duplicate waiver {ident}")
        if not all(isinstance(entry[k], str) and entry[k].strip() for k in keys):
            fail("docs/example-waivers.json", f"{ident}: missing field")
        if not re.fullmatch(r"[a-f0-9]{64}", entry["sha256"]):
            fail("docs/example-waivers.json", f"{ident}: invalid SHA-256")
        try:
            expiry = date.fromisoformat(entry["expires"])
        except ValueError:
            fail("docs/example-waivers.json", f"{ident}: invalid expiry")
        if expiry < date.today():
            fail("docs/example-waivers.json", f"{ident}: expired")
        allowed[ident] = entry
    referenced = set()
    for doc in (root / "docs").rglob("*.md"):
        rel = doc.relative_to(root).as_posix()
        if (rel.startswith(("docs/goal-", "docs/csharp-oracle/", "docs/internal/archive/"))
                or rel.startswith("docs/tutorials/")):
            continue
        source = doc.read_text(encoding="utf-8").replace("\r\n", "\n")
        # Scan original lines with the executable runner's fence and annotation
        # grammar. A greedy prose regex used to miss '<', '>' and trailing text,
        # allowing an unregistered waiver to suppress execution.
        for reason, script in scan_not_run_annotations(rel, doc.read_text(encoding="utf-8")):
            match = re.match(r"(WAIVER-[A-Z0-9-]+)\b", reason)
            if not match or match.group(1) not in allowed:
                fail(rel, f"unapproved not-run annotation: {reason!r}")
            ident = match.group(1)
            entry = allowed[ident]
            if rel != entry["path"] or ident in referenced:
                fail(rel, f"{ident}: path mismatch or duplicate use")
            digest = hashlib.sha256(script.encode("utf-8")).hexdigest()
            if digest != entry["sha256"]:
                fail(rel, f"{ident}: documented script changed (sha256 mismatch)")
            referenced.add(ident)
    if referenced != set(allowed):
        fail("docs/example-waivers.json", f"unused waivers: {sorted(set(allowed)-referenced)}")

    return dict(crates=len(members), skills=skills, adrs=adrs, corpus=total,
                tutorials=len(tutorial_paths))


def self_test(root: Path) -> int:
    with tempfile.TemporaryDirectory(prefix="rustlace-metadata-") as tmp:
        baseline = Path(tmp) / "fixture"
        for path in (
            "Cargo.toml", "README.md", "AGENTS.md", "CONTRACT.md",
            "docs/parity-with-studio.md", "docs/build-and-run.md",
            "docs/skills.md", "docs/tutorials/README.md",
            "docs/crates/lovelace-corpus.md", "docs/version-applicability.md",
            "docs/example-waivers.json",
        ):
            target = baseline / path
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(root / path, target)
        for dirname in ("docs/adr", ".dsh/skills", "parity/corpus"):
            shutil.copytree(root / dirname, baseline / dirname)
        for name in ("linear-algebra", "symbolic", "signal-processing", "reproducible"):
            path = f"docs/tutorials/{name}.md"
            shutil.copy2(root / path, baseline / path)

        validate(baseline)
        mutations = [
            ("crate-count", "README.md", "fourteen workspace members", "twelve workspace members"),
            ("canonical-owner-omitted", "README.md",
             "| \x60lovelace-rational\x60 |", "| \x60lovelace-rational-old\x60 |"),
            ("build-count", "docs/build-and-run.md", "fourteen members", "twelve members"),
            ("skill-count", "README.md", "eight task skills", "nine task skills"),
            ("corpus-count", "README.md", "for 232 records", "for 231 records"),
            ("tutorial-version", "docs/tutorials/linear-algebra.md",
             "checked-out RustLace development revision", "RustLace 0.4.1"),
            ("normative-corpus-count", "CONTRACT.md",
             "lang.json` holds 112 records", "lang.json` holds 111 records"),
            ("parity-inventory", "docs/parity-with-studio.md",
             "lang.json` – 112 records", "lang.json` – 111 records"),
        ]
        # An agent must not be able to silence a failing example simply by
        # inserting a not-run annotation while leaving other examples green.
        waived_page = baseline / "docs/crates/lovelace-corpus.md"
        original_waiver_page = waived_page.read_text(encoding="utf-8")
        waived_page.write_text(
            original_waiver_page + "\n<!-- not-run: because the example fails -->\n"
            + "```lovelace\n1 / 0\n```\n",
            encoding="utf-8",
        )
        try:
            validate(baseline)
        except MetadataFailure as exc:
            if "unapproved not-run" not in str(exc):
                fail("docs/crates/lovelace-corpus.md", f"wrong waiver rejection: {exc}")
        else:
            fail("docs/crates/lovelace-corpus.md", "unapproved waiver passed")
        waived_page.write_text(original_waiver_page, encoding="utf-8")

        for label, file, original, replacement in mutations:
            target = baseline / file
            content = target.read_text(encoding="utf-8")
            if original not in content:
                fail(file, f"{label}: mutation anchor missing")
            target.write_text(content.replace(original, replacement, 1), encoding="utf-8")
            try:
                validate(baseline)
            except MetadataFailure as exc:
                if file not in str(exc):
                    fail(file, f"{label}: wrong rejection: {exc}")
            else:
                fail(file, f"{label}: mutant passed")
            target.write_text(content, encoding="utf-8")

        # A correct count must not mask a second stale assertion elsewhere.
        # The parity page already contains an uncounted corpus link before its
        # inventory; the clean fixture also guards against the old first-hit bug.
        parity_page = baseline / "docs/parity-with-studio.md"
        original = parity_page.read_text(encoding="utf-8")
        parity_page.write_text(
            original + "\n- `parity/corpus/lang.json` – 111 records (obsolete)\n",
            encoding="utf-8",
        )
        try:
            validate(baseline)
        except MetadataFailure as exc:
            if "docs/parity-with-studio.md: lang inventory drift" not in str(exc):
                fail("docs/parity-with-studio.md", f"wrong duplicate-count rejection: {exc}")
        else:
            fail("docs/parity-with-studio.md", "contradictory count alongside correct count passed")
        parity_page.write_text(original, encoding="utf-8")

        # A fifth tutorial is never silently exempt from version disclosure.
        fifth_tutorial = baseline / "docs/tutorials/newcomer.md"
        fifth_tutorial.write_text("# New tutorial\n", encoding="utf-8")
        try:
            validate(baseline)
        except MetadataFailure as exc:
            if "docs/tutorials/newcomer.md: tutorial does not declare" not in str(exc):
                fail("tutorial-version-discovery", f"wrong rejection: {exc}")
        else:
            fail("tutorial-version-discovery", "new unversioned tutorial passed")
        fifth_tutorial.write_text(
            "# New tutorial\nchecked-out RustLace development revision\n",
            encoding="utf-8",
        )
        try:
            validate(baseline)
        except MetadataFailure as exc:
            if "docs/tutorials/README.md: unlisted tutorial" not in str(exc):
                fail("tutorial-index-discovery", f"wrong rejection: {exc}")
        else:
            fail("tutorial-index-discovery", "unlisted tutorial passed")
        fifth_tutorial.unlink()

        # Exercise the actual waiver validator, not just a free-text reject.
        # Include the exact annotation shapes that formerly diverged from the
        # shell executor: '<', '>', and text following the closing '-->'.
        inventory_file = baseline / "docs/example-waivers.json"
        waiver_page = baseline / "docs/crates/lovelace-corpus.md"
        saved_inventory = inventory_file.read_text(encoding="utf-8")
        saved_page = waiver_page.read_text(encoding="utf-8")
        waived_script = "1 / 0"
        approved = {
            "id": "WAIVER-TEST",
            "path": "docs/crates/lovelace-corpus.md",
            "sha256": hashlib.sha256(waived_script.encode("utf-8")).hexdigest(),
            "justification": "Only a self-test; do not use in actual docs",
            "approved_by": "metadata-self-test",
            "expires": "2099-12-31",
        }
        extra_mutants = 0

        def waiver_case(label: str, annotation: str, records: list[dict],
                        expected: str | None) -> None:
            nonlocal extra_mutants
            waiver_page.write_text(
                saved_page + "\n" + annotation
                + "\n" + "\x60\x60\x60lovelace\n" + waived_script + "\n\x60\x60\x60\n",
                encoding="utf-8",
            )
            inventory_file.write_text(
                json.dumps({"waivers": records}, ensure_ascii=False),
                encoding="utf-8",
            )
            try:
                validate(baseline)
            except MetadataFailure as exc:
                if expected is None or expected not in str(exc):
                    fail(label, f"unexpected waiver rejection: {exc}")
            else:
                if expected is not None:
                    fail(label, "invalid waiver passed")
            finally:
                waiver_page.write_text(saved_page, encoding="utf-8")
                inventory_file.write_text(saved_inventory, encoding="utf-8")
            if expected is not None:
                extra_mutants += 1

        # Positive authorization proves '<', '>', and trailing prose are not
        # mistaken for unused waivers or silently ignored.
        waiver_case("approved-angle", "<!-- not-run: WAIVER-TEST because x < 0 > 1 -->",
                    [approved], None)
        waiver_case("approved-trailing", "<!-- not-run: WAIVER-TEST --> see the note",
                    [approved], None)

        for label, annotation in (
            ("unapproved-less-than", "<!-- not-run: because x < 0 is undefined -->"),
            ("unapproved-greater-than", "<!-- not-run: because x > 0 is undefined -->"),
            ("unapproved-trailing", "<!-- not-run: WAIVER-TEST --> see the note"),
        ):
            waiver_case(label, annotation, [], "unapproved not-run")

        waiver_case("expired", "<!-- not-run: WAIVER-TEST -->",
                    [{**approved, "expires": "2000-01-01"}], "expired")
        waiver_case("wrong-digest", "<!-- not-run: WAIVER-TEST -->",
                    [{**approved, "sha256": "0" * 64}], "sha256 mismatch")
        waiver_case("invalid-digest-shape", "<!-- not-run: WAIVER-TEST -->",
                    [{**approved, "sha256": "0" * 63}], "invalid SHA-256")
        waiver_case("path-mismatch", "<!-- not-run: WAIVER-TEST -->",
                    [{**approved, "path": "docs/other.md"}], "path mismatch")
        waiver_case("duplicate-id", "<!-- not-run: WAIVER-TEST -->",
                    [approved, approved], "duplicate waiver")
        waiver_case("unused-id", "<!-- not-run: WAIVER-TEST -->",
                    [approved, {**approved, "id": "WAIVER-UNUSED"}], "unused waivers")
        waiver_case("missing-field", "<!-- not-run: WAIVER-TEST -->",
                    [{key: value for key, value in approved.items()
                      if key != "approved_by"}], "waiver fields")
        waiver_case("changed-script", "<!-- not-run: WAIVER-TEST -->",
                    [{**approved, "sha256": hashlib.sha256(b"2 / 0").hexdigest()}],
                    "sha256 mismatch")
    return len(mutations) + 4 + extra_mutants


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", type=Path, default=ROOT)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    try:
        if args.self_test:
            mutants = self_test(args.root)
            print(f"DOCREPORT gate=doc-metadata-mutants status=ok mutants={mutants} failed=0")
        else:
            result = validate(args.root)
            fields = " ".join(f"{key}={value}" for key, value in result.items())
            print(f"DOCREPORT gate=doc-metadata status=ok {fields} failed=0")
        return 0
    except (OSError, KeyError, MetadataFailure) as exc:
        print(f"doc-metadata: FAIL: {exc}", file=sys.stderr)
        name = "doc-metadata-mutants" if args.self_test else "doc-metadata"
        print(f"DOCREPORT gate={name} status=fail failed=1")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
