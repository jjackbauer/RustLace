# Round 3 — numerical claims (RQ-002, RQ-003, RQ-018)

**Branch:** `docs/documentation-quality-remediation`
**Commit used for every observation:** `fa87ecc6abe41d3e174d62b211dd6b6fac2e29b7`
**Host:** WSL Ubuntu, `CARGO_TARGET_DIR=/tmp/rl-target`, cargo 1.100.0. The runner was built with
`cargo build -p lovelace-run`; it has no `--version` flag, so the commit is named directly.
**Date:** 2026-11-16.

Files changed by this round (all inside this agent's ownership):

| File | RQ |
|---|---|
| `docs/editorial.md` | RQ-003 (taxonomy, provenance, parity rule) |
| `docs/extensions/dsp.md` | RQ-002, RQ-003, RQ-018 |
| `docs/crates/lovelace-real.md` | RQ-003, RQ-018 |
| `docs/language/README.md` | RQ-018 |
| `docs/language/precision-and-formatting.md` | RQ-003, RQ-018 |
| `docs/language/values-and-types.md` | RQ-003, RQ-018 |
| `docs/language/expressions-and-operators.md` | RQ-018 |
| `docs/language/builtins.md` | RQ-003, RQ-018 |
| `docs/language/lexical-structure.md` | RQ-018 |
| `docs/language/plots.md` | RQ-018 |

`docs/tutorials/**` was not touched (RQ-003 criterion 4 is deferred to the tutorial owner this
round).

---

## 1. RQ-002 verification

### 1.1 Prohibited sentence is gone

```console
$ grep -n "no longer depends on the digit budget" docs/extensions/dsp.md
(no output; exit 1)
```

### 1.2 The "no longer produced by any input" sentence is gone

```console
$ grep -n "no longer produced by any input" docs/extensions/dsp.md
(no output; exit 1)
```

The scoped replacement now reads:

> The old incorrect fixed-width substitution was removed, and the demonstrated rational examples are
> correct and produce the same values at every budget tested here: 30, 37, 38 and 1000 decimal places.
> This is a scoped statement about those examples, not a claim that every `movingavg` result is exact
> at every budget.

and, in the same historical subsection:

> The former incorrect `movingavg([2.5], 3)` result is no longer produced for that input. `0.(8)` is
> the legitimate representation of `8/9` and still renders for `8/9`; it is simply not the
> `movingavg([2.5], 3)` result. `0.(0)` is likewise the representation of zero. Neither historical row
> is an expected result.

### 1.3 Historical location

The wrong values `[0.(8)]` / `[0.(0)]` appear only in the
`### Historical defect (fixed): movingavg below 38 digits` subsection of
`docs/extensions/dsp.md`. The subsection is headed "Historical defect", explicitly says it is "a
historical record, not current behaviour", and labels its columns "Before (historical, wrong)" and
"Now (current)". ADR-004 likewise retains its own correction record; the editorial parity example no
longer repeats the wrong value.

### 1.4 Runtime evidence from `lovelace-run`

Command shape:

```console
$ /tmp/rl-target/debug/lovelace-run --eval "setprecision(<p>); <expr>" --text
```

All three rational examples, at the four budgets:

```text
BUDGET 30 CASE movingavg([2.5], 3)   => [0.8(3)] (Vector)
BUDGET 30 CASE movingavg([0.1], 3)   => [0.0(3)] (Vector)
BUDGET 30 CASE movingavg([1/3], 3)   => [0.(1)] (Vector)
BUDGET 37 CASE movingavg([2.5], 3)   => [0.8(3)] (Vector)
BUDGET 37 CASE movingavg([0.1], 3)   => [0.0(3)] (Vector)
BUDGET 37 CASE movingavg([1/3], 3)   => [0.(1)] (Vector)
BUDGET 38 CASE movingavg([2.5], 3)   => [0.8(3)] (Vector)
BUDGET 38 CASE movingavg([0.1], 3)   => [0.0(3)] (Vector)
BUDGET 38 CASE movingavg([1/3], 3)   => [0.(1)] (Vector)
BUDGET 1000 CASE movingavg([2.5], 3) => [0.8(3)] (Vector)
BUDGET 1000 CASE movingavg([0.1], 3) => [0.0(3)] (Vector)
BUDGET 1000 CASE movingavg([1/3], 3) => [0.(1)] (Vector)
```

The JSON envelopes for all twelve runs carry `"exact":true` (two occurrences each: the value and
the variable echo).

`0.(8)` is reachable as the representation of `8/9`:

```text
$ /tmp/rl-target/debug/lovelace-run --eval "8/9" --text
= 0.(8) (Real)
$ /tmp/rl-target/debug/lovelace-run --eval "0.(8)" --text
= 0.(8) (Real)
$ /tmp/rl-target/debug/lovelace-run --eval "0.(8) == 8/9" --text
= True (Boolean)
```

Long-period truncation (the criterion-3 behaviour) through `movingavg([1], 7)`, which is `1/7`:

```text
$ setprecision(2);  movingavg([1], 7)  => [0.14] (Vector)          inspect(...).exact => False
$ setprecision(6);  movingavg([1], 7)  => [0.142857] (Vector)      inspect(...).exact => False
$ setprecision(7);  movingavg([1], 7)  => [0.(142857)] (Vector)    inspect(...).exact => True
$ setprecision(40); movingavg([1], 7)  => [0.(142857)] (Vector)    inspect(...).exact => True
```

The 6-place run is the important case: the period is six digits and the budget is six, but the
period is *not* discovered because the loop's position test runs before the repeated-remainder test.
The value is a finite truncation marked `inexact`. At 7 places the same call keeps the period.

### 1.5 `Real::from_fraction` citation, checked against the source

`Real::div` calls `fraction_real`, which calls
`Real::from_fraction(num, den, max_digits)` in `lovelace-real/src/lib.rs`. The function's documented
and implemented rules are:

1. A zero remainder is returned immediately as exact (`inexact: false`); a terminating quotient whose
   denominator has only 2 and 5 factors is handled by `terminate_exactly` and is exact at any budget.
2. A non-terminating quotient is expanded by the digit loop and bounded by `max_digits`.
3. If a remainder repeats before the budget is exhausted, `period_start` / `period_len` are recorded
   and the value is a stored periodic decimal.
4. If the budget is exhausted first, `truncated = period_len == 0 && !rem.is_zero()` is true, the
   value is stored as a finite truncation with no period metadata, and `inexact` is set.
5. A period that closes exactly at the cap is never discovered: the `position < max_digits` test is
   evaluated before the repeated-remainder test on the next iteration. The doc comment says this
   explicitly and the 1/7-on-a-6-place-budget run above reproduces it.

The DSP precision rule cites this function by name at `docs/extensions/dsp.md` and describes exactly
these points; no new numeric error bound is asserted.

---

## 2. RQ-003 verification

### 2.1 The four categories in `docs/editorial.md`

```console
$ grep -nE "\*\*Exact\*\*|\*\*Precision-limited\*\*|\*\*Error-bounded\*\*|\*\*Parity-verified\*\*" docs/editorial.md
33:| **Exact** | The value is the mathematically correct result, and the representation holds it without dropping a digit. | ...
34:| **Precision-limited** | The value is a truncation at a stated working precision. The budget says how many digits were computed; it is not a proven final-error bound for an iterative, transcendental or cumulative algorithm. | ...
35:| **Error-bounded** | The value is accompanied by a stated bound on its final error and by the evidence for that bound: a truncation analysis for the algorithm that produced it, interval arithmetic, or an independent oracle. | ...
36:| **Parity-verified** | The value reproduces a named reference implementation, including its defects, and is reported as evidence about that implementation's behaviour. Parity is never mathematical correctness. | ...
exit=0
```

The section retains the old three-way coverage: exactness, the representation-vs-correctness rule,
the `inexact`-is-provenance rule, and the reproduced-defect rule. It adds the split between
precision-limited (a digit budget) and error-bounded (a proved final-error statement with evidence),
and it adds the parity rule. The heading is now "The four kinds of claim"; an HTML anchor named
`the-three-kinds-of-claim` is retained immediately above it so older links in
`docs/reference-template.md` keep resolving.

### 2.2 Working precision is not an error bound

`docs/editorial.md` now says, in the rules:

> A working precision is a budget, not a bound. "30 digits of working precision" does not by itself
> prove a final-error bound of `10^-30` for an arbitrary iterative, transcendental or cumulative
> algorithm; such a claim needs its own analysis or independent evidence.

`docs/extensions/dsp.md` says the same for the DSP 30-place default and for the precision-limited
category. `docs/language/precision-and-formatting.md` repeats it for `setprecision(n)`. No page
claims a `10^-30` final-error bound.

### 2.3 `inexact` is provenance, not an error bound

- `docs/editorial.md`: the rule now reads "An `inexact` marker is provenance, not an error bound: it
  records that a digit was dropped, not how far the stored value is from the true result. It is not a
  correctness certificate either ... and provenance propagates, so an operation on an inexact input
  can produce an exactly representable result that is still flagged inexact."
- `docs/extensions/dsp.md`: the page now states directly that "The `exact` / `inexact` flag is
  provenance: it records whether a digit was dropped, not how far the stored value is from the true
  result, and it is neither an error bound nor a correctness certificate"; the long-period rule says
  the truncated quotient carries "`inexact` provenance"; and the precision-limited category says a
  digit budget is not an error bound.
- `docs/crates/lovelace-real.md`: `Real::is_exact()` "reports provenance ... It is not an error
  bound and not a correctness certificate", and the closing sentence repeats the warning.
- `docs/language/values-and-types.md` and `docs/language/builtins.md` repeat the distinction at the
  language-reference level.

The propagation claim is not inferred from prose: `with_provenance_of2` in
`lovelace-real/src/lib.rs` carries an operand's `inexact` bit through add/sub/mul, and `sqrt` keeps
an inexact input's provenance even for a perfect square. Observed:

```text
$ setprecision(2); x = 1/7; [x, x - x, inspect(x - x).exact]
= [0.14, 0, False] (Vector)
$ setprecision(2); x = 1/7; y = x - x + 4; [y, sqrt(y), inspect(sqrt(y)).exact]
= [4, 2, False] (Vector)
```

### 2.4 Parity is not correctness

`docs/editorial.md` now has:

> Parity with a known-defective historical implementation is never independent proof of mathematical
> correctness. Report the parity check and the independent oracle separately; a parity fixture can
> pin behaviour, but it cannot certify the value.

The same rule appears in the `docs/extensions/dsp.md` parity category and in the
`docs/language/builtins.md` claim table. The historical `movingavg` table is labelled historical and
is not presented as current expected output.

### 2.5 Tutorials (deferred)

`docs/tutorials/**` was not edited. Those pages still use the old three-way wording; the tutorial
owner has that half of criterion 4 this round.

---

## 3. RQ-018 exactness-wording audit

### 3.1 Required greps

```console
$ grep -rn "Numbers are exact\|over exact numeric vectors" docs/language docs/extensions
(no output; exit 1)
```

No unqualified "exact numeric vectors" remains in `docs/extensions/dsp.md`; its opening now states
that the representation is decimal-backed and that several results are precision-limited.

### 3.2 Pages touched and what changed

| Page | Equivalent wording found | Resolution |
|---|---|---|
| `docs/language/README.md` | "Numbers are exact: `Natural`, `Integer`, and `Real`" and "uses exact arithmetic ... no silent rounding" | Rewritten to "Numbers are decimal, not binary floating point" and "uses arbitrary-precision decimal arithmetic"; exactness is called representational and `sqrt(2)` is called precision-limited. |
| `docs/language/precision-and-formatting.md` | "Lovelace numbers are exact until an operation has to truncate" | Rewritten to "use an exact decimal representation until an operation has to truncate", followed by the correctness caveat. Added the computation-precision-is-not-an-error-bound paragraph. |
| `docs/language/values-and-types.md` | "`Natural` and `Integer` are always exact. A `Real` is exact when ..." | Kept the representational definition, prefaced it with "Exactness here is the representation's provenance bit, not a proof of correctness", and added the flag-is-not-an-error-bound paragraph plus the historical-`movingavg` reference. |
| `docs/language/expressions-and-operators.md` | "Division is exact." | Scoped: exact when the expansion terminates or the period fits the budget; otherwise truncated and marked inexact, with the `setprecision(5); 1 / 7` example. |
| `docs/language/builtins.md` | "The numeric builtins preserve exactness where they can"; "`inspect` is the reliable way to ask whether a Real is exact"; "the error is bounded by that budget" | Rewritten to exact representation vs precision-limited value; `inspect` reads the truncation provenance bit; the claim table now has the four categories and says no catalogue entry publishes a final-error bound. |
| `docs/language/lexical-structure.md` | "Decimals are exact decimal values"; "The value is exact and carries period metadata" | Scoped to the literal's representation and added "not an error bound on a computed expression"; "literal is represented exactly". |
| `docs/language/plots.md` | "plotted coordinates are exact model data" | Rewritten as `Real` decimal data with no binary floating point, plus an explicit statement that this is representation, not a proof for every interpolated sample. |
| `docs/extensions/dsp.md` | "Every one is a signal operation over exact numeric vectors"; "carrying exact periodic-decimal metadata"; "roots of unity from exact arithmetic" | Opening now separates decimal representation from mathematical exactness; metadata is "periodic-decimal metadata when its expansion repeats"; kernels use "the engine's decimal arithmetic". |
| `docs/crates/lovelace-real.md` | "A `Real` is exact when ... precision-bounded when it does not" and "`sqrt`/`pi` are precision-bounded" | Rewritten around `is_exact()` provenance, with "precision-limited" for `sqrt`/`pi` and the no-final-error-bound caveat. |

`docs/extensions/mathir.md` and other extension pages were not edited; they were outside this
agent's ownership and their "exact numeric backend/kernels" wording is a representation statement,
not a DSP claim.

---

## 4. Gates and executable examples

```console
$ bash tools/check-docs.sh --claims
check-docs: 57 Markdown files, 170 relative links, 241 code references
check-docs: claims audit: 86 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: OK

$ bash tools/check-docs.sh --readme
check-docs: 57 Markdown files, 170 relative links, 241 code references
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK

$ bash tools/check-docs.sh --symbols
check-docs: 57 Markdown files, 170 relative links, 241 code references, 20 line-suffix citations, 20 symbol citations
check-docs: OK

$ bash tools/check-reference-pages.sh
check-reference-pages: 16 page(s), 112 required sections in order
check-reference-pages: OK
```

The language and extension runners were also re-run because the edited pages contain fenced
examples that must still execute:

```text
docs/language/examples/run-examples.sh: All 193 examples passed. (exit 0)
docs/extensions/examples/run-examples.sh: 27 examples checked; 27 passed; 0 failed; OK. (exit 0)
```

The repaired arithmetic test is still green (no fixed-width substitution was restored):

```text
$ cargo test -p lovelace-lang movingavg
running 9 tests
... 9 passed; 0 failed; 1741 filtered out
```

---

## 5. Per-file changelog (sentence replacements)

### `docs/editorial.md`

- Heading `## The three kinds of claim` → `## The four kinds of claim` (with anchor alias
  `<a id="the-three-kinds-of-claim"></a>` retained for older links).
- "Every numerical statement must say which of these it is." → "Every numerical statement must say
  which of these four it is: **exact**, **precision-limited**, **error-bounded** or
  **parity-verified**."
- Exact row: "The value is the mathematically correct result, exactly representable." → "The value
  is the mathematically correct result, and the representation holds it without dropping a digit."
- Precision-bounded row → replaced by a Precision-limited row: "The value is a truncation at a
  stated working precision. The budget says how many digits were computed; it is not a proven
  final-error bound for an iterative, transcendental or cumulative algorithm." Example changed from
  "`sqrt(2)` at 30 places; `pi`" to the 30-place truncation caveat.
- New Error-bounded row: a stated bound plus the evidence for it (truncation analysis, interval
  arithmetic, or an independent oracle); `ε` is illustrative, not a product bound.
- Parity row → Parity-verified row. Its old example `movingavg([2.5], 3)` → `0.(8)` was removed;
  the row now points to the historical DSP subsection and ADR-004 without repeating the wrong value.
- "It is not a correctness guarantee, and it may not be used as one." → "... it does not make a
  precision-limited result exact and it does not bound the error of an algorithm."
- New rule: a working precision is a budget, not a bound; 30 digits alone do not prove `10^-30`.
- Old `inexact` rule ("An `inexact` marker is provenance ... Do not use it to excuse a wrong leading
  digit.") → expanded rule including "not a correctness certificate", the historical wrong-marked-
  exact case, and provenance propagation.
- New rule: "Parity with a known-defective historical implementation is never independent proof of
  mathematical correctness."
- "A numerical claim needs an independent oracle, a reference calculation, or a stated error
  bound" → "A numerical claim needs an independent oracle, an algorithm-specific truncation
  analysis, or a stated error bound".
- Review checklist: "labelled exact, precision-bounded or parity" → "labelled exact,
  precision-limited, error-bounded or parity-verified".

### `docs/extensions/dsp.md`

- "**16 builtins.** Every one is a signal operation over exact numeric vectors." → the opening
  paragraph about decimal-backed `Real`/`Complex` samples, no binary floating point, and
  precision-limited results.
- "Three different properties are often described as 'exact'..." → "Four different properties ..."
  with exact / precision-limited / error-bounded / parity-verified definitions.
- Old "Exact representation" bullet → "Exact value" bullet, adding "Exact representation is not by
  itself a correctness proof".
- Old "Precision-bounded value" bullet with "the error is bounded by the precision" → "Precision-
  limited value" bullet with the 30-place-budget-not-`10^-30` statement.
- New Error-bounded bullet: no DSP page currently publishes such a bound; a digit budget is not one.
- Old "Reproduced reference behaviour ... the one live case, `movingavg`, now divides exactly" →
  "Parity-verified behaviour": `movingavg` was fixed, no live parity artifact is recorded here, and
  its wrong values survive only in the historical record and ADR-004.
- Old paragraph that reduced the claim to a statement about representation ("It is **not** a
  guarantee that every 30-place result is mathematically correct") → "The decimal representation is a
  property of the values, not a correctness guarantee: it is **not** a proof that every 30-place
  result is mathematically correct".
- Added: "The `exact` / `inexact` flag is provenance: it records whether a digit was dropped, not
  how far the stored value is from the true result, and it is neither an error bound nor a
  correctness certificate."
- "carrying exact periodic-decimal metadata) or an exact `Complex` of two `Real`s" → "carrying
  periodic-decimal metadata when its expansion repeats) or a `Complex` of two `Real`s".
- "build their roots of unity from exact arithmetic" → "build their roots of unity with the engine's
  decimal arithmetic".
- 30-place bullet: appended "This is a working budget, not a final-error bound (see the four claim
  kinds above)."
- Removed the false "38-digit fixed-width conversion bound" bullet (`dsp_lreal128_convertible` no
  longer exists) and replaced it with "No fixed-width substitution remains in `movingavg`",
  describing the deletion of `dsp_lreal128_division_render`.
- "18/37-place working width for periodic operands. The fixed-width tier expands..." → "18/37-place
  working width in the separate fixed-width tier. The `fixed_dsp` tier (tested but not called by any
  builtin; see Limitations) expands ...".
- New "Long periods are truncated, not preserved" bullet: cites `Real::from_fraction`, explains
  budget truncation, no period metadata, `inexact` provenance, the cap-closing rule, and the
  observed `setprecision(2/6/7); movingavg([1], 7)` values.
- Section heading "Fixed: `movingavg` below 38 digits" → "Historical defect (fixed): `movingavg`
  below 38 digits", with the "historical record, not current behaviour" paragraph.
- "divides through the engine's exact `Real::div`" → "divides through the engine's `Real::div`".
- Table header "Before (wrong)" / "Now" → "Before (historical, wrong)" / "Now (current)".
- "The result no longer depends on the digit budget, and the `exact` flag is now correct for all
  three: each is an exact rational." → the scoped paragraph: old substitution removed, examples
  correct and identical at 30/37/38/1000, scoped to those examples, long-period truncation noted.
- "`0.(8)` and `0.(0)` are no longer produced by any input and must not be cited as expected
  results." → the `0.(8)`-is-8/9 / `0.(0)`-is-zero paragraph; only the former `movingavg([2.5], 3)`
  result is said to be gone for that input.
- ADR link `../../docs/adr/004-thirty-digit-default.md` → `../adr/004-thirty-digit-default.md`
  (same target, repository-relative form).

### `docs/crates/lovelace-real.md`

- Whole opening of "Precision behaviour" replaced: "A `Real` is exact when its decimal expansion
  terminates or recurs, and precision-bounded when it does not..." → "`Real::is_exact()` reports
  provenance ... It is not an error bound and not a correctness certificate ... provenance
  propagates ...".
- "`sqrt(digits)` and `pi(digits)` are precision-bounded at the requested count." → "are
  precision-limited at the requested count: the published digits are a truncation, and the digit
  count alone does not establish a final-error bound."
- "An `inexact` marker is provenance, not an error bound (see the editorial guide ...)" kept, anchor
  updated from `#the-three-kinds-of-claim` to `#the-four-kinds-of-claim`.
- Limitation "Exactness depends on the budget." → "The `is_exact` flag depends on the budget.";
  "can be exact at one budget and bounded at another" → "can report exact at one budget and inexact
  at another".

### `docs/language/README.md`

- "Numbers are exact: `Natural`, `Integer`, and `Real` (arbitrary-precision decimal with exact
  repeating-period metadata). They widen ..." → "Numbers are decimal, not binary floating point ...
  Exactness is representational — a computed result can be precision-limited or carry an `inexact`
  provenance flag — and the numerics widen ...".
