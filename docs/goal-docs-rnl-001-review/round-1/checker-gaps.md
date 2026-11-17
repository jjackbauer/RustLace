# Checker gap review — round 1

Repository state tested: `F:\dev\RustLace`, commit `8ec1aaf` (`ci: let the architecture checker resolve
metadata on a cold runner`), Python 3.12.10, Windows host.

Method: each claim was tested in its own copy of `tools/architecture-fixtures/base` under
`.tmp/review-rnl-001/<name>/`, from the repository root, with the exact command

```console
python tools/check-architecture.py --root .tmp/review-rnl-001/<name> --metadata .tmp/review-rnl-001/<name>/metadata.json
```

The exit code and the diagnostic lines actually printed are recorded below; the console transcript of
the ten checker runs is `.tmp/review-rnl-001/transcript.txt`. A control fixture isolates the syntax shape that the claim says
causes the bypass. No tracked file was edited by this review: the counterexample trees are under the
git-ignored `.tmp/`, and this page is the only deliverable. (A probe import rewrote the tracked `.pyc`;
`git checkout -- tools/__pycache__/check-architecture.cpython-312.pyc` restored it.) Another agent shares
this worktree and edited it after the runs below: `tools/__pycache__/check-architecture.cpython-312.pyc`
was staged for deletion and `.gitignore` gained Python-cache rules. Those later edits are not part of
this review; every verdict is against `HEAD` `8ec1aaf` and the fixture runs, and the claim-7 facts below
are re-checkable from `HEAD` with `git ls-tree` / `git show`.

## Verdicts

