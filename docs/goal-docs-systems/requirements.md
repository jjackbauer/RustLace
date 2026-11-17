# Implementation contract — `lovelace-systems` and static integration

This is the orchestrator-owned architecture decision for PR B/PR C. Implementers follow it; a
deviation is a falsified claim unless the orchestrator records a superseding decision.

Paths are repository-relative. `cargo` means
`& "$env:USERPROFILE\.cargo\bin\cargo.exe"` on this host.

## 1. Crate and dependency direction

- New workspace member `lovelace-systems` (directory `lovelace-systems/`), version/edition/license
  inherited from `[workspace.package]`.
- Dependencies: `lovelace-nat`, `lovelace-int`, `lovelace-real` only. **Never** `lovelace-lang`,
  `lovelace-desktop`, `lovelace-symbolics`, `lovelace-json`, or an external crate.
- `#![forbid(unsafe_code)]`, `missing_docs = "warn"` inherited; every public item documented.
- `lovelace-lang` gains `lovelace-systems = { path = "../lovelace-systems" }`. The adapter lives in
  `lovelace-lang/src/systems_family.rs` (or `systems.rs`) and re-implements **no** formula.
- No floating point anywhere: no `f32`/`f64` in the crate or adapter.
- Dependencies of `lovelace-systems` on the interpreter are impossible by manifest, enforced by
  `cargo tree -p lovelace-systems`.

## 2. Module map (ARC-002)

| Module | Owns |
|---|---|
| `numeric` | `Q` exact rational, checked arithmetic, `Real` conversion, digit-growth budget |
| `units` | dimension tags and canonical units (`Time`, `Rate`, `Demand`, `util` conversion) |
| `model` | `Station`, `StationId`, `ServiceDiscipline`, `Workload`, `OpenWorkload`, `ClosedWorkload`, `QueueNetwork`, `SolverOptions` |
| `validation` | identifier, population, capacity, rate, visits, routing, reachability, stability checks |
| `queueing` | M/M/1, M/M/c (Erlang C), M/M/∞, single-station analysis |
| `operational_laws` | Little, utilization, forced flow, interactive response, conservation checks |
| `networks` | open Jackson analysis, traffic equations from visits or routing |
| `mva` | exact single-class MVA recurrence |
| `capacity` | bottleneck classification, bounds, scenario comparison |
| `sweep` | population / arrival-rate sweeps with bounded point counts |
| `results` | `AnalysisResult`, `StationResult`, `Provenance`, `Exactness`, `SolverStatus` |
| `errors` | `SystemsError`, `ErrorCategory`, stable codes, `Display` |
| `lib.rs` | crate docs, re-exports, `analyze` dispatcher |

## 3. Exact numeric contract (NUM-001..NUM-006)

`numeric::Q` is `{ num: Int, den: Nat }`, canonical: `den > 0`, `gcd(|num|, den) == 1`, zero is
`0/1`, sign only in `num`. Constructors reject a zero denominator. Every operation is exact.

