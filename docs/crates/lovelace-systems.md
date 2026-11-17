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

- **One shared rational kernel.** `Q` is a type-identical alias of `lovelace_rational::Rat`
  (`pub use lovelace_rational::Rat as Q;`). The crate owns queueing formulas, not arithmetic; the
  former Systems rational module was deleted in RNL-MIG-002. Full arbitrary-precision GCD normalization,
  cross-cancellation, checked division and budgets live in the kernel.
- **One workload per model.** Mixed open/closed networks are a different solver, not a silent mode.
- **Explicit unsupported combinations.** Multi-server closed MVA, priorities, finite buffers,
  class switching, routing matrices, fork-join and simulation are refused with `UnsupportedModel`
  naming the failing assumption. No M/M/1 substitution.
- **In-network occupancy.** A collected MVA population reports `Q_network(n) = n - X(n)*Z`; a
  customer in the think state is not in the network. The reported aggregate equals the sum of
  station occupancies.
- **One sweep grid.** An inclusive endpoint is accepted only when the range is exactly divisible by
  the step; a partial final step is refused with `SweepEndpointNotOnGrid`, never clamped.
- **Complexity.** Single-class MVA is `O(N*K)` arithmetic and `O(K)` state; intermediate
  populations are opt-in and bounded by `SolverOptions::max_state_space`.

## Inputs

- `Q` — the canonical exact rational from `lovelace-rational`; `from_parts` returns a `Result`,
  `div` returns `Option`, `checked_div` and `checked_pow` return categorized errors.
- `model` — `Station`, `StationId`, `ServiceDiscipline` (`FcfsSingleServer`, `Delay`,
  `MultiServer { servers }`), `OpenWorkload`, `ClosedWorkload`, `Workload`, `QueueNetwork`,
  `SolverOptions`, `Cancel`/`NeverCancel`/`FlagCancel`. A `Station` or `QueueNetwork` record may
  carry the additive `exact_values` sidecar; the language adapter consumes it, and a record without
  the sidecar still parses from the decimal fields for backward compatibility.
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

Dependencies are `lovelace-nat`, `lovelace-int` and `lovelace-rational` only. There is no external
crate, no floating point and no `unsafe` (`#![forbid(unsafe_code)]`).

## Outputs

`analyze`, `mm1`, `mmc`, `mm_infinity`, `analyze_open` and `analyze_closed` return
`Result<AnalysisResult, SystemsError>`. A result carries per-station metrics in declaration order,
network aggregates, exact law checks and provenance: model class, solver name and version,
assumptions, exactness, approximation (empty for P0), precision, warnings, status and any resource
limit. A refusal is a stable code plus one of the frozen categories.

Through the language, every numeric field is a decimal `Real` projection and an additive
`exact_values` record carries the canonical numerator/denominator pair
(`sys_mm1(8, 10).exact_values.utilization` is `ExactRational(numerator: 4, denominator: 5)`).

The model-construction records are values too, and they carry the same sidecar. A `Station` record
has `exact_values` with `service_time`, `visits` and `service_demand`; a `QueueNetwork` record has
`arrival_rate` for an open workload or `think_time` for a closed one. The language adapter's
`parse_station` and `parse_network` read that sidecar first and fall back to the decimal field only
when the whole `exact_values` record is absent. A present but malformed sidecar is refused rather
than silently replaced by the projection, so `sys_station("cpu", 1/97, 1)` reaches `sys_open` or
`sys_closed` and then `sys_analyze` or `sys_mva` with denominator 97, not with the truncated decimal
`0.0103...`.

## Precision behaviour

Every value is an exact `Q`. The rendering precision in `SolverOptions` is used only at the language
adapter's `Real::from_rational`, which preserves `lovelace-real`'s terminating/periodic metadata and
reports a truncated projection when the period does not fit the budget; it is not an iteration
tolerance and not a certified numerical error bound. A cancelled or budget-exhausted computation
returns `Err`, never a partial successful result. A precision-limited `Real` is refused as a model
input rather than promoted to an exact rational.

## Errors

| Refusal | Code / category |
|---|---|
| A rate, population, identifier or discipline outside the model vocabulary | `InvalidArrivalRate`, `InvalidPopulation`, `InvalidIdentifier`, `UnsupportedDiscipline` / `DomainError` or `UnsupportedOperation` |
| A finite-capacity station at or above utilization 1 | `UnstableStation` / `DomainError` |
| No defined stationary throughput (zero demand and zero think time) | `DegenerateModel` / `DomainError` |
| A sweep endpoint not on the exact step grid | `SweepEndpointNotOnGrid` / `DomainError` |
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

`lovelace-systems` is a workspace crate and is not published to crates.io yet, so it is consumed by
path: another crate in this repository depends on it with
`lovelace-systems = { path = "../lovelace-systems" }`, and a crate outside the repository points
that same line at its checkout.

`lovelace-systems/examples/mmc_and_mva.rs` is the complete, runnable program for the two models
below. From the repository root it prints the two exact values:

```console
$ cargo run -p lovelace-systems --example mmc_and_mva
mmc(8, 5, 2) probability_waiting=32/45
mva throughput=2000/101
```

The fence below is the model code from that program, not a source file on its own: the example
wraps it in `fn main` and adds the two `println!` lines. The documentation gate compiles and runs
the fence as written, so both assertions are executed rather than merely displayed.

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

The same models are reachable from the Lovelace language as the ten `sys_*` builtins, documented
with their records and refusals in [the systems extension page](../extensions/systems.md); that
page also covers [running them in the desktop IDE](../extensions/systems.md#run-it-in-the-ide-or-the-cli).
[The systems analysis guide](../systems-analysis.md) is the complete account of the model
vocabulary, both solvers, capacity, sweeps and the Rust surface.

### Tests

`cargo test -p lovelace-systems` runs the unit and golden tests: the M/M/1 and M/M/c published
values, the MVA recurrence at N=1 and N=2, the think-state occupancy regression, the exact sweep
grid, conservation checks, degenerate and unstable refusals, budget and cancellation paths, and
the adapter/queueing contract. The language family adds the `1/97` construction-to-analysis
regressions through both open and closed networks, plus the decimal-only backward-compatibility
path. `tests/oracle.rs` compares selected results against the independent
Python `fractions.Fraction` formulas in `parity/systems/oracle.tsv`.
`parity/corpus/systems.json` pins the same models through the language.