| # | Claim | Verdict | Evidence (command + observed output / file:line) | Counterexample attempted | Reason |
|---|---|---|---|---|---|
| 1 | Whole-crate exemption: `duplicateDetection.ownerCrates` includes `lovelace-lang`; therefore adding `pub struct Rat { num: Int, den: Nat }` with arithmetic methods to `lovelace-lang/src/lib.rs` in the fixture is not rejected. | Supported | `python tools/check-architecture.py --root .tmp/review-rnl-001/claim1-owner-skip --metadata .tmp/review-rnl-001/claim1-owner-skip/metadata.json` → exit 0, `ARCH summary: crates=6 files=6 violations=0 waived=0` (copy `ownerCrates` set to the real six-crate list). Unpatched fixture: `python tools/check-architecture.py --root .tmp/review-rnl-001/claim1-unpatched --metadata .tmp/review-rnl-001/claim1-unpatched/metadata.json` → exit 1, `violations=3`, `duplicate-numeric-type` @ `lovelace-lang/src/lib.rs:10`, `duplicate-numeric-representation` @ `:10`, `duplicate-arithmetic-impl` @ `:12`. `python .tmp/review-rnl-001/owner_probe.py` → `real: ownerCrates=[... 'lovelace-lang'] violations=0`; `fixture: ... violations=3`. Real list at `tools/architecture-policy.json:225`. | Appended `use lovelace_int::Int; use lovelace_nat::Nat;` and `pub struct Rat { num: Int, den: Nat }` plus a four-method `impl Rat` to `lovelace-lang/src/lib.rs`; replaced only `duplicateDetection.ownerCrates` in the copy's policy with the real policy's list. | `check_duplicates` skips an owner crate before scanning it: `for crate, crate_declarations in declarations.items(): / if not crate_declarations or crate in owners: / continue` (`tools/check-architecture.py:764-766`). `lovelace-lang` is in the real policy's `ownerCrates` (`tools/architecture-policy.json:219-226`), so no declaration, name, field-pair or impl rule runs for it. The fixture's reduced policy omits `lovelace-lang` from that list (`tools/architecture-fixtures/base/tools/architecture-policy.json:87-91`), which is why the shipped fixture rejects the same file; the claim names the real policy, and both the patched copy and the direct probe with the real policy produce zero violations. |
| 2 | Split impl blocks: a duplicate scalar type whose arithmetic methods are split across two `impl` blocks, each with fewer than `minimumArithmeticMethods` (4), evades `duplicate-arithmetic-impl`. | Supported | `python tools/check-architecture.py --root .tmp/review-rnl-001/claim2-split-impls --metadata .tmp/review-rnl-001/claim2-split-impls/metadata.json` → exit 1, `violations=1`; the only diagnostic is `VIOLATION duplicate-numeric-representation lovelace-systems/src/lib.rs:21 crate 'lovelace-systems' declares 'Pair' with a numerator/denominator-like numeric representation`. No `duplicate-arithmetic-impl`; no diagnostic for `Scalar`. Control `python tools/check-architecture.py --root .tmp/review-rnl-001/claim2-control --metadata .tmp/review-rnl-001/claim2-control/metadata.json` → exit 1, `violations=2`: `duplicate-numeric-representation` @ `:21` and `duplicate-arithmetic-impl` @ `:23` (`implements 4 scalar arithmetic methods on 'Scalar'`). `python .tmp/review-rnl-001/parse_probe.py` → `impl type_name='Pair' trait_name=None methods=['add', 'sub']` and `impl type_name='Pair' trait_name=None methods=['mul', 'div']`; `Scalar` shows the same split. | Appended `Pair { num: Int, den: Nat }` with 2+2 methods and `Scalar { top: Int, bottom: Nat }` with 2+2 methods to `lovelace-systems/src/lib.rs`; the control puts all four `Scalar` methods in one impl. | `method_counts` keeps the per-impl maximum: `method_counts[impl.type_name] = max(method_counts.get(impl.type_name, 0), count)` (`tools/check-architecture.py:770-774`). The per-impl rule fires only when `count >= minimum` (`:826-831`), and `numeric_field_match` requires `arithmetic_method_count >= minimum` for the typed-field path (`:736-739`). Two impls of two methods never reach 4, so neither check fires for `Scalar` (no diagnostic at all) and `duplicate-arithmetic-impl` never fires for `Pair`. `Pair` is still rejected, but by the conclusive `num`/`den` pair rule (`:733-735`), not by the split method count; the split-only bypass is therefore real for a renamed shape and rule-specific for a named-pair shape. |
| 3 | Generic trait impl: `impl<T> std::ops::Add for Wrapper<T>` (or `impl<T> Add for Wrapper<T>`) evades `duplicate-arithmetic-trait`, e.g. because the parser identifies `T` or the generic parameter instead of `Wrapper`. | Supported | `python tools/check-architecture.py --root .tmp/review-rnl-001/claim3-generic-trait --metadata .tmp/review-rnl-001/claim3-generic-trait/metadata.json` → exit 0, `violations=0`. Control `python tools/check-architecture.py --root .tmp/review-rnl-001/claim3-control --metadata .tmp/review-rnl-001/claim3-control/metadata.json` → exit 1, `VIOLATION duplicate-arithmetic-trait lovelace-systems/src/lib.rs:21 crate 'lovelace-systems' implements arithmetic trait Add for 'Wrapper' outside the canonical owners`. `python .tmp/review-rnl-001/parse_probe.py` → `impl type_name='T' trait_name='' methods=['add']` for both generic forms. | Appended `struct Wrapper<T>(pub T)` + `impl<T> std::ops::Add for Wrapper<T>` and `struct Holder<T>(pub T)` + `impl<T> Add for Holder<T>` to `lovelace-systems/src/lib.rs`; the control uses non-generic `impl std::ops::Add for Wrapper` with a `Q` field. | `split_impl_header` returns `type_part = "Wrapper<T>"` (`tools/check-architecture.py:530-552`); `type_name_of` returns the last identifier, `T` (`:555-557`); `trait_name_of` truncates at `<`, so `"<T> std::ops::Add"` yields `""` (`:560-563`). `check_duplicates` then takes `if impl.type_name not in by_name: continue` (`:800-802`), so the impl is never compared with `arithmeticTraits`. The non-generic control does fire, which isolates the generic header as the cause. |
| 4 | Free-function kernel: a duplicate rational representation with free functions `fn rat_add(...)`, `fn rat_mul(...)`, `fn rat_div(...)`, `fn rat_gcd(...)` and no inherent methods is not rejected. | Supported | `python tools/check-architecture.py --root .tmp/review-rnl-001/claim4-free-functions --metadata .tmp/review-rnl-001/claim4-free-functions/metadata.json` → exit 0, `ARCH summary: crates=6 files=6 violations=0 waived=0`. | Appended `struct Ratio { top: Int, bottom: Nat }` and four free functions `rat_add`, `rat_mul`, `rat_div`, `rat_gcd` to `lovelace-systems/src/lib.rs`; `Ratio` has no `impl` block. | `method_names` is called only on an `impl` body: `methods = method_names(body)` (`tools/check-architecture.py:666`), so free functions are never collected as methods. `method_counts` for `Ratio` is therefore absent, and the typed-field path requires `arithmetic_method_count >= 4` (`:736-739`); `Ratio`, `top` and `bottom` are not in `forbiddenTypeNames` or `numericFieldPairs`, so nothing fires. A type literally named `Rat` or carrying `num`/`den` would be caught by the other two rules, but this representation is not. |
| 5 | Renamed representation: a type not in `forbiddenTypeNames` with field names not in `numericFieldPairs` and arithmetic methods split across impls passes. | Supported | `python tools/check-architecture.py --root .tmp/review-rnl-001/claim5-renamed --metadata .tmp/review-rnl-001/claim5-renamed/metadata.json` → exit 0, `ARCH summary: crates=6 files=6 violations=0 waived=0`. | Appended `struct Quantity { left: Int, right: Nat }` with two impls, `add`/`sub` and `mul`/`div`, to `lovelace-systems/src/lib.rs`. | `Quantity` is not in `forbiddenTypeNames`; `left`/`right` is not a `numericFieldPairs` entry; each impl has 2 arithmetic methods (< 4), so neither the per-impl rule (`tools/check-architecture.py:826-831`) nor the typed-field path (`:736-739`) can fire. The declaration therefore produces no violation. |
| 6 | Exception breadth: an exception with `scope: ["lovelace-*"]`, a valid ADR path, reviewer, rationale and removal plan is accepted (the checker only rejects literal `*`). | Supported | `python tools/check-architecture.py --root .tmp/review-rnl-001/claim6-exception --metadata .tmp/review-rnl-001/claim6-exception/metadata.json` → exit 0, `violations=0 waived=2`; `ARCH exception: EX-BROAD-1`; `ARCH waived duplicate-numeric-type lovelace-symbolics/src/lib.rs by EX-BROAD-1 (docs/adr/999-exception-scope-probe.md)` and the same for `lovelace-systems/src/lib.rs`. Control `python tools/check-architecture.py --root .tmp/review-rnl-001/claim6-literal-star --metadata .tmp/review-rnl-001/claim6-literal-star/metadata.json` → exit 1, `violations=3`, including `VIOLATION unauthorized-exception ... scope must be a non-empty list of narrow paths/crates (not '*')`. | Added `pub struct Rat;` to `lovelace-systems/src/lib.rs` and `pub struct Fraction;` to `lovelace-symbolics/src/lib.rs`; created `docs/adr/999-exception-scope-probe.md` in the copy; installed one exception with `id`, `rule: "duplicate-numeric-type"`, `adr`, `reviewer`, `rationale`, `removalPlan` and `scope: ["lovelace-*"]`. The control keeps the same exception with `scope: ["*"]`. | `validate_exceptions` rejects only a literal `"*"` member: `if not isinstance(scope, list) or not scope or "*" in scope:` (`tools/check-architecture.py:861-863`); it performs no test that the pattern is narrow or crate-specific. `waive_violations` then `fnmatch`es each scope pattern against the path, the crate name, and `crate/path` (`:898-903`). `lovelace-*` matches every crate in the workspace, so the single exception waived two violations in two different crates and the gate exited 0. |
| 7 | Repository hygiene: `tools/__pycache__/check-architecture.cpython-312.pyc` is tracked by git and `.gitignore` has no Python cache rule. | Supported | `git ls-tree -r -l HEAD tools/__pycache__` → `100644 blob e0bd96c314e24b4410bf2b85dd93efd3d51f7878   50074	tools/__pycache__/check-architecture.cpython-312.pyc` (exit 0); `git cat-file -e HEAD:tools/__pycache__/check-architecture.cpython-312.pyc` → exit 0. At run time `git ls-files -- tools/__pycache__` printed the same path and `git check-ignore -v` printed nothing with exit 1. `git show HEAD:.gitignore` has no `__pycache__` match and no `.pyc` match. `git log -1 --format='%h %s' -- tools/__pycache__` → `32ea6cd feat!: one shared rational kernel and enforced architecture policy`. | Direct repository-state query; the counterexample would be an ignore rule or an untracked file, and neither exists in `HEAD`. | The blob is present in `HEAD` with mode `100644` and size 50074 bytes, so it is tracked, not merely on disk. The `HEAD` `.gitignore` has no `__pycache__/` or `*.pyc` rule; at run time `git check-ignore` returned 1 for the worktree path as well. A later worktree edit by another agent (post-HEAD) added `__pycache__/` and `*.py[cod]` rules and staged the `.pyc` deletion; that change is not part of this verdict. |