**`lovelace-symbolics::Rat` audit (the decision's evidence):** its `new` reduces only when numerator
and denominator both fit `u64`; `add`/`mul` convert denominators through decimal strings and do not
reduce arbitrary-precision results. A single-class MVA at N≈40 multiplies denominators every
iteration, so unreduced growth is exponential in digits. Therefore systems uses `Q` over the same
`Nat`/`Int` limb types, with `Nat::gcd` reduction after every operation. This is one rational
implementation for this crate, not a second ad-hoc system: the audit and the shared limb types are
the recorded rationale (DEC in the goal journal).

- Public arithmetic returns `Result<_, SystemsError>` for division by zero and for exceeding
  `SolverOptions::max_rational_digits` (decimal digits of numerator+denominator), code
  `RationalBudgetExceeded`, category `BudgetExceeded`. A caller never gets a truncated rational.
- `Q::to_real(max_digits) -> Real` uses `Real::from_fraction`, so terminating decimals stay exact and
  a repeating quotient carries period metadata. `Q::to_decimal(digits)` is rendering only.
- `Q::from_real(&Real)` uses `Real::to_rational`, exact for every `Real` (terminating or periodic).
- Digits/precision are rendering and budget knobs, never a license to round in the math.

## 4. Units (MOD-004)

Canonical base units: time = seconds, rate = jobs/second, service time = seconds/visit, demand =
seconds/job, population = jobs, utilization/probability = dimensionless.

`units::Time`, `units::Rate`, `units::Demand` wrap a `Q` and expose `seconds/minutes/hours`,
`per_second/per_minute/per_hour` constructors plus `as_seconds`/`as_per_second`. Incompatible
quantities cannot be added because the wrappers differ by type; the adapter documents that builtin
arguments are canonical seconds/jobs/second. No conversion is applied silently.

## 5. Model and identifiers (MOD-001..MOD-006, ARC-003)

```rust
pub struct StationId(String);            // 1..=64 bytes, [A-Za-z0-9_.-], unique per network
pub struct ClassId(String);              // P0: one class, "default"
pub enum ServiceDiscipline {
    FcfsSingleServer,                    // load-independent, one server
    Delay,                               // infinite servers
    MultiServer { servers: usize },      // M/M/c, servers >= 2
}
pub struct Station {
    pub id: StationId,
    pub discipline: ServiceDiscipline,
    pub service_time: Q,                 // s/visit
    pub visits: Q,                       // visits per transaction, >= 0
}
pub enum Workload { Open(OpenWorkload), Closed(ClosedWorkload) }
pub struct OpenWorkload { pub arrival_rate: Q }        // jobs/s, > 0 for analysis
pub struct ClosedWorkload { pub population: u64, pub think_time: Q } // >= 0
pub struct QueueNetwork { pub name: String, pub workload: Workload, pub stations: Vec<Station> }
pub struct SolverOptions {
    pub precision: usize,          // decimal digits for Real rendering, default 30
    pub max_iterations: u64,       // default 100_000
    pub max_state_space: u64,      // default 1_000_000 population-vector entries
    pub max_rational_digits: usize,// default 4_096
    pub collect_populations: bool, // default false (MVA retains only final+previous)
}
```

`Station::service_demand() = visits * service_time`. Zero visits with positive service time is a
station that is present but carries no traffic; it reports zero throughput/occupancy, never a
division by zero. A station with `visits > 0` requires `service_time > 0` (finite positive capacity).
Mixed open/closed is representable only by the two variants and is not a silent mode.

Validation (`validation::validate`) runs before any analysis and returns the first stable error:
`InvalidIdentifier`, `DuplicateStationId`, `EmptyNetwork`, `InvalidPopulation`, `InvalidArrivalRate`,
`InvalidServiceTime`, `InvalidVisits`, `UnsupportedDiscipline`, `MixedWorkloadNotSupported`,
`UnstableStation`, `DegenerateModel`, `RoutingNotReachable`, `RoutingProbabilityRow`,
`RationalBudgetExceeded`, `IterationBudgetExceeded`, `StateSpaceBudgetExceeded`, `Cancelled`,
`UnsupportedModel`. Categories reuse the frozen seven (`DomainError`, `UnsupportedOperation`,
`BudgetExceeded`, `NoSolution`, `TypeMismatch`, `InternalInvariantFailure`); no new category is
invented. Messages are complete sentences ending in a period.

## 6. Station formulas (QUE-001..QUE-003)

Inputs `lambda >= 0`, `mu > 0` (mu is per server). `a = lambda/mu`, `rho = a/c`.

- **M/M/1** (`c=1`): stationary only when `lambda < mu`; otherwise `UnstableStation` with
  `rho >= 1` in the message and **no** finite metrics. `rho`, `W = 1/(mu-lambda)`,
  `Wq = rho/(mu-lambda)`, `L = lambda*W`, `Lq = lambda*Wq`, `U = rho`, `X = lambda`.
- **M/M/c**: `a = lambda/mu`, `rho = a/c`, stable only when `lambda < c*mu`.
  `P_wait = [a^c/(c! (1-rho))] / [sum_{k=0}^{c-1} a^k/k! + a^c/(c!(1-rho))]`;
  `Wq = P_wait/(c*mu - lambda)`, `W = Wq + 1/mu`, `Lq = lambda*Wq`, `L = Lq + a`, `U = rho`,
  `busy_servers = a`. Evaluated with exact rationals and a stable recurrence (agreement with the
  direct formula in tests); no `f64`.
- **M/M/∞ / delay**: `U = a` (mean busy servers, may exceed 1), `W = 1/mu`, `Wq = 0`,
  `L = a`, `Lq = 0`, `X = lambda`. Distinguished from a large finite `c` by model class, not by
  rounding.

## 7. Operational laws and bottlenecks (LAW-001..LAW-004)

- `little_law(X, W) = X*W`; `utilization_law(X, D) = X*D`; `forced_flow(V, X) = V*X`;
  `interactive_response(N, X, Z) = X*(R+Z)` inverted as `R = N/X - Z` with domain checks.
- Station result scopes are explicit: `occupancy` (mean in system) vs `queue_length` (mean waiting),
  `response_time` (residence) vs `waiting_time`; per-visit station metrics multiplied by `visits`
  only where the network aggregate is reported.
- **Bottleneck**: for single-server load-independent stations, `D_i = V_i*S_i` and
  `X_bound = 1/max(D_i)`; for M/M/c, capacity-adjusted `X_bound = min_i c_i/(V_i*S_i)`; delay
  centers contribute no finite bound. The result names the station(s) and whether the bound is a
  mathematical asymptotic bound (single-server, Z=0) or a capacity bound (multi-server). It never
  claims the bound is achieved.
- Scenario comparison copies the model and reports baseline vs scenario with the changed fields.

## 8. Open networks (NET-001)

Accepted P0 form: explicit per-station `visits` (`V_i >= 0`) plus external `arrival_rate`.
`X_i = V_i*lambda`; every finite-capacity station must satisfy `X_i*S_i < c_i`; M/M/c formulas use
`lambda_i = X_i`, `mu_i = 1/S_i`. Network response per transaction `R = sum_i V_i*R_i`; system
occupancy `L = lambda*R`; per-station `L_i = X_i*R_i`. A routing matrix is validated when supplied
(rows are probabilities summing to 1, traffic equations solvable) and solved by exact Gaussian
elimination to visits; an unreachable/inconsistent matrix is `RoutingNotReachable`, never a guessed
visit count. Mixed open/closed is `UnsupportedModel`.

## 9. Exact single-class MVA (NET-002, MVA-001..MVA-004)

Supported: closed, one class, stations of `FcfsSingleServer` and `Delay`, visit ratios, think time
`Z >= 0`. Recurrence for `n = 1..=N`, `Q_i(0) = 0`:

```
R_i(n) = D_i * (1 + Q_i(n-1))        // FcfsSingleServer, D_i = V_i*S_i
R_i(n) = D_i                          // Delay
X(n)   = n / (Z + sum_i R_i(n))
Q_i(n) = X(n) * R_i(n)
U_i(n) = X(n) * D_i                   // single-server; a delay center reports occupancy X*D_i
```

- `N = 0` returns all-zero metrics, `X = 0`, status `Degenerate` with a warning, not an error.
- `sum D_i = 0`: if `Z > 0`, `X = N/Z`, `R_i = Q_i = 0`; if `Z == 0`, `DegenerateModel` (0/0), no
  arbitrary throughput.
- Result exposes `R = sum_i R_i`, `N == X*(R+Z)` and `Q_i == X*R_i`, `U_i == X*D_i` as checked
  invariants; a violation is `InternalInvariantFailure`, never a rounded pass.
- Multi-server FCFS, priorities, finite buffers, class switching, mixed workloads are refused with
  `UnsupportedModel` naming the failing assumption (`MVA-004`); never approximated by M/M/1.
- Complexity: `O(N*K)` arithmetic, `O(K)` state; all intermediate populations only when
  `collect_populations` and `N*K <= max_state_space`, else `StateSpaceBudgetExceeded`.
- Every long loop polls the cancellation token and the iteration budget; a cancelled or
  budget-exceeded run returns `Err`, never a partial successful `AnalysisResult`.

## 10. Results and provenance (NUM-003)

`AnalysisResult` carries: `provenance { model_class, solver_name, solver_version, assumptions:
Vec<&'static str>, numeric_exactness: Exact | PrecisionLimited { precision }, approximation:
Option<&'static str>, precision, warnings, status: Solved | Degenerate, resource_limit:
Option<String> }` plus `stations: Vec<StationResult>` and `aggregate`. All numeric fields are `Q`;
the adapter renders `Real`s and exact numerator/denominator text for JSON. Ordering is the station
declaration order (never a hash map); identical inputs/solver/precision produce identical output.

`StationResult`: `id, discipline, visits, service_time, service_demand, throughput, utilization,
busy_servers, response_time, waiting_time, occupancy, queue_length, probability_waiting:
Option<Q>`. `AggregateResult`: `throughput, response_time, occupancy, queue_length, population:
Option<u64>, think_time: Option<Q>`, plus `law_checks: Vec<LawCheck>` (`name, lhs, rhs, holds`).

## 11. Language adapter and builtin surface (ARC-004, API-001..API-006)

New file in `lovelace-lang`; functions registered in `registry::BUILTINS` with
`module: "Systems"`, normal `category`, `summary`, `return_kind`, `examples`, and dispatched from
`builtins::call`. No dynamic registration.

Surface (collision-safe; arguments canonical seconds / jobs per second):

| Builtin | Arity | Returns |
|---|---|---|
| `sys_mm1(lambda, mu)` | 2 | record `StationAnalysis` |
| `sys_mmc(lambda, mu, servers)` | 3 | record `StationAnalysis` with `servers`, `probability_waiting` |
| `sys_delay(lambda, mu)` | 2 | record `StationAnalysis` (M/M/∞) |
| `sys_station(name, service_time, visits)` | 3 | record `Station` (FCFS single) |
| `sys_station(name, service_time, visits, servers_or_delay)` | 4 | record `Station`; integer >=2 = M/M/c, text `"delay"` = infinite server |
| `sys_open(arrival_rate, stations)` | 2 | record `QueueNetwork` (open workload) |
| `sys_closed(population, think_time, stations)` | 3 | record `QueueNetwork` (closed workload) |
| `sys_analyze(network[, precision])` | 1–2 | record `Analysis` (open Jackson or closed MVA chosen by workload) |
| `sys_mva(network[, precision])` | 1–2 | record `Analysis`, closed only, else `UnsupportedOperation` |
| `sys_bottlenecks(network)` | 1 | record `Bottlenecks` |
| `sys_sweep(network, from, to, step)` | 4 | array of records `SweepPoint` (closed = population integers, open = arrival rate) |

Records are constructed natively (`Value::Record`); no grammar change. Field access is the existing
record field syntax; a test must pin `r.rho`-style access and array-of-record indexing. Structured
results are the only result; no display string is parsed by any consumer. Error paths use
`LangError::coded` with the mapped stable code/category and a UTF-16 position; cancellation and
budget map to `Cancelled` / `BudgetExceeded`.

`--functions` / `--function sys_mm1`, help, completion and the structured projection all read the
same static descriptor; a test pins that each `sys_*` name is discoverable and that
`--function` prints its summary/examples. Desktop parity: the workbench consumes engine snapshots
and calls the same engine; `FunctionOrigin::Plugin` and its "plugin" label/colour are deleted in
favour of builtin/module/user, no model math is added to the UI, and `--gui-selftest` /`--ui-audit`
stay green (API-004).

Samples (executable documentation tests, actual language syntax):
`docs/language/examples/systems/*.ls` — web request through API/database/cache stations; a worker
pool; a closed interactive workload; a sweep plotted through the existing SVG plot facility.

## 12. Wire and protocol migration (REM-005)

- `BuiltinInfo` drops `plugin: Option<&'static str>` and gains `module: &'static str`
  (`"Core" | "DSP" | "Symbolics" | "MathIR" | "Systems"`). It is a discovery label: it gates
  nothing and is never consulted by `call`.
- `lovelace-run` `PROTOCOL_VERSION` becomes `"2"`. `functions[]` carries `module`, no `plugin`.
  `StateFunction` drops `plugin`, gains `module`. Help renders `Module: Systems`.
- `CONTRACT.md` and `docs/symbolics/dsh-protocol.md` document v2 and the migration:
  `plugin` removed, `module` added, all other fields stable. Golden envelopes are regenerated with
  `LOVELACE_RECORD_GOLDENS=1`; every protocolVersion assertion moves 1 → 2 (a stated contract move,
  not a weakened test).
- Baseline function count stays 124; systems names are additive.

## 13. Deletion rules (PR A)

- Delete `lovelace-lang/src/modus.rs`, `pub mod modus`, `Engine::modus`, `Engine::enabled_plugins`,
  `new_bare`, `load_plugin`, `register_builtin`, `register_builtin_descriptor`,
  `register_array_builtin`, `register_kernel`, `try_dispatch_kernel`, `plugin_name_of`,
  `plugin_enabled`, `registered_descriptor`, `register_assumption_context`, `host_function`, the
  `modus.call` dispatch branch, the `plugin_enabled` gate in `call`, the Modus assumption scope in
  `eval`, and `StateFunction.plugin`.
- Delete `builtin_count_for_plugins`; every builtin is available on `Engine::new()`.
- Preserve `assume`/`assumptions` + `context::Context` behavior; preserve DSP's 30-digit default
  with tests and an ADR.
- `round67_modus.rs`: delete. `round69_parameter_domain.rs`, `round95_engine_singletons.rs`,
  `suite_parity_batch.rs`, `round95_singletons.rs` and any other test that used the plugin surface:
  migrate the independent guarantee into a core test, delete only the plugin-specific assertion,
  and write the reason in `docs/goal-docs-systems/deleted-tests.md`.
- No replacement plugin framework of any kind: no trait-object registry, no `Box<dyn Fn>` builtin
  table, no runtime registration.