- "engine ... uses exact arithmetic: there is no floating-point type ... `1 / 3` is the exact
  periodic decimal ... and `sqrt(2)` is computed to the precision you ask for." → "uses
  arbitrary-precision decimal arithmetic ... Exactness is representational: `1 / 3` is the exact
  periodic decimal `0.(3)` ... while `sqrt(2)` is a precision-limited value computed to the
  precision you ask for."

### `docs/language/precision-and-formatting.md`

- Opening "Lovelace numbers are exact until an operation has to truncate..." → "Lovelace numbers
  use an exact decimal representation until an operation has to truncate... That representation is
  not a proof of numerical correctness..."
- Added "A **computation precision** is a budget, not an error bound..." paragraph after the host
  default paragraph.
- "The exactness difference is observable:" → "The provenance difference is observable:
  `inspect(...).exact` records whether a digit was dropped, not whether the value is mathematically
  correct."
- Periodic-rendering paragraph: appended "That flag is provenance, not an error bound. A period that
  closes exactly at the cap is also not discovered: `1/7` ... at `setprecision(6)` ... at
  `setprecision(7)` ...".
- "they compute exactly `n` digits" → "they compute to `n` decimal places".
- "At 7 places the period fits and the value is exact again" → "the value reports exact again".

### `docs/language/values-and-types.md`