**Counts: Supported 7, Falsified 0, NotCheckable 0** (claims 1-7).

## Checker source grounding

### Owner-crate skip — `tools/check-architecture.py:764-766`

```python
    for crate, crate_declarations in declarations.items():
        if not crate_declarations or crate in owners:
            continue
```

`owners` is `set(detection["ownerCrates"])` (`:749`). The skip encloses both the declaration loop
(`:776`) and the impl loop (`:800`), so every duplicate rule is bypassed for an owner crate.

### `split_impl_header` and impl identity — `tools/check-architecture.py:530-563`

```python
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
```

The impl analyzer stores the result at `:663` and calls `method_names` at `:666`. The generic header
`<T> std::ops::Add for Wrapper<T>` splits into trait `<T> std::ops::Add` and type `Wrapper<T>`;
`type_name_of` returns `T`, `trait_name_of` returns the empty string, and `check_duplicates` then skips
the impl at `:800-802`:

```python
        for impl in crate_impls:
            if impl.type_name not in by_name:
                continue
```

### Method counting — `tools/check-architecture.py:770-774` and `:826-831`

```python
        method_counts: dict[str, int] = {}
        for impl in crate_impls:
            if impl.type_name in by_name:
                count = len(set(impl.methods) & arithmetic_methods)
                method_counts[impl.type_name] = max(method_counts.get(impl.type_name, 0), count)
```

