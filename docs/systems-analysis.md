# Systems analysis guide

`lovelace-systems` is the exact queueing and operations-research library behind the ten `sys_*`
builtins. This page is the complete guide: the model vocabulary, every solver, the results and
refusals they produce, the budgets and the precision contract, and the two ways to call it — from
Lovelace (the desktop workbench or `lovelace-run`) and from Rust.

Two other pages are narrower: [`docs/extensions/systems.md`](extensions/systems.md) is the reference
for the ten builtins, and [`docs/crates/lovelace-systems.md`](crates/lovelace-systems.md) is the
crate page (purpose, public surface, dependencies, tests). This guide is the long-form explanation
that connects them.

Every number below is an **exact** rational (a claim about the model's exact solution) unless it is
explicitly labelled **precision-limited**. The decimal and periodic renderings are how the engine
displays an exact rational; a rendering that had to truncate a period is marked as such. No number
on this page is error-bounded or parity-verified.

## Using it from the IDE or the CLI

The library's user-facing surface is the ten `sys_*` builtins, and they need no adapter: they are
ordinary Lovelace calls.

- **Desktop IDE.** Start `cargo run -p lovelace-desktop`, paste a call into the editor and press
  **Run** or `Ctrl+Enter`. The run's typed result line lands in the Logs pane and the committed
  values in the workspace pane (`Ctrl+4`; `Ctrl+Alt+1` selects its Result view). `F8` jumps to the
  last failed run's diagnostic and `F1` lists the keyboard map.
- **Command-line runner.** `cargo run -p lovelace-run -- --eval "sys_mm1(8, 10).utilization" --text`
  prints `= 0.8 (Real)`; `cargo run -p lovelace-run -- examples/systems.ls --text --omit-variables`
  runs the checked-in script, and `--json` gives the wire envelope.
- **Discovery.** `cargo run -p lovelace-run -- --functions sys` lists the matching builtins (the ten
  `Systems` ones; the pattern also matches `solve_system` and `solve_system_full`), and
  `cargo run -p lovelace-run -- --function sys_mm1` prints one entry (parameters, return kind,
  summary, examples).

**Build caveat.** The `exact_values` sidecar used by some examples on this page was added after
v0.7.1: a v0.7.1 desktop answers `record 'StationAnalysis' has no member 'exact_values'`. The metric
fields are unchanged, and a build of this tree has the sidecar; the v0.7.1 artifact predates it.

Each builtin is documented function by function — arguments and accepted kinds, the record it
returns and that record's fields, precision behaviour and refusals, with an executed example — in
[the systems reference](extensions/systems.md#per-builtin-reference). The rest of this page explains
the models those functions compute.

## What the library computes

| Area | Models | Language builtins |
|---|---|---|
| One station | M/M/1, M/M/c (Erlang C), M/M/&infin; (delay center) | `sys_mm1`, `sys_mmc`, `sys_delay` |
| Model construction | validated station and network records | `sys_station`, `sys_open`, `sys_closed` |
| Open networks | Jackson networks with explicit visit ratios | `sys_analyze` |
| Closed networks | exact single-class Mean Value Analysis | `sys_mva`, `sys_analyze` |
| Operational laws | Little, utilization, forced flow, interactive response | (fields of every result) |
| Capacity | bottleneck stations, throughput bounds, what-if scenarios | `sys_bottlenecks` |
| Sweeps | population (closed) and arrival-rate (open) grids | `sys_sweep` |

The crate depends only on the workspace's numeric kernel (`lovelace-nat`, `lovelace-int`,
`lovelace-rational`), contains no floating-point value path, no `unsafe` code
(`#![forbid(unsafe_code)]`) and no dependency on the interpreter, the desktop or the CLI.

## The model

A model is one `QueueNetwork`: a name, exactly one workload, and an ordered list of stations.
Station order is preserved in every result, so two identical runs serialize identically.

### Canonical units

Every measurement is an exact `Q` (the canonical `lovelace_rational::Rat`); a population is an
integer count. The model stores canonical units and applies no silent conversion:

| Quantity | Canonical unit | Example |
|---|---|---|
| Time | second | `9/100` is 90 ms |
| Rate | jobs per second | `8` is eight arrivals per second |
| Service time | seconds per visit | `1/20` is 50 ms of service |
| Service demand | seconds per job | `D = V * S` |
| Population | a non-negative integer | `2` |
| Utilization, probability | dimensionless | `4/5` |

The Rust API has typed wrappers for the unit-bearing quantities (`Time`, `Rate`, `Demand`) and a
`Dimension` tag with `check_same_dimension` for diagnostics; conversions are explicit and exact
(`Time::in_unit(value, TimeUnit::Minute)` multiplies by 60, `Rate::in_unit(value, RateUnit::PerHour)`
divides by 3600). The language surface takes canonical numbers only and offers no unit suffix.

### Identifiers

A station identifier is 1 to 64 bytes of ASCII letters, digits, `_`, `.` or `-`, and is unique
within a network. An invalid identifier is `InvalidIdentifier` / `DomainError`; a repeat is
`DuplicateStationId` / `DomainError`.

### Stations

A `Station` has an `id`, a `ServiceDiscipline`, a service time per visit and a visit ratio. The
service demand is always derived, never supplied: `service_demand = visits * service_time`.

| Discipline | Meaning | Server count |
|---|---|---|
| `FcfsSingleServer` | one FCFS server, infinite waiting room, load-independent service | 1 |
| `Delay` | infinitely many servers; no customer ever waits | none |
| `MultiServer { servers }` | `c` identical FCFS servers (M/M/c) | 2 to 1 000 000 |

The discipline labels are `fcfs`, `delay` and `mmc`. A multi-server station with a server count
outside `2..=1_000_000` is refused with `UnsupportedDiscipline` / `UnsupportedOperation`.

### Workloads

Exactly one workload per model, because the workload is a single enum rather than a flag:

- `OpenWorkload { arrival_rate }` — jobs arrive from outside at that rate and leave. Zero is a
  defined, empty arrival stream; a negative rate is `InvalidArrivalRate` / `DomainError`.
- `ClosedWorkload { population, think_time }` — a fixed population cycles between the network and
  the think state. Population is any `u64` (zero is a defined, empty model); a negative think time
  is `DegenerateModel` / `DomainError`.

Mixed open/closed models are not a mode this library can express, so there is no silent
coercion between them: the two solvers are separate entry points and each refuses the wrong
workload with `UnsupportedModel` / `UnsupportedOperation`.

### Validation

`QueueNetwork::new` (and the language constructors) validate before any solver runs, and return the
first refusal:

| Rule | Refusal |
|---|---|
| At least one station | `EmptyNetwork` / `DomainError` |
| Unique identifiers | `DuplicateStationId` / `DomainError` |
| Visits not negative | `InvalidVisits` / `DomainError` |
| A station with positive visits has a positive service time | `InvalidServiceTime` / `DomainError` |
| A station's service time is not negative | `InvalidServiceTime` / `DomainError` |
| Multi-server count in `2..=1_000_000` | `UnsupportedDiscipline` / `UnsupportedOperation` |
| Open arrival rate not negative | `InvalidArrivalRate` / `DomainError` |
| Closed think time not negative | `DegenerateModel` / `DomainError` |

A station with zero visits carries no traffic, so a zero service time is legal there — the model is
defined and empty rather than a fabricated `1/0`. Solver-specific assumptions (stability, which
disciplines a solver supports) are checked by the solver that relies on them and name the failing
assumption.

## Single-station analysis

Four entry points solve one station:

```text
mm1(lambda, mu, options, cancel)              -> AnalysisResult
mmc(lambda, mu, servers, options, cancel)     -> AnalysisResult
mm_infinity(lambda, mu, options, cancel)      -> AnalysisResult
solve_station(id, arrival, service_time, visits, discipline, options, cancel) -> StationResult
```

`mm1`, `mmc` and `mm_infinity` take rates: `lambda` arrivals per second and `mu` services per second
per server. `solve_station` takes a known arrival rate and a service time per visit, which is the
kernel both the single-station API and the open-network solver use.

The M/M/1 metrics are `rho = lambda * S`, `R = S / (1 - rho)`, `W = rho * R`, `L = lambda * R`,
`Lq = lambda * W`, `U = rho`. For M/M/c, `a = lambda * S`, `rho = a / c`, Erlang C gives the
probability of waiting `C`, `W = C / (c * mu - lambda)`, `R = W + S`, and the mean busy servers are
`a`. For M/M/&infin;, `R = S`, `W = 0`, and the occupancy is `a`. Every published station metric is
per visit; the network aggregate is per transaction.

| Field | Meaning |
|---|---|
| `throughput` | jobs per second through the station |
| `utilization` | `rho`; for a delay center, the mean number in service |
| `busy_servers` | mean busy servers (may exceed 1 only for a delay center) |
| `response_time` | residence time per visit (service + wait), seconds |
| `waiting_time` | wait before service per visit, seconds |
| `occupancy` | mean number in the station (in service + waiting) |
| `queue_length` | mean number waiting (excluding those in service) |
| `probability_waiting` | Erlang C; only M/M/c defines it |

A finite-capacity station at or above utilization 1 has no stationary distribution and is refused
with `UnstableStation` / `DomainError`, naming the station and its offered utilization. A
multi-server station does not need `c` in the metric names: `utilization` is already `a / c`.

A zero arrival rate is a defined, empty arrival stream: throughput, utilization, occupancy and
queue length are 0, `response_time` is the station's service time, `probability_waiting` is 0 for
M/M/c and absent otherwise, and the result's provenance status is `degenerate` with a warning.

```lovelace
sys_mm1(8, 10).utilization
```

```text
= 0.8 (Real)
```

```lovelace
sys_mm1(8, 10).response_time
```

```text
= 0.5 (Real)
```

`0.8` and `0.5` are exact: they are the rationals `4/5` and `1/2` rendered as terminating decimals.
The exact pair is always beside the projection:

```lovelace
sys_mm1(8, 10).exact_values.utilization
```

```text
= ExactRational(numerator: 4, denominator: 5) (ExactRational)
```

Erlang C and the delay center:

```lovelace
sys_mmc(8, 5, 2).probability_waiting
```

```text
= 0.7(1) (Real)
```

```lovelace
sys_delay(8, 10).occupancy
```

```text
= 0.8 (Real)
```

`0.7(1)` is the exact rational `32/45` in periodic form (the `(1)` repeats).

## Open networks (Jackson)

`analyze_open` requires an open workload. For each station the arrival rate is the forced-flow value
`X_i = V_i * lambda`; the station is then an independent M/M/c queue (Jackson's theorem) solved by
the same station kernel. The aggregate is:

- `response_time = sum_i V_i * R_i` — expected residence time of one transaction,
- `occupancy = lambda * response_time` (Little's law),
- `throughput = lambda`,
- `queue_length = sum_i Lq_i`.

Every finite-capacity station must be stable; otherwise the analysis is refused with
`UnstableStation` naming the offending station. Every station also publishes a `forced-flow` law
check, and the aggregate publishes a `little` check; both are exact comparisons.

```lovelace
sys_analyze(sys_open(8, [sys_station("api", 1/20, 1), sys_station("db", 1/50, 2)])).response_time
```

```text
= 0.14(2156862745098039) (Real)
```

Here `api` has one visit and a 50 ms service time, and `db` (still one server) has two visits and a
20 ms service time per visit, so its demand is `2/50 = 1/25` s. The exact network response time is
`29/204` s, shown above in periodic form. The per-station forced flow is visible directly:

```lovelace
sys_analyze(sys_open(8, [sys_station("api", 1/20, 1), sys_station("db", 1/50, 2)])).stations[1].throughput
```

```text
= 16 (Real)
```

Two degenerate cases are defined rather than refused: a zero arrival rate (an empty stream) and a
model in which every station has zero visits (no traffic). Both return zero throughput and zero
occupancy, a `degenerate` status and a warning. A delay center adds a warning to an otherwise exact
result, because it carries no capacity bound and its occupancy can exceed its server count.

## Closed networks (exact MVA)

`analyze_closed` requires a closed workload and solves the single-class Mean Value Analysis
recurrence exactly. With `N` customers, per-station service demand `D_i = V_i * S_i` and think time
`Z`:

```text
R_i(n) = D_i * (1 + Q_i(n-1))    FCFS single server
R_i(n) = D_i                     delay center
X(n)   = n / (Z + sum_i R_i(n))
Q_i(n) = X(n) * R_i(n)
```

The recurrence runs `N` iterations with `K` per-station values, so the cost is `O(N*K)` arithmetic
and `O(K)` state. Retaining every intermediate population is opt-in
(`SolverOptions::collect_populations`), bounded by `max_state_space` (`N*K` entries), and produces
`PopulationPoint`s.

Three conventions matter:

- **In-network occupancy.** A customer in the think state is not in the network, so the aggregate
  occupancy is `n - X(n) * Z`, not `n`. The solver checks that this equals the sum of the station
  occupancies and refuses with `InternalInvariantFailure` rather than publishing a mismatch.
- **Visits versus transactions.** The published `response_time` is per visit (`R_i(n) / V_i`), which
  is what a caller compares with the service time; the aggregate response time is per transaction.
  A station with zero visits reports a zero response time rather than dividing by zero.
- **Delay centers** report `waiting_time = 0` and `queue_length = 0`.

The supported nodes are FCFS load-independent single-server stations and delay centers. A
multi-server station in a closed model is refused with `UnsupportedModel` / `UnsupportedOperation`,
naming the failing assumption; a closed model is never approximated by an M/M/1 substitution.
Priorities, finite buffers, class switching, routing matrices, fork-join and simulation are outside
this family and are refused rather than approximated.

Degenerate cases: population 0 returns zeros with status `degenerate`; zero total demand with a
positive think time means the population never leaves the think state, so the throughput is
`N / Z`, the stations are zero and the status is `degenerate`; zero total demand with zero think
time is `DegenerateModel` / `DomainError` because the cycle time is `0/0`.

```lovelace
sys_mva(sys_closed(2, 9/100, [sys_station("cpu", 1/100, 1)])).throughput
```

```text
= 19.(8019) (Real)
```

That is the exact rational `2000/101` jobs per second, in periodic form. The station metrics at that
operating point:

```lovelace
sys_mva(sys_closed(2, 9/100, [sys_station("cpu", 1/100, 1)])).stations[0].response_time
```

```text
= 0.011 (Real)
```

```lovelace
sys_mva(sys_closed(2, 9/100, [sys_station("cpu", 1/100, 1)])).occupancy
```

```text
= 0.(2178) (Real)
```

With `N = 2`, `Z = 0.09` and `X = 2000/101`, the in-network occupancy is `2 - X*Z = 22/101`, which
is `0.(2178)`: the customers spend most of their cycle thinking, so only 22/101 of them are inside
the network on average. The result also carries the interactive-response law as an exact check:

```lovelace
sys_mva(sys_closed(2, 9/100, [sys_station("cpu", 1/100, 1)])).laws[2].name
```

```text
= interactive-response
```

## Operational laws

The laws are definitions plus exact checks; they are not approximations and not tolerance tests.

| Law | Function | Formula |
|---|---|---|
| Little | `little_law` | `L = X * R` |
| Utilization | `utilization_law` | `U = X * D` |
| Forced flow | `forced_flow` | `X_i = V_i * X` |
| Interactive response | `interactive_response` | `R = N / X - Z` |

The `check_*` forms (`check_little`, `check_utilization`, `check_forced_flow`, `check_interactive`)
return a `LawCheck { name, left, right, holds }` whose `holds` is the exact rational comparison.
`interactive_response` refuses a model with zero throughput (`DegenerateModel`) and a result that
would imply a negative response time.

```lovelace
sys_analyze(sys_open(8, [sys_station("api", 1/20, 1)])).laws[1].holds
```

```text
= True (Boolean)
```

## Capacity planning

### Bottlenecks and bounds

`bottlenecks(network)` reports, per station, the service demand and the station's own throughput
ceiling `c / D` (absent for a delay center and for zero demand). The network bound is the smallest
finite ceiling, and the report states its basis:

| Basis | Meaning |
|---|---|
| `asymptotic-single-server` | `1 / max D_i`; the exact asymptotic throughput of a closed single-server network as the population grows with zero think time, and the stability threshold of an open one |
| `capacity-adjusted` | `min c_i / D_i` when at least one station has several servers |
| `unbounded` | every station is a delay center, so there is no finite bound |

The bound is a ceiling, not a throughput achieved at a finite population, and the report says so in
its warnings; `max_demand` and the `bottleneck` flag on each entry make the report readable without
recomputing anything.

```lovelace
sys_bottlenecks(sys_open(8, [sys_station("api", 1/20, 1), sys_station("db", 1/50, 2)])).throughput_bound
```

```text
= 20 (Real)
```

```lovelace
sys_bottlenecks(sys_open(8, [sys_station("api", 1/20, 1), sys_station("db", 1/50, 2)])).basis
```

```text
= asymptotic-single-server
```

`api` has demand `1/20` and ceiling 20; `db` has demand `1/25` and ceiling 25, so `api` is the
bottleneck and the network bound is 20 jobs per second. The analysis above ran at 8 jobs per second,
well below that ceiling; the bound is what says so without running a solver at all.

### What-if scenarios

`Scenario` is a change applied to a copy of a model:

| Field | Effect |
|---|---|
| `arrival_rate` | replace an open arrival rate |
| `population` | replace a closed population |
| `think_time` | replace a closed think time |
| `demand_scale` | multiply named stations' service times by a factor |
| `server_counts` | replace named stations' server counts |

`Scenario::apply` validates the copy, so a scenario that names an unknown station (`UnsupportedModel`)
or sets an impossible server count (`UnsupportedDiscipline`) is refused rather than producing a
model that cannot exist; a scenario that makes a station unstable is refused by the solver when
`compare` runs, with `UnstableStation` naming it. `changed_fields` names what the scenario actually
changes on this workload; `compare` runs the baseline and the scenario through the same solver and
options and returns both results side by side. `closed_with` and `open_with` are the smaller helpers
that only replace the workload.

## Sweeps

A sweep is a batch of ordinary analyses over an immutable baseline: each point copies the model,
changes exactly one parameter and calls the same dispatcher.

- `sweep_population(network, from, to, step, options, cancel)` requires a closed workload and takes
  integer bounds.
- `sweep_arrival(network, from, to, step, options, cancel)` requires an open workload and takes
  rational bounds.

Both use one grid contract: the endpoint is inclusive only when the span is exactly divisible by the
step. A partial final step is refused with `SweepEndpointNotOnGrid` / `DomainError`, never clamped
onto the endpoint. The point count is bounded by `MAX_SWEEP_POINTS` (10 000) and by
`SolverOptions::max_iterations` before anything is computed, so a caller cannot request an unbounded
allocation. Points are returned in increasing parameter order, each with its full analysis, and
`SweepResult::max_utilization` reports the largest utilization over the whole sweep.

```lovelace
sys_sweep(sys_closed(1, 0, [sys_station("cpu", 1/10, 1)]), 1, 3, 1).points[2].throughput
```

```text
= 10 (Real)
```

An off-grid request is refused rather than rounded:

```lovelace
sys_sweep(sys_closed(1, 0, [sys_station("cpu", 1/10, 1)]), 1, 4, 2)
```

```text
Error [SweepEndpointNotOnGrid/DomainError]: The sweep endpoint 4 is not reachable from 1 in exact steps of 2; adjust the endpoint or the step.
```

## Results and provenance

Every solver returns `Result<AnalysisResult, SystemsError>`. An `AnalysisResult` carries:

| Field | Contents |
|---|---|
| `provenance` | model class, solver name and version, assumptions, exactness, approximation, precision, warnings, status, resource limit |
| `stations` | one `StationResult` per station, in declaration order |
| `aggregate` | network throughput, response time, occupancy, queue length, population/think time for a closed model, and the `law_checks` |
| `populations` | the intermediate `PopulationPoint`s, only when collection was requested |

`Provenance` states the terms of the result independently: `model_class` (`station-mm1`,
`station-mmc`, `station-mminf`, `open-jackson`, `closed-mva`), `solver_name`
(`mmc.erlang-c`, `jackson.open-mmc`, `mva.single-class`, ...), `solver_version` (`0.5.0`, the
algorithm version, distinct from the crate version), the exact `assumptions` sentences,
`exactness` (`exact` or the reserved `precision-limited`), `approximation` (empty for every solver
here), `precision`, `warnings`, `status` (`solved` or `degenerate`) and `resource_limit` (reserved
for a budget that shaped the result; no solver here sets it, because an exhausted budget is an
`Err`).

`StationResult` carries the identifier, discipline, server count, visits, service time and demand,
then the metrics from the single-station table, plus `probability_waiting` where it is defined.

## Errors

A refusal is a stable code and one of the frozen categories. A caller matches on the code, never on
prose, and the message is a complete sentence.

| Code | Category | Raised when |
|---|---|---|
| `InvalidIdentifier` | `DomainError` | an identifier is empty, over 64 bytes, or uses a character outside `A-Za-z0-9_.-` |
| `DuplicateStationId` | `DomainError` | two stations declare the same identifier |
| `EmptyNetwork` | `DomainError` | a network has no stations |
| `InvalidVisits` | `DomainError` | a visit ratio is negative |
| `InvalidServiceTime` | `DomainError` | a traffic-carrying station has a non-positive service time, or a service time is negative |
| `InvalidArrivalRate` | `DomainError` | an open arrival rate or a single-station `lambda` is negative |
| `InvalidServiceRate` | `DomainError` | a single-station `mu` is not positive |
| `UnsupportedDiscipline` | `UnsupportedOperation` | a multi-server station has a server count outside `2..=1_000_000` |
| `UnstableStation` | `DomainError` | a finite-capacity station's offered utilization is not below 1 |
| `DegenerateModel` | `DomainError` | the model has no defined stationary throughput (`0/0` cycle, negative think time, zero-throughput interactive law) |
| `UnsupportedModel` | `UnsupportedOperation` | the chosen solver does not implement this model, naming the assumption (wrong workload, M/M/c in a closed model, unknown scenario station) |
| `SweepEndpointNotOnGrid` | `DomainError` | a sweep endpoint is not reachable in exact steps |
| `IterationBudgetExceeded` | `BudgetExceeded` | the requested iteration or point count exceeds `max_iterations` |
| `StateSpaceBudgetExceeded` | `BudgetExceeded` | the retained population entries exceed `max_state_space`, or a sweep exceeds 10 000 points |
| `RationalBudgetExceeded` | `BudgetExceeded` | an exact intermediate exceeds `max_rational_digits` |
| `Cancelled` | `BudgetExceeded` | the caller's `Cancel` fired; no partial result is published |
| `TypeMismatch` | `TypeMismatch` | a sweep step is not positive, the range is reversed, or a count is not integral |
| `InternalInvariantFailure` | `InternalInvariantFailure` | a solver's own conservation check failed; never reachable by a valid model |

The categories are the interpreter's frozen seven (`ParseError` is the only one this library cannot
produce), so a refusal crosses the language and wire boundary with the same shape as a language
error.

## Budgets, precision and cancellation

`SolverOptions` carries the rendering precision and the hard budgets, with these defaults:

| Field | Default | Meaning |
|---|---|---|
| `precision` | 30 | decimal places used only when a `Q` is rendered as a `Real` |
| `max_iterations` | 100 000 | solver iterations, or sweep points |
| `max_state_space` | 1 000 000 | retained population entries (`N*K`) |
| `max_rational_digits` | 4 096 | decimal digits of any exact intermediate |
| `collect_populations` | `false` | retain every intermediate MVA population |

**Precision is not accuracy.** Every model value is an exact `Q`; `precision` is used only where the
value is projected to a decimal `Real`, and it is neither an iteration tolerance nor a certified
error bound. A periodic value renders with its period (`0.(3)`) regardless of the budget, and a
value whose period does not fit the budget renders truncated. The exact pair is always available on
the language side as `exact_values`.

```lovelace
sys_analyze(sys_open(8, [sys_station("api", 1/97, 1)]), 8).stations[0].service_time
```

```text
= 0.01030927 (Real)
```

That rendering is **precision-limited** (8 decimal places of a 96-digit period); the model value is
the exact rational `1/97`, and the record's `exact_values.service_time` still carries numerator 1
and denominator 97. Raising the precision renders more digits; it does not change the value.

A precision-limited `Real` is refused as a model input rather than promoted to an exact rational, so
a truncated projection cannot silently become a model parameter. A cancelled or budget-exhausted
computation returns `Err`, never a partial successful result: `Cancel` is a trait polled inside the
long loops, with `NeverCancel` and the atomic `FlagCancel` as the two standard implementations.

## The Lovelace surface

The ten builtins are declared in the registry with the same stable names the desktop and the runner
publish. Each one's arguments, accepted kinds, return record, fields, precision behaviour and
refusals are documented function by function in
[the systems reference](extensions/systems.md#per-builtin-reference); this table is the index:

| Builtin | Arguments | Returns |
|---|---|---|
| `sys_mm1` | `lambda, mu` | `StationAnalysis` |
| `sys_mmc` | `lambda, mu, servers` | `StationAnalysis` |
| `sys_delay` | `lambda, mu` | `StationAnalysis` |
| `sys_station` | `name, service_time, visits [, servers \| "delay"]` | `Station` |
| `sys_open` | `arrival_rate, stations` | `QueueNetwork` |
| `sys_closed` | `population, think_time, stations` | `QueueNetwork` |
| `sys_analyze` | `network [, precision]` | `Analysis` |
| `sys_mva` | `network [, precision]` | `Analysis` (closed only) |
| `sys_bottlenecks` | `network` | `Bottlenecks` |
| `sys_sweep` | `network, from, to, step` | `Sweep` |

`sys_station`'s optional fourth argument is an integer server count (M/M/c, at least 2) or the text
`"delay"` (infinite servers); omitting it means one FCFS server. `sys_analyze` dispatches on the
workload; `sys_mva` requires a closed one. `sys_sweep` sweeps the population for a closed model and
the arrival rate for an open one. The optional `precision` argument must be 1 to 100 000 and
defaults to the value recorded on the network (30).

Arguments are exact numbers: integers, fractions and exact decimals are accepted; a
precision-limited `Real` is refused with `InvalidArgument` / `TypeMismatch`. Every record's decimal
field is a `Real` projection, and each record carries an additive `exact_values` record of
`ExactRational { numerator, denominator }`, so a value such as `1/97` survives construction,
analysis and the wire unchanged.

### Records

The language returns records, not display strings; [the systems
reference](extensions/systems.md#outputs) lists every record and its full field list. The
provenance fields on `StationAnalysis` and `Analysis` are `model`, `solver`, `solver_version`,
`exact`, `exactness`, `precision`, `status`, `assumptions`, `approximation` and `warnings`. Every
exact-valued field (as opposed to an integer count such as `population` or `servers`) has a matching
entry in `exact_values`; compare exact values through their numerator and denominator, because an
`ExactRational` is a record and not a number:

```lovelace
sys_mm1(8, 10).exact_values.utilization.denominator
```

```text
= 5 (Natural)
```

The projection can still be compared directly, exactly:

```lovelace
sys_mm1(8, 10).utilization == 4/5
```

```text
= True (Boolean)
```

### Refusals through the language

A systems refusal keeps the crate's code and category and is raised as a `LangError`:

```lovelace
sys_mm1(10, 10)
```

```text
Error [UnstableStation/DomainError]: Station 'station' is unstable: its offered utilization is 1, which is not below 1.
```

```lovelace
sys_mva(sys_open(1, [sys_station("api", 1, 1)]))
```

```text
Error [UnsupportedModel/UnsupportedOperation]: The model is not supported: closed MVA requires a closed workload.
```

```lovelace
sys_station("has space", 1, 1)
```

```text
Error [InvalidIdentifier/DomainError]: Station identifier 'has space' must be 1 to 64 characters of letters, digits, '_', '.' or '-'.
```

### Desktop and runner

The workbench runs the same engine, so the calls above work unchanged in the editor: **Run** or
`Ctrl+Enter` evaluates the active document. The builtins are discoverable through the registry
(`--functions`, `--function sys_mm1`) and through the editor's completion. The desktop path is
described in [`docs/extensions/systems.md`](extensions/systems.md#run-it-in-the-ide-or-the-cli); the runner
prints the same result lines as the examples above:

```console
$ cargo run -p lovelace-run -- --eval "sys_mm1(8, 10).utilization" --text
= 0.8 (Real)
```

## The Rust surface

From another crate in this repository the dependency is
`lovelace-systems = { path = "../lovelace-systems" }`; from outside the repository, point that path
at a checkout. The crate is not published to crates.io yet.

### Free functions

| Signature | Returns | Purpose |
|---|---|---|
| `mm1(lambda, mu, options, cancel)` | `Result<AnalysisResult, SystemsError>` | M/M/1 single station |
| `mmc(lambda, mu, servers, options, cancel)` | `Result<AnalysisResult, SystemsError>` | M/M/c through Erlang C |
| `mm_infinity(lambda, mu, options, cancel)` | `Result<AnalysisResult, SystemsError>` | M/M/&infin; delay center |
| `solve_station(id, arrival, service_time, visits, discipline, options, cancel)` | `Result<StationResult, SystemsError>` | one station from a known arrival rate |
| `analyze(network, options, cancel)` | `Result<AnalysisResult, SystemsError>` | dispatch on the model's workload |
| `analyze_open(network, options, cancel)` | `Result<AnalysisResult, SystemsError>` | open Jackson analysis |
| `analyze_closed(network, options, cancel)` | `Result<AnalysisResult, SystemsError>` | exact single-class MVA |
| `bottlenecks(network)` | `BottleneckAnalysis` | bottleneck stations and the throughput bound |
| `changed_fields(network, scenario)` | `Vec<&'static str>` | the field names a scenario changes on this model |
| `compare(network, scenario, options, cancel)` | `Result<Comparison, SystemsError>` | baseline and scenario through the same solver |
| `closed_with(network, population, think_time)` | `Result<QueueNetwork, SystemsError>` | a copy with only the workload replaced |
| `open_with(network, rate)` | `Result<QueueNetwork, SystemsError>` | the same for an open workload |
| `sweep_population(network, from, to, step, options, cancel)` | `Result<SweepResult, SystemsError>` | closed population grid |
| `sweep_arrival(network, from, to, step, options, cancel)` | `Result<SweepResult, SystemsError>` | open arrival-rate grid |
| `little_law(throughput, response_time)` | `Q` | `L = X * R` |
| `utilization_law(throughput, service_demand)` | `Q` | `U = X * D` |
| `forced_flow(visits, throughput)` | `Q` | `X_i = V_i * X` |
| `interactive_response(population, throughput, think_time)` | `Result<Q, SystemsError>` | `R = N / X - Z` |
| `check_little(occupancy, throughput, response_time)` | `LawCheck` | exact check of Little's law |
| `check_utilization(utilization, throughput, service_demand)` | `LawCheck` | exact check of the utilization law |
| `check_forced_flow(station_throughput, visits, network_throughput)` | `LawCheck` | exact check of forced flow |
| `check_interactive(population, throughput, response_time, think_time)` | `LawCheck` | exact check of the interactive law |
| `validate(network)` | `Result<(), SystemsError>` | the construction rules on their own |
| `is_empty_traffic(network)` | `bool` | true when no station has a positive visit ratio |
| `station_utilization(service_time, throughput, servers)` | `Option<Q>` | `U = X * S / c`, `None` for a delay center |
| `check_stable(id, service_time, throughput, servers)` | `Result<(), SystemsError>` | the stability refusal on its own |
| `check_same_dimension(left, right)` | `Result<(), SystemsError>` | the dimension refusal on its own (**`units::check_same_dimension`**, not the crate root) |

### Types

| Type | Methods and fields |
|---|---|
| `Q` | the canonical rational (`lovelace_rational::Rat`, type-identical alias); this crate adds no methods to it |
| `Station` | `new(id, discipline, service_time, visits)`, `service_demand`, `servers`; fields `id`, `discipline`, `service_time`, `visits` |
| `StationId` | `new(value)`, `as_str` |
| `ServiceDiscipline` | variants `FcfsSingleServer`, `Delay`, `MultiServer { servers }`; `servers`, `as_str` |
| `OpenWorkload`, `ClosedWorkload` | public fields `arrival_rate` / `population` + `think_time` |
| `Workload` | variants `Open`, `Closed`; `class` |
| `QueueNetwork` | `new(name, workload, stations)`, `station(id)`, `station_mut(id)`, `total_demand`; fields `name`, `workload`, `stations` |
| `SolverOptions` | `default`, `check_rational`, `check_iterations`; fields `precision`, `max_iterations`, `max_state_space`, `max_rational_digits`, `collect_populations` |
| `Cancel` | trait method `is_cancelled`; implementations `NeverCancel` and `FlagCancel(Arc<AtomicBool>)` |
| `AnalysisResult` | `station(id)`, `laws_hold`; fields `provenance`, `stations`, `aggregate`, `populations` |
| `StationResult` | the station metric fields from the single-station table, plus `probability_waiting` |
| `AggregateResult` | `throughput`, `response_time`, `occupancy`, `queue_length`, `population`, `think_time`, `law_checks` |
| `PopulationPoint` | `population`, `throughput`, `response_time`, `occupancy`, `station_occupancy` |
| `Provenance` | `exact(...)` constructor; the provenance fields listed above |
| `LawCheck` | `exact(name, left, right)` constructor; fields `name`, `left`, `right`, `holds` |
| `Exactness` | variants `Exact`, `PrecisionLimited { precision }`; `is_exact`, `as_str` |
| `SolverStatus` | variants `Solved`, `Degenerate`; `as_str` |
| `BottleneckAnalysis` | `bottlenecks`; fields `stations`, `max_demand`, `throughput_bound`, `basis`, `warnings` |
| `BottleneckEntry` | fields `id`, `service_demand`, `servers`, `ceiling`, `bottleneck` |
| `BoundBasis` | variants `AsymptoticSingleServer`, `CapacityAdjusted`, `Unbounded`; `as_str` |
| `Scenario` | `apply(network)`; fields `arrival_rate`, `population`, `think_time`, `demand_scale`, `server_counts`; `Default` |
| `Comparison` | fields `baseline`, `scenario`, `changed` |
| `SweepResult` | `max_utilization`; fields `axis`, `points`, `warnings` |
| `SweepPoint` | fields `parameter`, `result` |
| `SweepAxis` | variants `Population`, `ArrivalRate`; `as_str` |
| `Time` | `seconds`, `in_unit`, `as_seconds`, `into_seconds` |
| `Rate` | `per_second`, `in_unit`, `as_per_second`, `into_per_second` |
| `Demand` | `seconds_per_job`, `as_seconds_per_job`, `into_seconds_per_job` |
| `TimeUnit`, `RateUnit` | `Second`/`Minute`/`Hour`, `PerSecond`/`PerMinute`/`PerHour` |
| `Dimension` | `as_str` |
| `SystemsError` | `code`, `category`, `message`, plus `new` and the named constructors that build the same categorized refusals (`invalid_identifier`, `unstable_station`, `rational_budget`, ...) |
| `ErrorCategory` | `as_str` |

### Constants and modules

| Constant | Value | Purpose |
|---|---|---|
| `SOLVER_VERSION` | `0.5.0` | the algorithm version published in every result's provenance |
| `VERSION` | the crate version | available without a Cargo environment at run time |
| `MAX_SERVERS` | `1_000_000` | the largest accepted server count |
| `MAX_SWEEP_POINTS` | `10_000` | the largest accepted sweep |
| `QUEUEING_ASSUMPTIONS` | 4 sentences | the M/M/1, M/M/c and M/M/&infin; assumptions |
| `JACKSON_ASSUMPTIONS` | 5 sentences | the open-network assumptions |
| `MVA_ASSUMPTIONS` | 5 sentences | the closed-network assumptions |

The public modules are `capacity`, `errors`, `model`, `mva`, `networks`, `operational_laws`,
`queueing`, `results`, `sweep`, `units` and `validation`; the root re-exports everything above, and
the three module-only items are `queueing::QUEUEING_ASSUMPTIONS`, `units::check_same_dimension` and
`validation::is_empty_traffic`.

`lovelace-systems/examples/mmc_and_mva.rs` is a complete program for the two models on this page's
first examples; run it from the repository root:

```console
$ cargo run -p lovelace-systems --example mmc_and_mva
mmc(8, 5, 2) probability_waiting=32/45
mva throughput=2000/101
```

Both lines are exact rationals (`32/45` and `2000/101`), printed as fractions rather than decimals.

The crate's own evidence: `cargo test -p lovelace-systems` covers the published M/M/1 and M/M/c
values, the MVA recurrence, the think-state occupancy regression, the sweep grid, conservation,
degenerate and unstable refusals, budgets and cancellation; `tests/oracle.rs` compares selected
results against the independent Python `fractions.Fraction` formulas in `parity/systems/oracle.tsv`;
and `parity/corpus/systems.json` pins the same models through the language. The measured counts are
in [`docs/testing.md`](testing.md).

## Assumptions and limits

The library solves the model's mathematics exactly; it does not test the world. Every result names
its assumptions in its provenance, and a consumer should check them against the system being
modelled:

- **Poisson arrivals, exponential service, infinite waiting rooms, load-independent service** for
  the station and open-network solvers.
- **Product form** for the closed solver: single customer class, FCFS single-server stations and
  delay centers, visit ratios and think time describing the closed population.
- **Visit ratios are supplied, not derived.** Routing matrices are not a P0 input; an open network
  takes explicit visit ratios and does not solve a traffic-equation system.
- **Multi-class, priorities, finite buffers, class switching, fork-join and simulation** are
  separate solvers; they are refused rather than approximated, and no approximation is published as
  exact.
- **The MVA result** is exact for the supported product-form network; it does not predict tail
  latency, and no systems builtin claims a p95/p99 without an appropriate distributional model.
- **A bound is not a result.** The bottleneck bound is a ceiling as the population grows; the report
  says it is not achieved at finite population, and the scenarios and sweeps exist to see what is
  achieved.

## How the numbers here were checked

The Lovelace outputs above were produced from this tree with the built runner and its `--text`
format (`cargo run -p lovelace-run -- <script> --text --omit-variables`), one expression per
script, and the error lines are the runner's own. The Rust outputs are the two statements in
`lovelace-systems/examples/mmc_and_mva.rs`, which asserts the same exact strings before printing
them. The underlying mathematics is pinned by three independent gates: the crate's unit and golden
tests, the Python `fractions.Fraction` oracle, and the documentation corpus that runs the language
examples through the engine. If a rendering on this page ever disagrees with a run, the run is the
evidence and the page is the defect.
