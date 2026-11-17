# Systems

## Purpose

The systems family is the analytical queueing and operations-research surface: ten builtins backed
by the [`lovelace-systems`](../../lovelace-systems/src/lib.rs) crate. Every number is an exact
rational (`lovelace-systems::Q`, a type-identical alias of the canonical `lovelace_rational::Rat`);
no value crosses as a binary float, and the display precision only chooses how a rational is
rendered as a `Real` projection.

The ten builtins are ordinary Lovelace calls: they work in the desktop workbench, in `lovelace-run`
and in every script. Each one is documented below in [Per-builtin reference](#per-builtin-reference);
the long-form account of the models and solvers is [the systems analysis guide](../systems-analysis.md).

### Run it in the IDE or the CLI

In the desktop workbench, open the window, type or paste a call into the editor and press **Run**
(or `Ctrl+Enter`); results appear in the Logs pane and the workspace:

```console
$ cargo run -p lovelace-desktop                 # an empty document
$ cargo run -p lovelace-desktop -- examples/systems.ls
```

From the command line, evaluate a call directly or run the checked-in example script:

```console
$ cargo run -p lovelace-run -- --eval "sys_mm1(8, 10).utilization" --text
= 0.8 (Real)
$ cargo run -p lovelace-run -- examples/systems.ls --text --omit-variables
= 20 (Real)
```

Discovery reads the same registry the editor completes from: `cargo run -p lovelace-run --
--functions sys` lists the `Systems` builtins, and `cargo run -p lovelace-run -- --function
sys_mm1` prints one builtin's declared parameters, return kind, summary and examples.

**Version note.** The `exact_values` member on these records, and the exact model transport behind
it (a `1/97` service time surviving `sys_station` → `sys_open` → `sys_analyze`), were added after
v0.7.1. On the v0.7.1 desktop, `sys_mm1(8, 10).exact_values.utilization` fails with
`record 'StationAnalysis' has no member 'exact_values'`; the builtins themselves and the examples
that do not read `exact_values` are unaffected. Use a build of this tree until a release contains
the change.

It contributes **10** of the engine's 134 builtins, all in the `Systems` category, and is declared
by ten `module: "Systems"` descriptors in
[`lovelace-lang/src/registry.rs`](../../lovelace-lang/src/registry.rs). The adapter that parses
records and builds results is
[`lovelace-lang/src/systems_family.rs`](../../lovelace-lang/src/systems_family.rs); it holds no
formula.

### Areas

| Area | Builtins | Model class |
| --- | --- | --- |
| Single station | `sys_mm1`, `sys_mmc`, `sys_delay` | M/M/1, M/M/c (Erlang C), M/M/∞ |
| Network construction | `sys_station`, `sys_open`, `sys_closed` | validated typed models |
| Analysis | `sys_analyze`, `sys_mva` | open Jackson, exact single-class MVA |
| Planning | `sys_bottlenecks`, `sys_sweep` | demand ceilings, population/arrival sweeps |

## Inputs

Arguments are canonical base units, and no conversion is applied silently: time in seconds, rates
in jobs per second, service time in seconds per visit, demand in seconds per job, utilization and
probability dimensionless. A minute is `60` seconds; the language surface accepts the canonical
numbers only.

Number arguments are **exact**: a `Natural`, an `Integer`, or a `Real` whose stored digits are the
exact value (a fraction such as `1/20` or a terminating decimal such as `0.05`). A
precision-limited `Real` — one whose rendering had to drop digits — is refused with
`InvalidArgument`/`TypeMismatch` rather than silently promoted to an exact rational.

| Builtin | Arity | Arguments |
| --- | --- | --- |
| `sys_mm1` | 2 | `lambda`, `mu` — arrival and per-server service rate |
| `sys_mmc` | 3 | `lambda`, `mu`, `servers` — M/M/c, `servers >= 2` |
| `sys_delay` | 2 | `lambda`, `mu` — M/M/∞ (a delay center) |
| `sys_station` | 3–4 | `name`, `service_time`, `visits` [, `servers` or `"delay"`] |
| `sys_open` | 2 | `arrival_rate`, `stations` (a vector of station records) |
| `sys_closed` | 3 | `population`, `think_time`, `stations` |
| `sys_analyze` | 1–2 | `network` [, `precision`] |
| `sys_mva` | 1–2 | `network` [, `precision`]; the network must be closed |
| `sys_bottlenecks` | 1 | `network` |
| `sys_sweep` | 4 | `network`, `from`, `to`, `step` |

The supported P0 vocabulary is one customer class; stations are FCFS single-server, M/M/c
(`servers >= 2`) or an infinite-server delay center; workloads are open (Poisson arrivals) or
closed (population and think time). A station record is built by `sys_station` and consumed by
`sys_open` / `sys_closed`; the resulting network record is the single input to the analysis
builtins.

A station identifier is 1 to 64 bytes of ASCII letters, digits, `_`, `.` or `-` and is unique within
a network. A network needs at least one station. A station that carries traffic (positive visits)
needs a positive service time; a station with zero visits may have a zero service time, because the
model is then defined and empty. A negative rate, visit ratio, service time or think time is
refused, never clamped.

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

| Record | Fields |
| --- | --- |
| `Station` | `id`, `discipline`, `service_time`, `visits`, `service_demand`, `servers` (finite only), `exact_values` |
| `QueueNetwork` | `name`, `workload`, `stations`, `precision`, `arrival_rate` (open) or `population` + `think_time` (closed), `exact_values` |
| `StationAnalysis` | provenance fields, `laws`, one station's metrics |
| `Analysis` | provenance fields, `throughput`, `response_time`, `occupancy`, `queue_length`, `population` + `think_time` (closed), `exact_values`, `laws`, `stations` |
| `StationMetrics` | `id`, `discipline`, `visits`, `service_time`, `service_demand`, `throughput`, `utilization`, `busy_servers`, `response_time`, `waiting_time`, `occupancy`, `queue_length`, `servers` (finite only), `probability_waiting` (where defined), `exact_values` |
| `LawCheck` | `name`, `left`, `right`, `holds`, `exact_values` |
| `Bottlenecks` | `basis`, `max_demand`, `stations` (`BottleneckStation`), `warnings`, `throughput_bound` (a void value when unbounded), `exact_values` |
| `Sweep` | `axis`, `points` (`SweepPoint`), `warnings` |
| `SweepPoint` | `parameter`, `throughput`, `response_time`, `max_utilization`, `exact_values`, `analysis` |

The provenance fields on `StationAnalysis` and `Analysis` are `model` (`station-mm1`,
`station-mmc`, `station-mminf`, `open-jackson`, `closed-mva`), `solver`, `solver_version`, `exact`
(Boolean), `exactness`, `precision`, `status` (`solved` or `degenerate`), `assumptions`,
`approximation` (a void value for every solver in this family) and `warnings` (a vector of text).

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

`sys_analyze` and `sys_mva` accept an optional `precision` argument: an integer from 1 to 100 000
decimal places. It defaults to the value recorded on the network (30 for every network these
builtins construct). The precision changes only how a `Q` is rendered into the decimal fields; it is
not an iteration tolerance and not an error bound. A value whose decimal period fits the budget
renders with its period; one that does not is truncated, and the exact pair in `exact_values` still
holds the full rational.

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
Argument refusals use the documented sentence
`<builtin>(): argument <n> expects <kind>; got <kind>.`

| Failure | Code / category |
|---|---|
| `lambda >= c*mu` (no stationary distribution) | `UnstableStation` / `DomainError`, naming the station and its offered utilization |
| Unknown identifier, duplicate id, empty network | `InvalidIdentifier` / `DuplicateStationId` / `EmptyNetwork`, `DomainError` |
| A negative rate, visit ratio, service time or think time | `InvalidArrivalRate` / `InvalidVisits` / `InvalidServiceTime` / `DegenerateModel`, `DomainError` |
| A closed model with zero demand and zero think time | `DegenerateModel` / `DomainError` (no arbitrary throughput is invented) |
| M/M/c station in a closed MVA model | `UnsupportedModel` / `UnsupportedOperation`, naming the failing assumption |
| An M/M/c server count outside `2..=1_000_000` | `UnsupportedDiscipline` / `UnsupportedOperation` |
| Wrong argument count or kind | `InvalidArgument` / `TypeMismatch`, with the declared signature or the documented `argument <n> expects ...; got ...` sentence |
| A sweep endpoint not on the exact step grid | `SweepEndpointNotOnGrid` / `DomainError` |
| A budget (iterations, state space, exact digits) exhausted | `BudgetExceeded` with the specific code, never a truncated successful result |

A precision-limited `Real` argument is the one input case that is a plain `InvalidArgument`: it is
refused rather than converted, because a systems parameter must be exact.

## Limitations

- Multi-class MVA, routing matrices, multi-server closed MVA, priorities, finite buffers and
  simulation are not in this family; they are refused with `UnsupportedModel`, never approximated
  silently.
- The MVA result is exact for the supported product-form network; it does not predict tail latency,
  and no systems builtin claims a p95/p99 without an appropriate distributional model.
- A sweep is bounded: at most 10 000 points, and the endpoint must be exactly on the step grid.
- A bottleneck bound is a ceiling as the population grows, not a throughput achieved at finite
  population; the report says so in its warnings.
- The crate carries its own unit and golden tests (`cargo test -p lovelace-systems`) plus the
  independent Python oracle (`cargo test -p lovelace-systems --test oracle`), the language family
  pins the `sys_*` records and refusals (`cargo test -p lovelace-lang systems_family`), including
  the `1/97` open and closed construction-to-analysis paths, and
  `parity/corpus/systems.json` runs the documented examples through the corpus gate.

## Examples

Every example below is a complete call with the output the runner prints for it
(`cargo run -p lovelace-run -- <script> --text --omit-variables`), so it can be pasted into the
editor as is. Each builtin's block gives its arguments, return record, the fields it defines, its precision
behaviour and the refusals it can produce.

### Per-builtin reference

#### sys_mm1

`sys_mm1(lambda, mu)` — M/M/1 stationary metrics from an arrival rate and a per-server service rate.

| Argument | Accepted kinds | Constraint |
| --- | --- | --- |
| `lambda` | exact number | non-negative; `0` is an empty stream |
| `mu` | exact number | positive |

Returns a `StationAnalysis` record: the provenance fields, `laws`, and the station's metrics. The
metrics are `rho = lambda * S`, `R = S / (1 - rho)`, `W = rho * R`, `L = lambda * R`,
`Lq = lambda * W`, with `S = 1/mu`. `probability_waiting` is absent for M/M/1.

Errors: `InvalidServiceRate` when `mu` is not positive; `InvalidArrivalRate` for a negative
`lambda`; `UnstableStation` when `lambda >= mu`; `InvalidArgument`/`TypeMismatch` for a non-number
or a precision-limited `Real`.

```lovelace
sys_mm1(8, 10).utilization
```

```text
= 0.8 (Real)
```

```lovelace
sys_mm1(8, 10).exact_values.utilization
```

```text
= ExactRational(numerator: 4, denominator: 5) (ExactRational)
```

#### sys_mmc

`sys_mmc(lambda, mu, servers)` — M/M/c metrics through the Erlang C probability of waiting.

| Argument | Accepted kinds | Constraint |
| --- | --- | --- |
| `lambda` | exact number | non-negative |
| `mu` | exact number | positive, per server |
| `servers` | non-negative integer | 2 to 1 000 000 |

Returns a `StationAnalysis` with `probability_waiting` (Erlang C) and `busy_servers` = `a = lambda * S`
alongside `utilization = a / c`.

Errors: `UnsupportedDiscipline` when `servers` is outside `2..=1_000_000`; `InvalidServiceRate` for a
non-positive `mu`; `UnstableStation` when `lambda >= servers * mu`; `InvalidArgument` for a
non-integer server count.

```lovelace
sys_mmc(8, 5, 2).probability_waiting
```

```text
= 0.7(1) (Real)
```

`0.7(1)` is the exact rational `32/45` in periodic form.

#### sys_delay

`sys_delay(lambda, mu)` — M/M/&infin;, the infinite-server delay center: no customer waits.

| Argument | Accepted kinds | Constraint |
| --- | --- | --- |
| `lambda` | exact number | non-negative |
| `mu` | exact number | positive |

Returns a `StationAnalysis` where `response_time = S`, `waiting_time = 0`, `occupancy = a = lambda * S`
and `utilization` is the mean number in service (it can exceed 1). There is no
`probability_waiting`, and no stability refusal: a delay center has no capacity bound.

Errors: `InvalidServiceRate` for a non-positive `mu`; `InvalidArrivalRate` for a negative `lambda`.
A zero `lambda` is a defined, empty result whose `status` is `degenerate`.

```lovelace
sys_delay(8, 10).response_time
```

```text
= 0.1 (Real)
```

#### sys_station

`sys_station(name, service_time, visits[, servers_or_delay])` — one validated station record.

| Argument | Accepted kinds | Constraint |
| --- | --- | --- |
| `name` | text | 1 to 64 bytes of letters, digits, `_`, `.`, `-`; unique in the network |
| `service_time` | exact number | non-negative; positive when `visits` is positive |
| `visits` | exact number | non-negative |
| `servers` (4th) | non-negative integer | 2 to 1 000 000 (M/M/c) |
| `"delay"` (4th) | text, case-insensitive | the infinite-server discipline |

Returns a `Station` record with `discipline` (`fcfs`, `delay` or `mmc`), `service_demand = visits * service_time`,
`servers` when finite, and the `exact_values` sidecar. Omitting the fourth argument means one FCFS
server.

Errors: `InvalidIdentifier`, `InvalidServiceTime`, `InvalidVisits`, `UnsupportedDiscipline` (a
server count outside `2..=1_000_000`), `InvalidArgument` for a wrong kind.

```lovelace
sys_station("pool", 1/10, 1, 4).servers
```

```text
= 4 (Natural)
```

#### sys_open

`sys_open(arrival_rate, stations)` — an open Jackson network from a Poisson arrival rate and a
vector of station records.

| Argument | Accepted kinds | Constraint |
| --- | --- | --- |
| `arrival_rate` | exact number | non-negative; `0` is an empty stream |
| `stations` | vector of `Station` records | at least one; unique identifiers |

Returns a `QueueNetwork` with `workload = open`, the supplied `stations`, `precision` 30,
`arrival_rate` and `exact_values.arrival_rate`.

Errors: `EmptyNetwork`, `DuplicateStationId`, `InvalidArrivalRate`, `InvalidArgument` for a
non-vector or a non-`Station` element.

```lovelace
sys_open(8, [sys_station("api", 1/20, 1)]).workload
```

```text
= open
```

#### sys_closed

`sys_closed(population, think_time, stations)` — a closed network: a fixed population cycling
between the think state and the stations.

| Argument | Accepted kinds | Constraint |
| --- | --- | --- |
| `population` | non-negative integer | any `u64`; `0` is an empty model |
| `think_time` | exact number | non-negative |
| `stations` | vector of `Station` records | at least one; unique identifiers |

Returns a `QueueNetwork` with `workload = closed`, `population`, `think_time` and
`exact_values.think_time`.

Errors: `EmptyNetwork`, `DuplicateStationId`, `DegenerateModel` for a negative think time,
`InvalidArgument` for a non-integer population or a non-vector of stations.

```lovelace
sys_closed(2, 9/100, [sys_station("cpu", 1/100, 1)]).population
```

```text
= 2 (Natural)
```

#### sys_analyze

`sys_analyze(network[, precision])` — analyze a model with the solver its workload selects: open
Jackson for an open network, exact single-class MVA for a closed one.

| Argument | Accepted kinds | Constraint |
| --- | --- | --- |
| `network` | `QueueNetwork` record | constructed by `sys_open` / `sys_closed` (or an equivalent record) |
| `precision` (optional) | non-negative integer | 1 to 100 000; defaults to the network's `precision` |

Returns an `Analysis` record: provenance, `throughput`, `response_time`, `occupancy`,
`queue_length`, `population`/`think_time` for a closed model, `exact_values`, `laws` and `stations`.

Errors: whatever the selected solver refuses — `UnstableStation` (a named station at or above
utilization 1), `UnsupportedModel`, `DegenerateModel` — plus `InvalidArgument` for a non-network or
an out-of-range precision.

```lovelace
sys_analyze(sys_open(8, [sys_station("api", 1/20, 1)])).response_time
```

```text
= 0.08(3) (Real)
```

```lovelace
sys_analyze(sys_open(8, [sys_station("api", 1/20, 1)])).laws[1].holds
```

```text
= True (Boolean)
```

#### sys_mva

`sys_mva(network[, precision])` — exact single-class Mean Value Analysis; closed models only.

| Argument | Accepted kinds | Constraint |
| --- | --- | --- |
| `network` | `QueueNetwork` record | must carry a closed workload |
| `precision` (optional) | non-negative integer | 1 to 100 000; defaults to the network's `precision` |

Returns an `Analysis` record. The published station `response_time` is per visit and the aggregate
`response_time` is per transaction; the aggregate `occupancy` is `n - X(n) * Z` (a customer in the
think state is not in the network).

Errors: `UnsupportedModel` for an open workload or for a multi-server station (the message names the
failing assumption); `DegenerateModel` when zero demand and zero think time leave no defined
throughput; `InvalidArgument` for a non-network or an out-of-range precision.

```lovelace
sys_mva(sys_closed(2, 9/100, [sys_station("cpu", 1/100, 1)])).throughput
```

```text
= 19.(8019) (Real)
```

That is the exact rational `2000/101` jobs per second.

#### sys_bottlenecks

`sys_bottlenecks(network)` — the per-station demand ceilings, the bottleneck station(s) and the
network throughput bound.

| Argument | Accepted kinds | Constraint |
| --- | --- | --- |
| `network` | `QueueNetwork` record | open or closed |

Returns a `Bottlenecks` record: `basis` (`asymptotic-single-server`, `capacity-adjusted` or
`unbounded`), `max_demand`, `stations` (`BottleneckStation`: `id`, `service_demand`, `ceiling`,
`bottleneck`, `servers`, `exact_values`), `throughput_bound` (a void value when there is no finite
bound) and `warnings`. The bound is a ceiling as the population grows, not a throughput achieved at
a finite population.

Errors: `InvalidArgument` for a non-network. No solver runs, so there is no stability refusal.

```lovelace
sys_bottlenecks(sys_open(8, [sys_station("api", 1/20, 1), sys_station("db", 1/50, 2)])).throughput_bound
```

```text
= 20 (Real)
```

#### sys_sweep

`sys_sweep(network, from, to, step)` — re-analyze a model over a parameter grid.

| Argument | Accepted kinds | Constraint |
| --- | --- | --- |
| `network` | `QueueNetwork` record | closed sweeps the population, open sweeps the arrival rate |
| `from`, `to` | integer counts for a closed model, exact numbers for an open one | `from <= to` |
| `step` | same kind as the bounds | positive; `(to - from)` exactly divisible by it |

Returns a `Sweep` record: `axis` (`population` or `arrival_rate`), `points` (`SweepPoint`:
`parameter`, `throughput`, `response_time`, `max_utilization`, `exact_values`, `analysis`) and
`warnings`. The endpoint is inclusive only when the grid is exact; a partial final step is refused
rather than clamped. A sweep is capped at 10 000 points and by the solver's iteration budget.

Errors: `SweepEndpointNotOnGrid` for a partial step; `InvalidArgument`/`TypeMismatch` for a
non-positive step, a reversed range or a non-integral count; `StateSpaceBudgetExceeded` /
`IterationBudgetExceeded` when the request exceeds the caps.

```lovelace
sys_sweep(sys_closed(1, 0, [sys_station("cpu", 1/10, 1)]), 1, 3, 1).points[2].throughput
```

```text
= 10 (Real)
```

### A worked model

Build the network once, then read it and analyze it — each call is a record, so a script reads named
fields rather than parsing a sentence:

```lovelace
net = sys_open(8, [sys_station("api", 1/20, 1), sys_station("db", 1/50, 2)]); net.stations[1].service_demand
```

```text
= 0.04 (Real)
```

```lovelace
net = sys_open(8, [sys_station("api", 1/20, 1), sys_station("db", 1/50, 2)]); sys_analyze(net).response_time
```

```text
= 0.14(2156862745098039) (Real)
```

```lovelace
net = sys_open(8, [sys_station("api", 1/20, 1), sys_station("db", 1/50, 2)]); sys_bottlenecks(net).max_demand
```

```text
= 0.05 (Real)
```
