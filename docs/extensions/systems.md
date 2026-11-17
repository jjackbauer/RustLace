# Systems

## Purpose

The systems family is the analytical queueing and operations-research surface: ten builtins backed
by the [`lovelace-systems`](../../lovelace-systems/src/lib.rs) crate. Every number is an exact
rational (`lovelace-systems::Q`, a type-identical alias of the canonical `lovelace_rational::Rat`);
no value crosses as a binary float, and the display precision only chooses how a rational is
rendered as a `Real` projection.

It contributes **10** of the engine's 134 builtins, all in the `Systems` category, and is declared
by ten `module: "Systems"` descriptors in
[`lovelace-lang/src/registry.rs`](../../lovelace-lang/src/registry.rs). The adapter that parses
records and builds results is
[`lovelace-lang/src/systems_family.rs`](../../lovelace-lang/src/systems_family.rs); it holds no
formula.

### What it computes

| Area | Builtins | Model class |
| --- | --- | --- |
| Single station | `sys_mm1`, `sys_mmc`, `sys_delay` | M/M/1, M/M/c (Erlang C), M/M/∞ |
| Network construction | `sys_station`, `sys_open`, `sys_closed` | validated typed models |
| Analysis | `sys_analyze`, `sys_mva` | open Jackson, exact single-class MVA |
| Planning | `sys_bottlenecks`, `sys_sweep` | demand ceilings, population/arrival sweeps |

## Inputs

Arguments are canonical base units, and no conversion is applied silently: time in seconds, rates
in jobs per second, service time in seconds per visit, demand in seconds per job, utilization and
probability dimensionless. A minute is `60` seconds through the crate's `Time::in_unit`; the
language surface accepts the canonical numbers only.

The supported P0 vocabulary is one customer class; stations are FCFS single-server, M/M/c
(`servers >= 2`) or an infinite-server delay center; workloads are open (Poisson arrivals) or
closed (population and think time). A station record is built by `sys_station` and consumed by
`sys_open` / `sys_closed`; the resulting network record is the single input to the analysis
builtins.

## Outputs

`sys_mm1`, `sys_mmc` and `sys_delay` return a `StationAnalysis` record; `sys_analyze` and `sys_mva`
return an `Analysis` record with `stations` (one `StationMetrics` record per station, declaration
order), network metrics, `laws` and provenance. Every record's numeric fields are `Real`
projections, and each carries an additive `exact_values` record whose fields are `ExactRational`
records (`numerator` Integer, `denominator` Natural); see Precision behaviour. `sys_bottlenecks` returns a `Bottlenecks` record and
`sys_sweep` returns a `Sweep` record whose `points` each carry their own `Analysis`. Every record is
read by field name; no consumer parses a display string.

The model-construction records are values too, and they carry the sidecar. `sys_station` returns a
`Station` with `exact_values.service_time`, `exact_values.visits` and
`exact_values.service_demand`; `sys_open` and `sys_closed` return a `QueueNetwork` with
`exact_values.arrival_rate` (open workload) or `exact_values.think_time` (closed workload). The
adapter's `parse_station` and `parse_network` consume the sidecar before the decimal field and fall
back only when the whole sidecar is absent, so a `1/97` service time or think time reaches
`sys_analyze` or `sys_mva` without passing through the decimal projection.

## Precision behaviour

A systems result states three independent things in its provenance record:

- **Model class and assumptions.** For example `closed-mva` with the product-form assumptions, or
  `station-mmc` with Poisson arrivals and exponential service.
- **Exactness.** `exact: True` and `exactness: exact` mean every reported number is an exact
  rational under the declared model. The precision field is only the decimal-rendering budget;
  `1/3` renders as `0.(3)` and is never rounded.
- **Approximation.** `approximation` is empty for every P0 solver. A future approximate solver must
  fill it, and its result is then not an `exact` claim about the model solution.

Exactness is not world-truth: the model's assumptions are a claim about the system being modeled,
and the crate does not test them. A closed-model cycle time excludes think time and is reported
separately.

