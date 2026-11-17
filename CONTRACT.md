# Workspace contract

The shared truth for anything built on this workspace: the crate graph, the fixed APIs, the
rendering rules and the corpus. It is deliberately short, and it changes only when a host contract
changes.

Build from the repository root:

    cargo build --workspace
    cargo test --workspace
    cargo run --release -p lovelace-corpus

**No external crates in the engine.** `[dependencies]` may reference workspace crates by path only.
No unsafe, no floating point in any value path. The one exception is the desktop's GUI stack
(`eframe`/`egui` 0.37, `resvg` 0.50, `raw-window-handle` 0.6) in `lovelace-desktop/Cargo.toml`. That
stack is cross-platform and is not scoped to Windows: non-Windows targets add eframe's `glow` backend.
`#![forbid(unsafe_code)]` guards `lovelace-int`, `lovelace-array`, `lovelace-know` and the whole of
`lovelace-desktop` on non-Windows targets (`#![cfg_attr(not(windows), forbid(unsafe_code))]`);
`unsafe` outside `lovelace-desktop/src/win32` is a defect.

## Crates

| Crate | Depends on | Role |
|---|---|---|
| `lovelace-json` | — | JSON with numbers kept as source text |
| `lovelace-nat` | — | arbitrary-precision naturals |
| `lovelace-int` | nat | sign + magnitude integers |
| `lovelace-rational` | nat, int | the one canonical exact rational kernel |
| `lovelace-real` | nat, int, rational | reals with exact periodic decimals; `Rat`↔`Real` projections |
| `lovelace-array` | — | generic `NdArray<T>` + `Field` |
| `lovelace-symbolics` | nat, int, rational | expressions, symbolic algebra, cancellation token (re-exports the canonical `Rat`) |
| `lovelace-systems` | nat, int, rational | exact queueing/operations research: M/M/1, M/M/c, M/M/∞, laws, open Jackson networks, exact MVA, capacity, sweeps; `Q` is the canonical `Rat` alias |
| `lovelace-lang` | all of the above + systems | lexer, parser, interpreter, builtins, the static library families, MathIR |
| `lovelace-run` | json, lang, int, real | the runner and wire envelope |
| `lovelace-know` | json, nat, int, rational | the behavioural graph (`ExactNumber` is a facade over `Rat`) |
| `lovelace-corpus` | json, lang, symbolics | the parity gate and the property corpora |
| `lovelace-bench` | nat, int, real, lang, systems | the micro-benchmark harness |
| `lovelace-desktop` | json, lang†, symbolics†, GUI stack (Windows) | the native IDE; † behind the default `real-engine` feature |

`lovelace-desktop-spike` is a standalone package (`[workspace]` of its own) and not a member.

Exactly one general-purpose rational implementation exists. `lovelace-rational` owns it;
`lovelace-symbolics::Rat`, `lovelace-systems::Q` and `lovelace-know::ExactNumber` are the same type or
facades that own no arithmetic. `docs/architecture-ownership.md` is normative and
`tools/architecture-policy.json` is its machine-readable form; `python tools/check-architecture.py`
verifies the real `cargo metadata` graph and the source declarations, and the `architecture-policy`
CI job is a required check.

## Fixed APIs

### Nat / Int

    Nat::zero(), from_u64(u64), from_decimal(&str)->Option<Nat>, to_decimal()->String,
    is_zero(), is_one(), add, sub(&Nat)->Option<Nat>, mul, div_rem(&Nat)->Option<(Nat,Nat)>,
    pow(u64)->Nat, factorial(u64)->Nat, mul_pow10(usize)->Nat, gcd(&Nat)->Nat, to_u64()->Option<u64>;
    Clone + Ord + Hash + Default.

    Int::zero(), from_i64(i64), from_decimal(&str)->Option<Int>, to_decimal()->String,
    is_zero(), is_negative(), magnitude()->&Nat, negate(), add, sub, mul,
    div_rem(&Nat)->Option<(Int,Int)>, pow(u64)->Int, factorial(u64)->Int;
    Clone + Ord + Hash + Default. Division truncates toward zero; the remainder takes the
    dividend's sign; there is no negative zero.