```python
            count = len(set(impl.methods) & arithmetic_methods)
            declaration = by_name[impl.type_name]
            if count >= minimum and (
                numeric_field_match(declaration, detection, count)
                or declaration.name in forbidden_names
            ):
```

`method_counts` is a maximum, not a sum, over impl blocks. `method_names` (`:566-586`) only records
`fn` items at brace depth 0 inside the body passed to it; the only call site is the impl body (`:666`),
so free functions are invisible.

### Numeric representation predicate — `tools/check-architecture.py:720-739`

```python
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
```

A `num`/`den` (or other configured) pair is conclusive. Arbitrary field names need at least two numeric
field types **and** `minimumArithmeticMethods` counted methods, which is why a split of 2+2 leaves the
per-type maximum at 2, below the required 4.

### Exception validation and waiver — `tools/check-architecture.py:861-874`, `:898-903`

```python
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
```

```python
            if any(
                fnmatch.fnmatch(violation.path, pattern)
                or fnmatch.fnmatch(violation.crate, pattern)
                or fnmatch.fnmatch(f"{violation.crate}/{violation.path}", pattern)
                for pattern in exception["scope"]
            ):
```

The only wildcard rejection is membership test against the literal string `"*"`. `lovelace-*` is accepted,
and `fnmatch` against the crate name makes it match every workspace crate.

## Counterexample sources (under `.tmp/review-rnl-001/`)

The fixture is a text-scanning fixture with a recorded `metadata.json`; it is not compiled, so the
name collision between the fixture's `use lovelace_rational::Rat` and the appended `struct Rat` (claim 1)
does not affect the checker's scan.

**claim1-owner-skip / claim1-unpatched** — appended to `lovelace-lang/src/lib.rs`:

```rust
use lovelace_int::Int;
use lovelace_nat::Nat;

pub struct Rat { num: Int, den: Nat }

impl Rat {
    pub fn add(&self, other: &Rat) -> Rat { other.clone() }
    pub fn sub(&self, other: &Rat) -> Rat { other.clone() }
    pub fn mul(&self, other: &Rat) -> Rat { other.clone() }
    pub fn div(&self, other: &Rat) -> Rat { other.clone() }
}
```

`claim1-owner-skip/tools/architecture-policy.json` differs from the base fixture only in
`duplicateDetection.ownerCrates`, which was set to the real policy's six-crate list.

**claim2-split-impls / claim2-control** — appended to `lovelace-systems/src/lib.rs`:

```rust
pub struct Pair { num: Int, den: Nat }

impl Pair {
    pub fn add(&self, other: &Pair) -> Pair { other.clone() }
    pub fn sub(&self, other: &Pair) -> Pair { other.clone() }
}

impl Pair {
    pub fn mul(&self, other: &Pair) -> Pair { other.clone() }
    pub fn div(&self, other: &Pair) -> Pair { other.clone() }
}

pub struct Scalar { top: Int, bottom: Nat }

impl Scalar {
    pub fn add(&self, other: &Scalar) -> Scalar { other.clone() }
    pub fn sub(&self, other: &Scalar) -> Scalar { other.clone() }
}

impl Scalar {
    pub fn mul(&self, other: &Scalar) -> Scalar { other.clone() }
    pub fn div(&self, other: &Scalar) -> Scalar { other.clone() }
}
```

The control contains only `Scalar`, with `add`, `sub`, `mul`, `div` in one `impl` block.

**claim3-generic-trait / claim3-control** — appended to `lovelace-systems/src/lib.rs`:

```rust
pub struct Wrapper<T>(pub T);

impl<T> std::ops::Add for Wrapper<T> {
    type Output = Wrapper<T>;
    fn add(self, other: Self) -> Self { other }
}

pub struct Holder<T>(pub T);

impl<T> Add for Holder<T> {
    type Output = Holder<T>;
    fn add(self, other: Self) -> Self { other }
}
```

The control has `pub struct Wrapper(pub Q);` and `impl std::ops::Add for Wrapper`.

**claim4-free-functions** — appended to `lovelace-systems/src/lib.rs`:

```rust
pub struct Ratio { top: Int, bottom: Nat }

pub fn rat_add(left: &Ratio, right: &Ratio) -> Ratio { right.clone() }
pub fn rat_mul(left: &Ratio, right: &Ratio) -> Ratio { right.clone() }
pub fn rat_div(left: &Ratio, right: &Ratio) -> Ratio { right.clone() }
pub fn rat_gcd(left: &Ratio, right: &Ratio) -> Ratio { right.clone() }
```

**claim5-renamed** — appended to `lovelace-systems/src/lib.rs`:

```rust
pub struct Quantity { left: Int, right: Nat }

impl Quantity {
    pub fn add(&self, other: &Quantity) -> Quantity { other.clone() }
    pub fn sub(&self, other: &Quantity) -> Quantity { other.clone() }
}

impl Quantity {
    pub fn mul(&self, other: &Quantity) -> Quantity { other.clone() }
    pub fn div(&self, other: &Quantity) -> Quantity { other.clone() }
}
```

**claim6-exception / claim6-literal-star** — `pub struct Rat;` appended to `lovelace-systems/src/lib.rs`,
`pub struct Fraction;` appended to `lovelace-symbolics/src/lib.rs`, and
`docs/adr/999-exception-scope-probe.md` created. The exception installed in the copy's policy is:

```json
{
  "id": "EX-BROAD-1",
  "rule": "duplicate-numeric-type",
  "adr": "docs/adr/999-exception-scope-probe.md",
  "rationale": "Temporary probe of exception scope matching in the review fixture.",
  "reviewer": "review-rnl-001",
  "removalPlan": "Remove this exception and the probe types after review.",
  "scope": ["lovelace-*"]
}
```

The control uses the same object with `"scope": ["*"]`.
