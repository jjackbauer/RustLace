#!/usr/bin/env python3
"""Deterministic architecture-policy checker for RustLace (RNL-GOV-003/004/005/010).

The checker reads `tools/architecture-policy.json`, inspects the **actual** workspace graph through
`cargo metadata`, scans every workspace crate's Rust sources for unauthorized numerical types and
arithmetic implementations, validates policy exceptions, and exits nonzero on any violation.

Exit codes:
    0  the policy is satisfied (zero unwaived violations)
    1  at least one mandatory rule is violated
    2  the checker could not do its job (missing/unparsable policy, no workspace, no sources,
       missing required crate/file, command failure). A zero-scan result is never a pass.

Usage:
    python tools/check-architecture.py [--root PATH] [--metadata PATH] [--json] [--report PATH]
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

EXIT_OK = 0
EXIT_VIOLATION = 1
EXIT_CONFIG = 2

SKIP_DIRS = {"target", ".git", ".tmp", "node_modules", "dist"}


class ConfigError(Exception):
    """The checker cannot perform its scan; this is never reported as success."""


@dataclass
class Violation:
    rule: str
    path: str
    line: int
    message: str
    crate: str = ""

    def sort_key(self) -> tuple:
        return (self.path, self.line, self.rule, self.message)

    def as_dict(self) -> dict:
        return {
            "rule": self.rule,
            "path": self.path,
            "line": self.line,
            "message": self.message,
            "crate": self.crate,
        }


@dataclass
class Declaration:
    name: str
    kind: str
    fields: list[str]
    field_types: list[str]
    path: str
    line: int


@dataclass
class ImplBlock:
    type_name: str
    trait_name: str | None
    methods: list[str]
    delegates: bool
    path: str
    line: int


# ---------------------------------------------------------------------------------------------
# Policy and metadata
# ---------------------------------------------------------------------------------------------


def load_policy(root: Path) -> dict:
    path = root / "tools" / "architecture-policy.json"
    if not path.is_file():
        raise ConfigError(f"missing architecture policy: {path}")
    try:
        policy = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ConfigError(f"cannot parse {path}: {error}")
    if not isinstance(policy, dict):
        raise ConfigError("the architecture policy must be a JSON object")
    if policy.get("schemaVersion") != 1:
        raise ConfigError(
            f"unsupported architecture policy schemaVersion: {policy.get('schemaVersion')!r}"
        )
    for key in ("layers", "crates", "allowedEdges", "numericOwners", "duplicateDetection"):
        if key not in policy:
            raise ConfigError(f"the architecture policy is missing '{key}'")
    return policy


def find_cargo() -> str:
    import os

    candidates: list[str] = []
    if os.environ.get("CARGO"):
        candidates.append(os.environ["CARGO"])
    found = shutil.which("cargo")
    if found:
        candidates.append(found)
    home = Path.home()
    candidates.append(str(home / ".cargo" / "bin" / "cargo"))
    candidates.append(str(home / ".cargo" / "bin" / "cargo.exe"))
    profile = Path(os.environ.get("USERPROFILE", "")) if os.environ.get("USERPROFILE") else None
    if profile:
        candidates.append(str(profile / ".cargo" / "bin" / "cargo.exe"))
    for candidate in candidates:
        if candidate and Path(candidate).is_file():
            return candidate
    raise ConfigError(
        "cannot locate cargo; install the Rust toolchain or pass --metadata with a recorded graph"
    )


def load_metadata(root: Path, override: str | None) -> dict:
    if override:
        path = Path(override)
        if not path.is_file():
            raise ConfigError(f"metadata file does not exist: {path}")
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise ConfigError(f"cannot parse metadata file {path}: {error}")
    else:
        # Never let cargo walk up to a parent workspace: a fixture root without its own manifest
        # must fail rather than silently check a different tree.
        if not (root / "Cargo.toml").is_file():
            raise ConfigError(
                f"no Cargo.toml at {root}; pass --metadata for a recorded fixture graph"
            )
        cargo = find_cargo()
        command = [
            cargo,
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ]
        completed = subprocess.run(
            command, cwd=root, capture_output=True, text=True, check=False
        )
        if completed.returncode != 0:
            # The offline flag is the engine-development default. A cold CI runner may not have
            # the registry index cached yet; retry once with the network allowed, and keep the
            # offline failure in the diagnostic if neither attempt works.
            offline_error = completed.stderr or completed.stdout
            online = [argument for argument in command if argument != "--offline"]
            completed = subprocess.run(
                online, cwd=root, capture_output=True, text=True, check=False
            )
            if completed.returncode != 0:
                raise ConfigError(
                    "cargo metadata failed offline and online; the checker cannot inspect the workspace:\n"
                    + "offline: " + offline_error + "\nonline: " + (completed.stderr or completed.stdout)
                )
        try:
            data = json.loads(completed.stdout)
        except json.JSONDecodeError as error:
            raise ConfigError(f"cargo metadata returned invalid JSON: {error}")
    if not isinstance(data, dict) or not data.get("packages"):
        raise ConfigError("the workspace metadata enumerates no packages; refusing a vacuous pass")
    if not isinstance(data.get("packages"), list):
        raise ConfigError("the workspace metadata 'packages' field is not a list")
    return data


def package_manifest_dir(root: Path, package: dict) -> Path:
    manifest = package.get("manifest_path")
    if manifest:
        path = Path(manifest)
        if not path.is_absolute():
            path = root / path
        return path.parent
    # Recorded fixtures may omit manifest_path; policy crates live under <root>/<name>.
    return root / package["name"]


# ---------------------------------------------------------------------------------------------
# Dependency-graph checks
# ---------------------------------------------------------------------------------------------


def dependency_name(dependency: dict) -> str:
    return dependency.get("name", "")


def dependency_kind(dependency: dict) -> str:
    return dependency.get("kind") or "normal"


def check_policy_shape(policy: dict) -> None:
    layers = policy["layers"]
    crates = policy["crates"]
    if not isinstance(crates, dict) or not crates:
        raise ConfigError("the policy declares no crates")
    for name, spec in crates.items():
        if spec.get("layer") not in layers:
            raise ConfigError(f"crate '{name}' declares unknown layer {spec.get('layer')!r}")
        if not str(spec.get("responsibility", "")).strip():
            raise ConfigError(f"crate '{name}' does not declare a responsibility")
        if "reuses" not in spec or not isinstance(spec["reuses"], list):
            raise ConfigError(f"crate '{name}' does not declare the abstractions it reuses")
    for name, edges in policy["allowedEdges"].items():
        if name not in crates:
            raise ConfigError(f"allowedEdges names undeclared crate '{name}'")
        if not isinstance(edges, list):
            raise ConfigError(f"allowedEdges for '{name}' is not a list")
        for edge in edges:
            if edge not in crates:
                raise ConfigError(f"allowedEdges for '{name}' names undeclared crate '{edge}'")
    detection = policy["duplicateDetection"]
    for key in ("ownerCrates", "forbiddenTypeNames", "numericFieldPairs", "arithmeticMethods"):
        if key not in detection:
            raise ConfigError(f"duplicateDetection is missing '{key}'")


def check_graph(
    policy: dict, metadata: dict, root: Path, violations: list[Violation]
) -> tuple[list[str], list[tuple[str, str]]]:
    packages = metadata["packages"]
    workspace_members = metadata.get("workspace_members") or []
    member_ids = set(workspace_members)
    if member_ids:
        members = [package for package in packages if package.get("id") in member_ids]
        if not members:
            # A recorded fixture may list names instead of opaque ids.
            member_names = {str(entry) for entry in workspace_members}
            members = [package for package in packages if package["name"] in member_names]
    else:
        members = list(packages)
    if not members:
        raise ConfigError("the workspace metadata enumerates no member packages")
    workspace_names = {package["name"] for package in members}
    policy_crates = policy["crates"]
    excluded = policy.get("excludedPackages", {})

    for name in workspace_names:
        if name not in policy_crates and name not in excluded:
            violations.append(
                Violation(
                    "undeclared-crate",
                    "tools/architecture-policy.json",
                    0,
                    f"workspace crate '{name}' is not declared in the architecture policy",
                    name,
                )
            )
    for name in policy_crates:
        if name not in workspace_names:
            violations.append(
                Violation(
                    "unknown-policy-crate",
                    "tools/architecture-policy.json",
                    0,
                    f"the policy declares crate '{name}', which is not a workspace member",
                    name,
                )
            )

    allowed_edges = policy["allowedEdges"]
    dev_edges = policy.get("devEdgesAllowed", {})
    third_party = policy.get("thirdPartyAllowed", {})
    layers = policy["layers"]

    normal_edges: list[tuple[str, str]] = []
    all_edges: dict[str, list[str]] = {name: [] for name in workspace_names}

    for package in members:
        crate = package["name"]
        for dependency in package.get("dependencies", []):
            target = dependency_name(dependency)
            kind = dependency_kind(dependency)
            if target in workspace_names:
                edge_allowed = target in allowed_edges.get(crate, [])
                if kind in ("dev", "build"):
                    edge_allowed = edge_allowed or target in dev_edges.get(crate, [])
                if not edge_allowed:
                    violations.append(
                        Violation(
                            "forbidden-dependency",
                            "Cargo.toml",
                            0,
                            f"crate '{crate}' has a {kind} dependency on '{target}', which the policy does not allow",
                            crate,
                        )
                    )
                all_edges[crate].append(target)
                if kind == "normal":
                    normal_edges.append((crate, target))
                rank_crate = layers.get(policy_crates.get(crate, {}).get("layer"), 99)
                rank_target = layers.get(policy_crates.get(target, {}).get("layer"), 99)
                if rank_target > rank_crate:
                    violations.append(
                        Violation(
                            "layer-direction",
                            "Cargo.toml",
                            0,
                            f"crate '{crate}' (layer rank {rank_crate}) depends on higher-layer '{target}' (rank {rank_target})",
                            crate,
                        )
                    )
            else:
                permitted = target in third_party.get(crate, [])
                if not permitted:
                    violations.append(
                        Violation(
                            "third-party-dependency",
                            "Cargo.toml",
                            0,
                            f"crate '{crate}' depends on third-party package '{target}', which is not permitted for that crate",
                            crate,
                        )
                    )

    # Cycles over every workspace edge (normal and dev); a cycle is never legal.
    color = {name: 0 for name in workspace_names}
    stack: list[str] = []

    def visit(node: str) -> None:
        color[node] = 1
        stack.append(node)
        for neighbour in all_edges.get(node, []):
            if neighbour not in color:
                continue
            if color[neighbour] == 1:
                start = stack.index(neighbour)
                cycle = stack[start:] + [neighbour]
                violations.append(
                    Violation(
                        "dependency-cycle",
                        "Cargo.toml",
                        0,
                        "workspace dependency cycle: " + " -> ".join(cycle),
                        node,
                    )
                )
            elif color[neighbour] == 0:
                visit(neighbour)
        stack.pop()
        color[node] = 2

    for name in sorted(workspace_names):
        if color[name] == 0:
            visit(name)

    return sorted(workspace_names), normal_edges


# ---------------------------------------------------------------------------------------------
# Rust source scanning
# ---------------------------------------------------------------------------------------------


def clean_rust_source(text: str) -> str:
    """Blank comments, strings and char literals while preserving byte offsets and newlines."""
    out = list(text)
    length = len(text)
    index = 0

    def mask(start: int, end: int) -> None:
        for position in range(start, min(end, length)):
            if out[position] != "\n":
                out[position] = " "

    while index < length:
        char = text[index]
        # Line comment.
        if char == "/" and index + 1 < length and text[index + 1] == "/":
            end = text.find("\n", index)
            end = length if end < 0 else end
            mask(index, end)
            index = end
            continue
        # Block comment, nested.
        if char == "/" and index + 1 < length and text[index + 1] == "*":
            depth = 1
            cursor = index + 2
            while cursor < length and depth:
                if text.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif text.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            mask(index, cursor)
            index = cursor
            continue
        # Raw string, including raw byte strings.
        raw_prefix = 1 if char == "r" else (2 if char == "b" and text.startswith("br", index) else 0)
        if raw_prefix:
            cursor = index + raw_prefix
            hashes = 0
            while cursor < length and text[cursor] == "#":
                hashes += 1
                cursor += 1
            if cursor < length and text[cursor] == '"':
                closing = '"' + "#" * hashes
                end = text.find(closing, cursor + 1)
                end = length if end < 0 else end + len(closing)
                mask(index, end)
                index = end
                continue
        # Ordinary string and byte string.
        if char == '"' or (char == "b" and index + 1 < length and text[index + 1] == '"'):
            cursor = index + (2 if char == "b" else 1)
            while cursor < length:
                if text[cursor] == "\\":
                    cursor += 2
                    continue
                if text[cursor] == '"':
                    cursor += 1
                    break
                cursor += 1
            mask(index, cursor)
            index = cursor
            continue
        # Char literal or byte char, but not a lifetime such as 'a.
        if char == "'" or (char == "b" and index + 1 < length and text[index + 1] == "'"):
            start = index + (2 if char == "b" else 1)
            cursor = start
            if cursor < length and text[cursor] == "\\":
                cursor += 1
                while cursor < length and text[cursor] != "'" and cursor - start < 12:
                    cursor += 1
                if cursor < length and text[cursor] == "'":
                    mask(index, cursor + 1)
                    index = cursor + 1
                    continue
            elif cursor + 1 < length and text[cursor + 1] == "'":
                mask(index, cursor + 2)
                index = cursor + 2
                continue
        index += 1
    return "".join(out)


def match_delimiter(text: str, open_index: int, opener: str, closer: str) -> int:
    depth = 0
    for index in range(open_index, len(text)):
        char = text[index]
        if char == opener:
            depth += 1
        elif char == closer:
            depth -= 1
            if depth == 0:
                return index
    raise ConfigError(f"unbalanced {opener!r} starting at byte {open_index}")


def line_of(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def top_level_field_names(body: str, base_offset: int, text: str) -> tuple[list[str], list[str]]:
    names: list[str] = []
    types: list[str] = []
    depth = 0
    index = 0
    length = len(body)
    while index < length:
        char = body[index]
        if char in "{([<":
            depth += 1
            index += 1
            continue
        if char in "})]>":
            depth -= 1
            index += 1
            continue
        if depth == 0:
            match = re.match(r"(?:pub(?:\([^)]*\))?\s+)?([A-Za-z_]\w*)\s*:\s*([^,}\n]+)", body[index:])
            if match:
                name = match.group(1)
                if name not in ("where", "for", "if", "match"):
                    names.append(name)
                    field_type = match.group(2).strip()
                    type_name = re.search(r"[A-Za-z_]\w*", field_type)
                    types.append(type_name.group(0) if type_name else field_type)
                index += match.end()
                continue
        index += 1
    return names, types


def tuple_field_types(body: str) -> list[str]:
    fields: list[str] = []
    depth = 0
    current: list[str] = []
    for char in body:
        if char in "<([":
            depth += 1
        elif char in ">)]":
            depth -= 1
        if char == "," and depth == 0:
            field = "".join(current).strip()
            if field:
                fields.append(field)
            current = []
        else:
            current.append(char)
    field = "".join(current).strip()
    if field:
        fields.append(field)
    types: list[str] = []
    for field in fields:
        stripped = re.sub(r"^(?:pub(?:\([^)]*\))?\s+)", "", field)
        match = re.search(r"[A-Za-z_]\w*", stripped)
        types.append(match.group(0) if match else stripped)
    return types


def split_impl_header(header: str) -> tuple[str | None, str]:
    """Split `impl` header text into (trait, type). `for<'a>` bounds are not separators."""
    depth = 0
    for_index = -1
    index = 0
    while index < len(header):
        char = header[index]
        if char in "<([":
            depth += 1
        elif char in ">)]":
            depth -= 1
        elif depth == 0 and header.startswith("for", index):
            before = header[index - 1] if index > 0 else " "
            after = header[index + 3] if index + 3 < len(header) else " "
            if not (before.isalnum() or before == "_") and not (after.isalnum() or after == "_"):
                for_index = index
                break
        index += 1
    if for_index < 0:
        return None, header
    trait = header[:for_index].strip()
    type_part = header[for_index + 3 :].strip()
    return trait, type_part


def type_name_of(text: str) -> str:
    identifiers = re.findall(r"[A-Za-z_]\w*", text)
    return identifiers[-1] if identifiers else ""


def trait_name_of(text: str) -> str:
    text = text.split("<", 1)[0]
    identifiers = re.findall(r"[A-Za-z_]\w*", text)
    return identifiers[-1] if identifiers else ""


def method_names(body: str) -> list[str]:
    names: list[str] = []
    depth = 0
    index = 0
    while index < len(body):
        char = body[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
        elif depth == 0 and body.startswith("fn", index):
            before = body[index - 1] if index > 0 else " "
            after = body[index + 2] if index + 2 < len(body) else " "
            if not (before.isalnum() or before == "_") and not (after.isalnum() or after == "_"):
                match = re.match(r"fn\s+([A-Za-z_]\w*)", body[index:])
                if match:
                    names.append(match.group(1))
                    index += match.end()
                    continue
        index += 1
    return names


def analyze_file(path: Path, relative: str, text: str) -> tuple[list[Declaration], list[ImplBlock]]:
    cleaned = clean_rust_source(text)
    declarations: list[Declaration] = []
    impls: list[ImplBlock] = []

    for match in re.finditer(r"\b(struct|enum|union|trait)\s+([A-Za-z_]\w*)", cleaned):
        kind = match.group(1)
        name = match.group(2)
        cursor = match.end()
        # Skip generics and where clauses until the body (or a semicolon for a unit struct).
        depth = 0
        body_open = -1
        body_kind = ""
        while cursor < len(cleaned):
            char = cleaned[cursor]
            if char == "<":
                depth += 1
            elif char == ">":
                depth = max(0, depth - 1)
            elif depth == 0 and char in "{(":
                body_open = cursor
                body_kind = char
                break
            elif depth == 0 and char == ";":
                break
            cursor += 1
        fields: list[str] = []
        field_types: list[str] = []
        if body_open >= 0:
            closer = "}" if body_kind == "{" else ")"
            body_close = match_delimiter(cleaned, body_open, body_kind, closer)
            body = cleaned[body_open + 1 : body_close]
            if body_kind == "{":
                fields, field_types = top_level_field_names(body, body_open + 1, cleaned)
            else:
                field_types = tuple_field_types(body)
        declarations.append(
            Declaration(
                name=name,
                kind=kind,
                fields=fields,
                field_types=field_types,
                path=relative,
                line=line_of(cleaned, match.start()),
            )
        )

    for match in re.finditer(r"\bimpl\b", cleaned):
        cursor = match.end()
        angle = 0
        paren = 0
        brace = -1
        while cursor < len(cleaned):
            char = cleaned[cursor]
            if char == "<":
                angle += 1
            elif char == ">" and angle:
                angle -= 1
            elif char == "(":
                paren += 1
            elif char == ")" and paren:
                paren -= 1
            elif char == "{" and angle == 0 and paren == 0:
                brace = cursor
                break
            elif char == ";" and angle == 0 and paren == 0:
                break
            cursor += 1
        if brace < 0:
            continue
        header = cleaned[match.end() : brace].strip()
        body_close = match_delimiter(cleaned, brace, "{", "}")
        body = cleaned[brace + 1 : body_close]
        trait, type_part = split_impl_header(header)
        type_name = type_name_of(type_part)
        if not type_name:
            continue
        methods = method_names(body)
        # A permitted specialization must delegate a numerical operation to the canonical field,
        # not merely clone it. Require a canonical arithmetic method call on the wrapped value.
        delegates = bool(
            re.search(
                r"self\s*\.\s*(?:0|value|inner|inner_value)\s*\.\s*"
                r"(?:add|sub|mul|div|rem|neg|abs|pow|gcd|lcm|reduce|normalize|reciprocal)\s*\(",
                body,
            )
        )
        impls.append(
            ImplBlock(
                type_name=type_name,
                trait_name=trait_name_of(trait) if trait else None,
                methods=methods,
                delegates=delegates,
                path=relative,
                line=line_of(cleaned, match.start()),
            )
        )
    return declarations, impls


def scan_crate_sources(
    crate: str,
    directory: Path,
    root: Path,
    violations: list[Violation],
) -> int:
    files = 0
    declarations: list[Declaration] = []
    impls: list[ImplBlock] = []
    for path in sorted(directory.rglob("*.rs")):
        if any(part in SKIP_DIRS for part in path.relative_to(directory).parts):
            continue
        relative = path.relative_to(root).as_posix()
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            raise ConfigError(f"cannot read source file {relative}: {error}")
        file_declarations, file_impls = analyze_file(path, relative, text)
        declarations.extend(file_declarations)
        impls.extend(file_impls)
        files += 1

    detection = None
    return files, declarations, impls


# ---------------------------------------------------------------------------------------------
# Duplicate numeric implementation detection
# ---------------------------------------------------------------------------------------------


def numeric_field_match(
    declaration: Declaration, detection: dict, arithmetic_method_count: int
) -> bool:
    """True when a declaration is a plausible general-purpose scalar representation.

    A named `numerator`/`denominator`-style pair is conclusive. Arbitrary field names are only
    suspicious when the type also carries at least the configured minimum of scalar arithmetic
    methods; a struct with two numeric configuration fields and no arithmetic is not a competing
    kernel. Enums are excluded structurally: their variants are not a numerator/denominator pair.
    """
    if declaration.kind not in ("struct", "union"):
        return False
    fields = {name.lower() for name in declaration.fields}
    for pair in detection["numericFieldPairs"]:
        if all(field.lower() in fields for field in pair):
            return True
    numeric_types = {name.lower() for name in detection["numericFieldTypes"]}
    typed = [name for name in declaration.field_types if name.lower() in numeric_types]
    minimum = int(detection.get("minimumArithmeticMethods", 4))
    return len(typed) >= 2 and arithmetic_method_count >= minimum


def check_duplicates(
    policy: dict,
    declarations: dict[str, list[Declaration]],
    impls: dict[str, list[ImplBlock]],
    violations: list[Violation],
) -> None:
    detection = policy["duplicateDetection"]
    owners = set(detection["ownerCrates"])
    forbidden_names = set(detection["forbiddenTypeNames"])
    arithmetic_methods = set(detection["arithmeticMethods"])
    arithmetic_traits = set(detection["arithmeticTraits"])
    minimum = int(detection.get("minimumArithmeticMethods", 4))
    specializations = {
        (entry["crate"], entry["type"])
        for entry in detection.get("permittedSpecializations", [])
        if "crate" in entry and "type" in entry
    }
    aliases = {
        (entry["crate"], entry["name"], entry.get("target", ""))
        for entry in detection.get("permittedAliases", [])
    }

    for crate, crate_declarations in declarations.items():
        if not crate_declarations or crate in owners:
            continue
        by_name = {declaration.name: declaration for declaration in crate_declarations}
        crate_impls = impls.get(crate, [])

        method_counts: dict[str, int] = {}
        for impl in crate_impls:
            if impl.type_name in by_name:
                count = len(set(impl.methods) & arithmetic_methods)
                method_counts[impl.type_name] = max(method_counts.get(impl.type_name, 0), count)

        for declaration in crate_declarations:
            if declaration.name in forbidden_names:
                violations.append(
                    Violation(
                        "duplicate-numeric-type",
                        declaration.path,
                        declaration.line,
                        f"crate '{crate}' declares numeric type '{declaration.name}'; the canonical implementation belongs to {policy['numericOwners'].get('rational', 'lovelace-rational')}",
                        crate,
                    )
                )
            if numeric_field_match(
                declaration, detection, method_counts.get(declaration.name, 0)
            ):
                violations.append(
                    Violation(
                        "duplicate-numeric-representation",
                        declaration.path,
                        declaration.line,
                        f"crate '{crate}' declares '{declaration.name}' with a numerator/denominator-like numeric representation",
                        crate,
                    )
                )

        for impl in crate_impls:
            if impl.type_name not in by_name:
                continue
            permitted = (crate, impl.type_name) in specializations
            if impl.trait_name in arithmetic_traits:
                if permitted:
                    if not impl.delegates:
                        violations.append(
                            Violation(
                                "specialization-not-delegating",
                                impl.path,
                                impl.line,
                                f"permitted domain specialization '{crate}::{impl.type_name}' implements {impl.trait_name} without delegating to a canonical primitive",
                                crate,
                            )
                        )
                else:
                    violations.append(
                        Violation(
                            "duplicate-arithmetic-trait",
                            impl.path,
                            impl.line,
                            f"crate '{crate}' implements arithmetic trait {impl.trait_name} for '{impl.type_name}' outside the canonical owners",
                            crate,
                        )
                    )
            count = len(set(impl.methods) & arithmetic_methods)
            declaration = by_name[impl.type_name]
            if count >= minimum and (
                numeric_field_match(declaration, detection, count)
                or declaration.name in forbidden_names
            ):
                if not permitted:
                    violations.append(
                        Violation(
                            "duplicate-arithmetic-impl",
                            impl.path,
                            impl.line,
                            f"crate '{crate}' implements {count} scalar arithmetic methods on '{impl.type_name}' outside the canonical owners",
                            crate,
                        )
                    )


# ---------------------------------------------------------------------------------------------
# Exceptions
# ---------------------------------------------------------------------------------------------


def validate_exceptions(policy: dict, root: Path, violations: list[Violation]) -> list[dict]:
    valid: list[dict] = []
    for index, exception in enumerate(policy.get("exceptions", [])):
        prefix = f"exceptions[{index}]"
        missing = [
            key
            for key in ("id", "rule", "adr", "rationale", "reviewer", "removalPlan", "scope")
            if not exception.get(key)
        ]
        reasons: list[str] = []
        if missing:
            reasons.append("missing " + ", ".join(missing))
        scope = exception.get("scope")
        if not isinstance(scope, list) or not scope or "*" in scope:
            reasons.append("scope must be a non-empty list of narrow paths/crates (not '*')")
        adr = exception.get("adr")
        if adr:
            adr_path = root / adr
            if not adr_path.is_file():
                reasons.append(f"ADR '{adr}' does not exist")
            elif "docs/adr/" not in adr.replace("\\", "/"):
                reasons.append("the exception must cite a docs/adr/ decision record")
        if len(str(exception.get("rationale", ""))) < 20:
            reasons.append("the rationale is too short to review")
        if len(str(exception.get("removalPlan", ""))) < 10:
            reasons.append("the removal plan is missing or too short")
        if reasons:
            violations.append(
                Violation(
                    "unauthorized-exception",
                    "tools/architecture-policy.json",
                    0,
                    f"{prefix} is not a valid architecture exception: " + "; ".join(reasons),
                    "lovelace-rational",
                )
            )
        else:
            valid.append(exception)
    return valid


def waive_violations(violations: list[Violation], exceptions: list[dict]) -> tuple[list[Violation], list[dict]]:
    remaining: list[Violation] = []
    waived: list[dict] = []
    for violation in violations:
        matched = None
        for exception in exceptions:
            if exception["rule"] != violation.rule:
                continue
            if any(
                fnmatch.fnmatch(violation.path, pattern)
                or fnmatch.fnmatch(violation.crate, pattern)
                or fnmatch.fnmatch(f"{violation.crate}/{violation.path}", pattern)
                for pattern in exception["scope"]
            ):
                matched = exception
                break
        if matched:
            waived.append(
                {
                    "violation": violation.as_dict(),
                    "exception": matched["id"],
                    "adr": matched["adr"],
                }
            )
        else:
            remaining.append(violation)
    return remaining, waived


# ---------------------------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------------------------


def run(root: Path, metadata_path: str | None) -> tuple[int, dict]:
    policy = load_policy(root)
    check_policy_shape(policy)
    metadata = load_metadata(root, metadata_path)
    violations: list[Violation] = []

    workspace_names, normal_edges = check_graph(policy, metadata, root, violations)

    required = policy.get("requiredChecks", {})
    for crate in required.get("requiredCrates", []):
        if crate not in workspace_names:
            raise ConfigError(f"required crate '{crate}' is not in the workspace")
    for owner in policy["numericOwners"].values():
        if owner not in workspace_names:
            raise ConfigError(f"numeric owner '{owner}' is not a workspace crate")
    # Every declared crate has a reference page: an ownership claim that is not documented is
    # not reviewable (RNL-DOC-004/007). Minimal policy fixtures opt out explicitly.
    for crate in (policy["crates"] if required.get("cratePages", True) else []):
        page = f"docs/crates/{crate}.md"
        if not (root / page).is_file():
            violations.append(
                Violation(
                    "crate-doc-missing",
                    page,
                    0,
                    f"crate '{crate}' is declared in the policy but has no reference page",
                    crate,
                )
            )
    for relative in required.get("requiredFiles", []):
        if not (root / relative).is_file():
            violations.append(
                Violation(
                    "required-file-missing",
                    relative,
                    0,
                    f"the architecture requires '{relative}', which does not exist",
                    "lovelace-rational",
                )
            )

    # The policy names the required CI status checks. A job removed from the workflow must fail
    # this check: otherwise the "required check" is only a sentence in a document.
    workflow_relative = required.get("ciWorkflow")
    if workflow_relative:
        workflow = root / workflow_relative
        if not workflow.is_file():
            violations.append(
                Violation(
                    "required-ci-job-missing",
                    workflow_relative,
                    0,
                    f"the required CI workflow '{workflow_relative}' does not exist",
                    "lovelace-rational",
                )
            )
        else:
            workflow_text = workflow.read_text(encoding="utf-8")
            for job in required.get("ciJobs", []):
                if not re.search(rf"(?m)^  {re.escape(job)}:\s*$", workflow_text):
                    violations.append(
                        Violation(
                            "required-ci-job-missing",
                            workflow_relative,
                            0,
                            f"the required CI job '{job}' is not defined in the workflow",
                            "lovelace-rational",
                        )
                    )
            if "pull_request:" not in workflow_text or "branches: [main]" not in workflow_text:
                violations.append(
                    Violation(
                        "ci-trigger-missing",
                        workflow_relative,
                        0,
                        "the workflow must run on pull requests and on changes to main",
                        "lovelace-rational",
                    )
                )

    declarations: dict[str, list[Declaration]] = {}
    impls: dict[str, list[ImplBlock]] = {}
    files_scanned = 0
    packages_by_name = {package["name"]: package for package in metadata["packages"]}
    for crate in sorted(workspace_names):
        package = packages_by_name.get(crate)
        if package is None:
            continue
        directory = package_manifest_dir(root, package)
        if not directory.is_dir():
            raise ConfigError(f"cannot locate the source directory for crate '{crate}': {directory}")
        count, crate_declarations, crate_impls = scan_crate_sources(
            crate, directory, root, violations
        )
        files_scanned += count
        declarations[crate] = crate_declarations
        impls[crate] = crate_impls

    minimum_files = int(required.get("minimumSourceFiles", 1))
    if files_scanned < minimum_files:
        raise ConfigError(
            f"only {files_scanned} Rust source files were scanned; the policy requires at least "
            f"{minimum_files}. A partial scan is not a pass."
        )
    minimum_crates = int(required.get("minimumWorkspaceCrates", 1))
    if len(workspace_names) < minimum_crates:
        raise ConfigError(
            f"only {len(workspace_names)} workspace crates were enumerated; the policy requires at "
            f"least {minimum_crates}"
        )

    check_duplicates(policy, declarations, impls, violations)

    # Forbidden locations: a file may re-export a canonical type, but it may not declare or
    # implement a numeric representation.
    forbidden = policy.get("forbiddenPaths", [])
    for crate, crate_impls in impls.items():
        # Declarations carry their own path; the union of both lists is the set of files that
        # contain an item the rule forbids.
        item_paths = {declaration.path for declaration in declarations.get(crate, [])}
        item_paths.update(impl.path for impl in crate_impls)
        for entry in forbidden:
            pattern = entry["path"]
            for relative in item_paths:
                if fnmatch.fnmatch(relative, pattern):
                    violations.append(
                        Violation(
                            entry.get("rule", "forbidden-numeric-location"),
                            relative,
                            0,
                            entry.get("reason", f"'{relative}' is a forbidden numeric location"),
                            crate,
                        )
                    )

    # Type aliases that rename a forbidden numeric type are themselves a declaration of a
    # competing name. `use ... as` re-exports are not declarations and are allowed.
    aliases = {
        (entry["crate"], entry["name"]): entry.get("target", "")
        for entry in policy["duplicateDetection"].get("permittedAliases", [])
    }
    for crate in workspace_names:
        package = packages_by_name[crate]
        directory = package_manifest_dir(root, package)
        for path in sorted(directory.rglob("*.rs")):
            if any(part in SKIP_DIRS for part in path.relative_to(directory).parts):
                continue
            relative = path.relative_to(root).as_posix()
            try:
                text = clean_rust_source(path.read_text(encoding="utf-8"))
            except (OSError, UnicodeDecodeError) as error:
                raise ConfigError(f"cannot read {relative}: {error}")
            for match in re.finditer(r"\btype\s+([A-Za-z_]\w*)\s*=", text):
                name = match.group(1)
                if name in policy["duplicateDetection"]["forbiddenTypeNames"] and (crate, name) not in aliases:
                    violations.append(
                        Violation(
                            "undeclared-numeric-alias",
                            relative,
                            line_of(text, match.start()),
                            f"crate '{crate}' declares type alias '{name}' for a competing numeric abstraction",
                            crate,
                        )
                    )

    valid_exceptions = validate_exceptions(policy, root, violations)
    remaining, waived = waive_violations(violations, valid_exceptions)
    remaining.sort(key=Violation.sort_key)

    report = {
        "schemaVersion": policy["schemaVersion"],
        "workspace": {
            "crates": workspace_names,
            "edges": [{"from": left, "to": right} for left, right in sorted(normal_edges)],
        },
        "numericOwners": policy["numericOwners"],
        "scanned": {
            "crates": len(workspace_names),
            "files": files_scanned,
        },
        "violations": [violation.as_dict() for violation in remaining],
        "waived": waived,
        "exceptions": [exception["id"] for exception in valid_exceptions],
    }
    return (EXIT_OK if not remaining else EXIT_VIOLATION), report


def print_report(report: dict, json_only: bool) -> None:
    if json_only:
        print(json.dumps(report, indent=2, sort_keys=True))
        return
    print(
        "ARCH summary: "
        f"crates={report['scanned']['crates']} "
        f"files={report['scanned']['files']} "
        f"violations={len(report['violations'])} "
        f"waived={len(report['waived'])}"
    )
    owners = ", ".join(
        f"{abstraction}->{owner}" for abstraction, owner in sorted(report["numericOwners"].items())
    )
    print(f"ARCH owners: {owners}")
    for exception in report["exceptions"]:
        print(f"ARCH exception: {exception}")
    for violation in report["violations"]:
        where = violation["path"]
        if violation["line"]:
            where += f":{violation['line']}"
        print(f"VIOLATION {violation['rule']} {where} {violation['message']}")
    for waived in report["waived"]:
        violation = waived["violation"]
        print(
            f"ARCH waived {violation['rule']} {violation['path']} by {waived['exception']} "
            f"({waived['adr']})"
        )


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=None, help="repository root (default: parent of tools/)")
    parser.add_argument("--metadata", default=None, help="recorded cargo-metadata JSON")
    parser.add_argument("--json", action="store_true", help="print the machine-readable report")
    parser.add_argument("--report", default=None, help="also write a Markdown impact report")
    args = parser.parse_args(argv)
    root = Path(args.root).resolve() if args.root else Path(__file__).resolve().parent.parent
    try:
        code, report = run(root, args.metadata)
    except ConfigError as error:
        print(f"ARCH CONFIG ERROR: {error}", file=sys.stderr)
        return EXIT_CONFIG
    print_report(report, args.json)
    if args.report:
        write_markdown(Path(args.report), report)
    return code


def write_markdown(path: Path, report: dict) -> None:
    lines = [
        "# Architecture impact report",
        "",
        f"- Workspace crates: {report['scanned']['crates']}",
        f"- Rust source files scanned: {report['scanned']['files']}",
        f"- Violations: {len(report['violations'])}",
        f"- Waived violations: {len(report['waived'])}",
        "",
        "## Canonical numeric owners",
        "",
        "| Abstraction | Owner |",
        "|---|---|",
    ]
    for abstraction, owner in sorted(report["numericOwners"].items()):
        lines.append(f"| {abstraction} | `{owner}` |")
    lines.extend(["", "## Allowed workspace edges", "", "| From | To |", "|---|---|"])
    for edge in report["workspace"]["edges"]:
        lines.append(f"| `{edge['from']}` | `{edge['to']}` |")
    lines.extend(["", "## Violations", ""])
    if not report["violations"]:
        lines.append("None.")
    for violation in report["violations"]:
        lines.append(
            f"- `{violation['rule']}` at `{violation['path']}:{violation['line']}`: "
            f"{violation['message']}"
        )
    lines.extend(["", "## Exceptions", ""])
    if not report["waived"]:
        lines.append("None.")
    for waived in report["waived"]:
        lines.append(
            f"- `{waived['violation']['rule']}` waived by `{waived['exception']}` "
            f"per `{waived['adr']}`"
        )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
