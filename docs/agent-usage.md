# Using RustLace as a consumer agent

This guide is for an agent **using** the Lovelace language and runner to solve a
problem. If you are changing RustLace's source, use [AGENTS.md](../AGENTS.md)
and the [contributor skills](skills.md) instead.

## 1. Pin the executable and understand the version boundary

Work from a specific checked-out commit, or a release whose matching reference
you have read. The checked-in documentation describes the working-tree binary.
Do not assume a released executable has APIs added later. See
[version applicability](version-applicability.md).

Use the same source revision for discovery, execution and verification:

```console
cargo build -p lovelace-run
cargo run -p lovelace-run -- --eval "1/3" --text
```

The CLI returns JSON by default and human text with `--text`; those are
different contracts. For external tooling, use JSON, never scrape the
human-readable line or assume the display string is an exact number.

## 2. Discover all the functions (pagination is not optional)

The registry currently contains more functions than the runner's default
listing limit of 50. Always request a sufficiently high limit and inspect the
`truncated`, `total`, and `returned` JSON fields:

```console
cargo run -p lovelace-run -- --functions --function-limit 500
cargo run -p lovelace-run -- --functions sys --function-limit 500
cargo run -p lovelace-run -- --function matrix_rank
cargo run -p lovelace-run -- --function sys_mm1
```

If `truncated` is true, **do not report discovery as complete**. Increase the
limit or narrow the search and repeat until the requested inventory is complete.
Use `--function <name>` to inspect parameters, arity, module and advertised
return kinds **before** constructing an unfamiliar call. A descriptor provides
discoverability, not a proof that every possible result kind is correct.

The [builtin catalogue](language/builtins.md) gives task-oriented descriptions
and limits beyond the registry descriptor.

## 3. Evaluate with typed and structured results

For stable automation, evaluate a checked-in script through `--file` or a
small inline expression, and request a compact envelope:

```console
cargo run -p lovelace-run -- --eval "matrix_rank([[1,2],[2,4]])" --omit-functions --omit-variables
```

Read `result.kind`, `result.structured`, `code`, `category`,
`diagnostics` and the process exit code. A success exits 0; a script
diagnostic exits 1; invalid CLI usage exits 2. In text mode, failures use
stderr, not the standard JSON envelope. Do not mistake a valid typed
"NoSolutions" result for a thrown diagnostic.

Unquoted strings within `--eval` can be shell-sensitive. For larger scripts
or calls containing quotes, write a `.ls` file and use `--file` instead.
The [reproducible tutorial](tutorials/reproducible.md) demonstrates portable
JSON extraction and plot artifacts.

## 4. Treat numerical provenance as a separate question

- `1/3` prints `0.(3)`: an exact periodic-decimal representation.
- `sqrt(2)` has a finite precision budget and is **precision-limited**, not
  error-bounded merely because its printed digits look convincing.
- `result.structured.exact` is representation/provenance evidence, **not**
  independent mathematical proof of the algorithm's answer.
- The CLI and desktop start with different precision defaults. Explicitly set
  precision when comparing runs or persisting computational evidence.
- When a `sys_*` model publishes `exact_values`, use the rational
  numerator/denominator sidecar as the authoritative exact value instead of
  interpreting the decimal projection as an exact rational.
- Symbolic `matrix_rank` may provide a generic estimate when variables are
  unconstrained: for `[[x]]`, rank depends on whether `x = 0`. Do not
  present a generic symbolic result as an unconditional certificate.

See [precision and formatting](language/precision-and-formatting.md),
[the Systems reference](extensions/systems.md) and
[the agent contract boundaries](agent-harness.md).

## 5. Recover from failures without fabricating success

Inspect the structured `code`, `category`, `message`, and
`signature` when present. Fix the source or input kind, then rerun.
Do not reinterpret a nonzero exit as a valid partial result without examining
the cancellation/diagnostic contract.

If an operation cannot certify its result (for example a domain-dependent
symbolic rank, a precision-limited approximation without an error bound, or an
unimplemented decomposition), say what is unverified and propose an exact
test or alternative. Never replace RustLace's numeric implementation with a
homegrown arithmetic kernel to evade a language limitation.

## 6. Report an auditable result

A completed agent answer should identify the source/executable revision,
input script, invocation, observed exit status, result kind, relevant structured
fields, working precision, and independent check where mathematical correctness
matters. Record the unresolved limitations and avoid calling an example run a
mathematical proof.