### Rat (the canonical rational kernel)

    Rat::zero(), one(), from_i64(i64), from_u64(u64), from_nat(&Nat), from_int(&Int),
    from_parts(Int, Nat)->Result<Rat, RationalError>, from_decimal(&str)->Option<Rat>,
    numerator()->&Int, denominator()->&Nat, is_zero/one/negative/positive/integer(),
    to_i64()->Option<i64>, to_u64()->Option<u64>, to_int_exact()->Option<Int>,
    add, sub, mul, neg, abs, div(&Rat)->Option<Rat>, checked_div(&Rat)->Result<Rat, RationalError>,
    pow_i64(i64)->Option<Rat>, checked_pow(i64, &Limits, &dyn Cancel)->Result<Rat, RationalError>,
    add_with/sub_with/mul_with/checked_div_with(..., &Limits), cmp;
    Clone + Debug + PartialEq + Eq + Hash + Ord + Default.
    Canonical form: positive non-zero denominator, sign only in the numerator, zero is 0/1,
    full arbitrary-precision gcd reduction. `Limits::digits`/`bits` bound growth; a budget
    failure is a categorized error, never a truncated success. `RationalError` exposes
    `code()`/`category()` with DomainError, BudgetExceeded or InternalInvariantFailure.

### Real

    Real::zero(), from_i64(i64), from_int(&Int), from_str(&str)->Option<Real>,
    from_fraction(&Nat, &Nat, max_digits)->Option<Real>,   // None on /0; detects the period
    from_rational(&Rat, max_digits)->Conversion<Real>,     // lossless flag + exactness report
    to_string()->String, is_exact()->bool, is_integer()->bool, to_int_exact()->Option<Int>,
    to_rational()->(Int, Nat), to_rat()->Conversion<Rat>,  // reports a truncated input
    add, sub, mul, neg, div(&Self, max_digits)->Option<Real>, cmp,
    pow_int(i64)->Real, sqrt(digits)->Real, pi(digits)->Real,
    abs(), trunc(), floor(), ceil(), frac(); Clone + PartialEq + PartialOrd.

### Array

    NdArray<T> { shape: Vec<usize>, data: Vec<T> }  // row-major
    from_vec(shape, data), scalar(T), zeros(shape), ones(shape), eye(n),
    shape(), rank(), numel(), get(&[usize])->Option<&T>, iter(), as_slice(),
    reshape(Vec<usize>), transpose(), squeeze(), flatten(), concat(&[Self], axis),
    append(&Self, axis), map(f), zip_map(&Self, f), zip_map_broadcast(&Self, f),
    sum(), prod(), min(), max(), sum_axis(axis), prod_axis(axis),
    index_partial(&[usize]), slice(&[Slice]).
    trait Field { zero, one, add, sub, mul, div, neg, is_zero, from_i64 }.

