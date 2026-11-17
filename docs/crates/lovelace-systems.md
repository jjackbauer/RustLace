# `lovelace-systems`

## Purpose

An exact analytical systems-modeling and performance-analysis library: queueing models, operational
laws, open Jackson networks, single-class Mean Value Analysis, bottleneck/capacity bounds and
sweeps. It is a standalone public crate usable from ordinary Rust and has no dependency on the
interpreter, the desktop or the CLI.

"Every reported number is an exact rational when the model is rational; the precision only chooses
how it is rendered" is the crate's central contract (`lovelace-systems/src/lib.rs`). The language
reaches the crate through a thin adapter (`lovelace-lang/src/systems_family.rs`), the CLI and
desktop reach it through the language, and none of those layers re-implements a formula.

### Design decisions

- **Exact rationals, reduced with `Nat::gcd`.** `lovelace_symbolics::Rat` was audited and rejected
  for this workload: it reduces only when both parts fit `u64` and rebuilds denominators through
  decimal strings, while an MVA recurrence multiplies denominators every iteration. The crate keeps
  one rational implementation, not two.
- **One workload per model.** Mixed open/closed networks are a different solver, not a silent mode.
- **Explicit unsupported combinations.** Multi-server closed MVA, priorities, finite buffers,
  class switching, routing matrices, fork-join and simulation are refused with `UnsupportedModel`
  naming the failing assumption. No M/M/1 substitution.
- **Complexity.** Single-class MVA is `O(N*K)` arithmetic and `O(K)` state; intermediate
  populations are opt-in and bounded by `SolverOptions::max_state_space`.

## Inputs

- `numeric::Q` — exact rational over `lovelace-nat`/`lovelace-int`; checked division, exact powers,
  `to_real`/`from_real`, digit-growth measure, canonical reduction after every operation.
- `model` — `Station`, `StationId`, `ServiceDiscipline` (`FcfsSingleServer`, `Delay`,
  `MultiServer { servers }`), `OpenWorkload`, `ClosedWorkload`, `Workload`, `QueueNetwork`,
  `SolverOptions`, `Cancel`/`NeverCancel`/`FlagCancel`.
- `queueing` — `mm1`, `mmc`, `mm_infinity`, `solve_station`.
- `operational_laws` — `little_law`, `utilization_law`, `forced_flow`, `interactive_response` and
  the exact `check_*` forms.
- `networks` — `analyze_open` (open Jackson).
- `mva` — `analyze_closed` (exact single-class MVA).
- `capacity` — `bottlenecks`, `Scenario`, `compare`, `closed_with`, `open_with`.
- `sweep` — `sweep_population`, `sweep_arrival`.
- `results` — `AnalysisResult`, `StationResult`, `AggregateResult`, `Provenance`, `Exactness`,
  `SolverStatus`, `LawCheck`, `PopulationPoint`.
- `errors` — `SystemsError`, `ErrorCategory`.
- `analyze` — the workload dispatcher.

Dependencies are `lovelace-nat`, `lovelace-int` and `lovelace-real` only. There is no external crate,
no floating point and no `unsafe` (`#![forbid(unsafe_code)]`).

## Outputs

`analyze`, `mm1`, `mmc`, `mm_infinity`, `analyze_open` and `analyze_closed` return
`Result<AnalysisResult, SystemsError>`. A result carries per-station metrics in declaration order,
network aggregates, exact law checks and provenance: model class, solver name and version,
assumptions, exactness, approximation (empty for P0), precision, warnings, status and any resource
limit. A refusal is a stable code plus one of the frozen categories.

## Precision behaviour

Every value is an exact `Q`. The rendering precision in `SolverOptions` is used only by
`Q::to_real`, which preserves `lovelace-real`'s terminating/periodic metadata; it is not an
iteration tolerance and not a certified numerical error bound. A cancelled or budget-exhausted
computation returns `Err`, never a partial successful result.

## Errors

| Refusal | Code / category |
|---|---|
| A rate, population, identifier or discipline outside the model vocabulary | `InvalidArrivalRate`, `InvalidPopulation`, `InvalidIdentifier`, `UnsupportedDiscipline` / `DomainError` or `UnsupportedOperation` |
| A finite-capacity station at or above utilization 1 | `UnstableStation` / `DomainError` |
| No defined stationary throughput (zero demand and zero think time) | `DegenerateModel` / `DomainError` |
| A discipline or workload the chosen solver does not implement | `UnsupportedModel` / `UnsupportedOperation`, naming the assumption |
| An iteration, state-space or exact-digit budget exhausted | `IterationBudgetExceeded`, `StateSpaceBudgetExceeded`, `RationalBudgetExceeded` / `BudgetExceeded` |
| Caller cancellation | `Cancelled` / `BudgetExceeded`, no partial result |

## Limitations

- The supported closed network is the single-class product-form subset: FCFS load-independent
  single-server stations and delay centers. Multi-server closed MVA, priorities, finite buffers,
  class switching, mixed open/closed networks, fork-join and simulation are separate solvers and
  are refused rather than approximated.
- Exact arithmetic preserves the model's solution, not the model's assumptions: the crate does not
  test that a real system is Poisson, exponential or product-form.
- Intermediate MVA populations are retained only on request; `N*K` must fit
  `SolverOptions::max_state_space`.
- Routing matrices are not a P0 input; open networks take explicit visit ratios.

## Examples

```rust
use lovelace_systems::{
    analyze, mmc, ClosedWorkload, NeverCancel, Q, QueueNetwork, ServiceDiscipline, SolverOptions,
    Station, Workload,
};

let result = mmc(
    &Q::from_i64(8),
    &Q::from_i64(5),
    2,
    &SolverOptions::default(),
    &NeverCancel,
)
.expect("stable M/M/2");
assert_eq!(
    result.stations[0]
        .probability_waiting
        .as_ref()
        .map(ToString::to_string)
        .as_deref(),
    Some("32/45")
);

let service = Q::from_i64(1).div(&Q::from_i64(100)).expect("1/100");
let closed = QueueNetwork::new(
    "interactive",
    Workload::Closed(ClosedWorkload {
        population: 2,
        think_time: Q::from_i64(9).div(&Q::from_i64(100)).expect("9/100"),
    }),
    vec![
        Station::new("cpu", ServiceDiscipline::FcfsSingleServer, service, Q::one())
            .expect("valid station"),
    ],
)
.expect("valid model");
let mva = analyze(&closed, &SolverOptions::default(), &NeverCancel).expect("solved");
assert_eq!(mva.aggregate.throughput.to_string(), "2000/101");
```

### Tests

`cargo test -p lovelace-systems` runs the unit and golden tests: the M/M/1 and M/M/c published
values, the MVA recurrence at N=1 and N=2, conservation checks, degenerate and unstable refusals,
budget and cancellation paths, and the adapter/queueing contract. `parity/corpus/systems.json`
pins the same models through the language.