- Added, before the definition: "Exactness here is the representation's provenance bit, not a proof
  of correctness."
- Added after the `inspect` example: "The flag records whether a digit was dropped. It is not an
  error bound and it does not certify the value ... provenance propagates ... The editorial guide
  defines the separate claim categories; the historical `movingavg` defect (ADR-004) is the recorded
  case of a wrong value carrying an exact bit."

### `docs/language/expressions-and-operators.md`

- "Division is exact." → "Division is exact when the quotient's decimal expansion terminates or its
  repeating period fits the active computation budget."
- "A non-terminating decimal is stored with its exact period" → "A non-terminating decimal whose
  period fits is stored with its exact period".
- Appended: "If the period does not fit the budget, the quotient is truncated at the budget and
  marked inexact: `setprecision(5); 1 / 7` stores `0.14285`, not the exact `1/7`."

### `docs/language/builtins.md`

- "The numeric builtins preserve exactness where they can and compute at the session's computation
  precision where they cannot." → "preserve the exact representation where the operation has one and
  otherwise compute at the session's computation precision. A truncated result is precision-limited,
  and the `exact` flag is provenance, not an error bound ...".
- "`inspect` is the reliable way to ask whether a Real is exact; the display form does not carry
  that bit." → "`inspect` is the reliable way to read whether a Real's stored digits were truncated;
  the display form does not carry that flag. It is provenance, not an error bound."