### Engine (the API every host and the corpus gate consume)

    pub struct Engine;
    impl Engine {
        pub fn new() -> Engine;                       // every builtin; no loader, no gating
        pub fn set_precision(&mut self, decimals: usize);
        pub fn precision(&self) -> usize;
        pub fn set_display_precision(&mut self, decimals: usize);
        pub fn eval(&mut self, source: &str) -> Result<Value, LangError>;
        pub fn format_typed(&self, v: &Value) -> String;   // "<display> (<Kind>)"
        pub fn format(&self, v: &Value) -> String;         // display only
        pub fn variables(&self) -> Vec<(String, String)>;  // name, typed display (sorted)
        pub fn functions(&self) -> Vec<(String, Vec<String>)>;
        pub fn take_output(&mut self) -> Vec<String>;      // print() lines since the last call
        pub fn last_plot(&self) -> Option<PlotCapture>;    // path, title, svg
        pub fn set_plot_dir(&mut self, dir: &str);
        pub fn set_cancel_after(&mut self, after: Option<Duration>);
        pub fn set_cancel_flag(&mut self, flag: Option<Arc<AtomicBool>>);
        pub fn revision(&self) -> u64;
    }
    pub struct PlotCapture { pub path: String, pub title: String, pub svg: String }
    pub struct LangError { pub message: String, pub position: usize, pub code: &'static str,
                           pub category: &'static str, ... }
    Value has a `kind_name(&self) -> &str` method.

`position` is a UTF-16 code-unit offset into the caller's source. Cancellation is checked at every
top-level statement boundary and inside long kernels; both produce a `LangError` with code
`Cancelled` and category `BudgetExceeded`.

### Systems exact values (RNL-NUM-011)

Every numeric field in a `sys_*` record is a decimal projection (`Real`). Each such record also
carries an additive `exact_values` record whose fields are `ExactRational` records with a signed
`numerator` (`Integer`) and a positive `denominator` (`Natural`). Example:
`sys_mm1(8, 10).exact_values.utilization` is `ExactRational(numerator: 4, denominator: 5)`.
The exact value is therefore available across the language and wire boundaries; the decimal field
alone is never treated as proof of exact preservation. This is additive: the envelope protocol
version remains 2 and a record consumer ignores fields it does not read. A precision-limited `Real`
is refused as a `sys_*` input (`InvalidArgument`/`TypeMismatch`) rather than promoted to an exact
rational.

## Rendering rules (from the corpus — these are the contract)

- typed rendering is `<display> (<Kind>)`; Kind ∈ Natural, Integer, Real, Boolean, Text, Vector,
  Array, Function, Void, Complex, Symbolic, Record, Domain, Enum.
- Natural/Integer: plain decimal, `-` prefix only when negative.
- Real: the **stored digits only**, and a trailing `.0` is never printed: `3.14`, `0.25`, `2` (for
  `sqrt(4)`), `0.(3)` for a period of one digit, `0.1(6)` for a period after a pre-period. Period
  notation is `preperiod(period)` with the period in parentheses. Truncated irrationals print their
  digits and are NOT marked with an ellipsis in the corpus.
- Boolean: `True` / `False`. Text: raw contents, no quotes.
- Vector/Array: `[a, b, c]` with a comma+space separator, nested exactly by rank.
- Record: `TypeName(field: value, field2: value)`, fields in declaration order, empty list `[]`,
  empty string printed as nothing after the colon+space.
- Errors are plain sentences ending with a period, e.g. `Cannot divide by zero.`,
  `Undefined variable 'nope'.`, `Index 3 is out of range for vector of length 3.`,
  `Operands could not be broadcast together with shapes [2] and [3].`,
  `if condition must be Boolean, but got 'Natural'.`, `'break' is only valid inside a loop.`,
  `Range step must not be zero.`, `Ragged nested list literal: every row must have the same shape.`,
  `Cannot widen from Natural to Text: only numeric kinds (Natural, Integer, Real) support widening.`

## Corpus

`parity/corpus/lang.json` holds 111 records (99 value, 10 error, 1 prints, 1 plot) extracted from the
C# language reference, and `parity/corpus/symbolics.json` holds 103. Each record is
`{id, doc, heading, script, kind, text}`; `kind` ∈ value | error | prints | plot. The gate reads
`<repository root>/parity/corpus/*.json` from `CARGO_MANIFEST_DIR`, so it runs from any working
directory. A missing or unparsable corpus is exit code 2, and at least one divergence is exit code 1;
it is never an empty pass.

## Host contracts

- **Runner** (`lovelace-run`): one JSON document on stdout and nothing else; three exit codes
  (0 success, 1 script/diagnostic failure, 2 usage); `--text` for human output; positions are UTF-16
  code units. The envelope carries `protocolVersion` 2, `mathIrVersion` 2 and
  `symbolicFormatVersion "#!lovelace-sym 1"`.
  **Protocol 2 migration:** `functions[].plugin` (a load family name such as
  `"Lovelace.Symbolics"`) is removed; `functions[].module` is added with the static family label
  (`"Core"`, `"DSP"`, `"Symbolics"`, `"MathIR"`, `"Systems"`). Every other field and every other
  envelope key is unchanged. There is no plugin field anywhere in the v2 wire, and no field enables
  or disables a builtin.
- **Desktop** (`lovelace-desktop`): one active run per document; the run's final state (snapshot and
  incremental cache) settles before the terminal event; events from a stale source revision are
  ignored; a cache hit is reported as reused only after the cached typed value was applied.
- **Fixtures carried with the tests** are listed in [`docs/rustlace-notes.md`](docs/rustlace-notes.md).
  They are inputs to the contracts above, not sample data: deleting one is deleting a check.