**Exact values through the language and wire.** A decimal field is a projection; the exact value is
beside it. `sys_mm1(8, 10).exact_values.utilization` is
`ExactRational(numerator: 4, denominator: 5)`, while `sys_mm1(8, 10).utilization` is `0.8`. The
same additive record appears on `Analysis`, `StationMetrics`, `LawCheck`, `PopulationPoint`,
`Bottlenecks`, `BottleneckStation` and `SweepPoint`, and on the model-construction records
`Station` and `QueueNetwork` (service time, visits and service demand; or arrival rate and think
time). The parser treats `exact_values` as authoritative and uses the decimal field only when the
whole sidecar is absent; a present but malformed sidecar is refused rather than falling back. The
wire envelope keeps `protocolVersion` 2:
the new record fields are additive and existing consumers read fields by name. A
precision-limited `Real` is refused as a `sys_*` input (`InvalidArgument`/`TypeMismatch`) instead of
being promoted to an exact rational.

## Errors

Every refusal is a structured `LangError`; the systems crate's code and category cross unchanged.

| Failure | Code / category |
|---|---|
| `lambda >= c*mu` (no stationary distribution) | `UnstableStation` / `DomainError`, naming the station and its offered utilization |
| Unknown identifier, duplicate id, empty network | `InvalidIdentifier` / `DuplicateStationId` / `EmptyNetwork`, `DomainError` |
| A closed model with zero demand and zero think time | `DegenerateModel` / `DomainError` (no arbitrary throughput is invented) |
| M/M/c station in a closed MVA model | `UnsupportedModel` / `UnsupportedOperation`, naming the failing assumption |
| Wrong argument count or kind | `InvalidArgument` / `TypeMismatch`, with the declared signature or the documented `argument <n> expects ...; got ...` sentence |
| A budget (iterations, state space, exact digits) exhausted | `BudgetExceeded` with the specific code, never a truncated successful result |

## Limitations

- Multi-class MVA, routing matrices, multi-server closed MVA, priorities, finite buffers and
  simulation are not in this family; they are refused with `UnsupportedModel`, never approximated
  silently.
- The MVA result is exact for the supported product-form network; it does not predict tail latency,
  and no systems builtin claims a p95/p99 without an appropriate distributional model.
- The crate carries its own unit and golden tests (`cargo test -p lovelace-systems`) plus the
  independent Python oracle (`cargo test -p lovelace-systems --test oracle`), the language family
  pins the `sys_*` records and refusals (`cargo test -p lovelace-lang systems_family`), including
  the `1/97` open and closed construction-to-analysis paths, and
  `parity/corpus/systems.json` runs the documented examples through the corpus gate.

## Examples

```lovelace
sys_mm1(8, 10).utilization
```

```text
= 0.8 (Real)
```

```lovelace
m = sys_closed(2, 9/100, [sys_station("cpu", 1/100, 1)]); sys_mva(m).throughput
```

```text
= 19.(8019) (Real)
```

```lovelace
sys_mmc(8, 5, 2).probability_waiting
```

```text
= 0.7(1) (Real)
```

A network is built from station records and then analyzed; the result is a record, so a script
reads named fields rather than parsing a sentence:

```lovelace
net = sys_open(8, [sys_station("api", 1/20, 1), sys_station("db", 1/50, 2)]); sys_analyze(net).response_time
```

```text
= 0.14(2156862745098039) (Real)
```

### In the desktop IDE

The workbench runs the same engine as the runner, so the calls above need no IDE-specific form.
Start the window, paste a call into the editor, and press **Run** or `Ctrl+Enter`:

```console
$ cargo run -p lovelace-desktop                 # an empty document
$ cargo run -p lovelace-desktop -- systems.ls   # or open a saved script
```

The run's typed result line appears in the Logs pane and the committed values in the workspace pane
(`Ctrl+4` focuses it; `Ctrl+Alt+1` selects its Result view). `F8` jumps to the last failed run's
diagnostic, `Ctrl+S` saves, `Ctrl+O` opens a script, and `F1` lists the whole keyboard map. The
panes and screenshots are on [the desktop crate page](../crates/lovelace-desktop.md).