- "three claim kinds" → "four claim kinds"; anchor updated to `#the-four-kinds-of-claim`.
- Precision-bounded row ("the error is bounded by that budget") → Precision-limited row ("The budget
  is not a proven final-error bound...").
- New Error-bounded row: no entry publishes a final-error bound.
- Parity row (which called the removed `movingavg` defect "the live case") → Parity-verified row:
  no live parity artifact is documented; the historical record is on the DSP page and ADR-004; the
  arithmetic was changed to the exact quotient rather than preserved for byte-parity.
- "does not make a precision-bounded result exact" → "does not make a precision-limited result
  exact".

### `docs/language/lexical-structure.md`

- "Decimals are exact decimal values, not binary floating point..." → "Decimal literals have an
  exact decimal representation, not a binary floating-point one ... That is a claim about the
  representation; it is not an error bound on a computed expression."
- "The value is exact and carries period metadata" → "The literal is represented exactly and carries
  period metadata".

### `docs/language/plots.md`

- "Coordinates are held as exact `Real` values through ... the plotted coordinates are exact model
  data." → "Coordinates are carried as `Real` values ... no binary floating point enters the model
  path ... That is a statement about the representation, not a proof that every interpolated sample
  is mathematically exact."

---

## 6. Not verified / residuals

- The old defective values were not re-observed by re-running removed code; there is no such code on
  this branch. Their preservation is the labelled historical table plus ADR-004's recorded
  observations, which is exactly the historical status RQ-002 asks for.
- No final-error bound is asserted for `sqrt`, `pi`, `dft`/`fft` roots, or cumulative DSP sums. The
  pages now say a digit budget is not that bound, and no new numeric bound was invented.
- `docs/tutorials/**` still uses the earlier three-way vocabulary (deferred to the tutorial owner).
- `docs/reference-template.md` still says "exact, precision-bounded or parity" and links to the old
  anchor; `docs/contributing.md` still uses the old three-way phrase. Both are outside this agent's
  file ownership. The editorial page keeps the old anchor id so the template's link continues to
  resolve.
- `AGENTS.md`, `README.md`, `.github/**`, `docs/testing.md` and `docs/goal-docs-v2/**` (other than
  this report) were not edited.
