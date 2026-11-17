# Round 2 — exact model transport through `sys_station` / `sys_open` / `sys_closed`

- **Goal**: PR #9, review Finding 1 (P0): exact model values are lost during construction.
- **Branch**: `fix/release-provenance`, head `8ec1aaf`.
- **Scoped source change**: `lovelace-lang/src/systems_family.rs` only.
- **Observed**: 2026-11-17 (America/Sao_Paulo), in the shared workspace; other agents' concurrent
  edits to other crates/docs were present and are not part of this change.
- **Not committed**: no commit was made (the task forbids committing).

## 1. Test-first evidence (before the implementation)

The two named regressions were added first, at
`lovelace-lang/src/systems_family.rs:1017-1047`:

- `long_period_service_time_survives_station_and_open`
- `long_period_think_time_survives_closed_and_mva`

### 1.1 The literal compound filter command

```console
$ cargo test -p lovelace-lang systems_family long_period
cargo.exe : error: unexpected argument 'long_period' found
Usage: cargo.exe test [OPTIONS] [TESTNAME] [-- [ARGS]...]

For more information, try '--help'.
EXITCODE=1
```

Cargo 1.100.0 accepts exactly one `TESTNAME` positional, so the compound filter cannot run as
written. The same two tests were run with the equivalent single filter:

### 1.2 Observed failing run (pre-fix)

```console
$ cargo test -p lovelace-lang systems_family::tests::long_period
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.01s
     Running unittests src\lib.rs (...)

running 2 tests
test systems_family::tests::long_period_service_time_survives_station_and_open ... FAILED
test systems_family::tests::long_period_think_time_survives_closed_and_mva ... FAILED

failures:

---- systems_family::tests::long_period_service_time_survives_station_and_open stdout ----

thread 'systems_family::tests::long_period_service_time_survives_station_and_open' (150640) panicked at lovelace-lang\src\systems_family.rs:853:37:
s = sys_station("cpu", 1/97, 1); m = sys_open(1, [s]); sys_analyze(m): sys_open(): argument 2 expects an exact number (an integer, fraction or exact decimal); got a precision-limited Real `0.010309278350515463917525773195`.
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- systems_family::tests::long_period_think_time_survives_closed_and_mva stdout ----

thread 'systems_family::tests::long_period_think_time_survives_closed_and_mva' (213664) panicked at lovelace-lang\src\systems_family.rs:853:37:
s = sys_station("cpu", 1/97, 1); m = sys_closed(2, 1/97, [s]); sys_mva(m): sys_closed(): argument 3 expects an exact number (an integer, fraction or exact decimal); got a precision-limited Real `0.010309278350515463917525773195`.

failures:
    systems_family::tests::long_period_service_time_survives_station_and_open
    systems_family::tests::long_period_think_time_survives_closed_and_mva

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 1737 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p lovelace-lang --lib`
EXITCODE=101
```

This is the `InvalidArgument` / `TypeMismatch` symptom named by the review: the decimal projection
`0.010309278350515463917525773195` is re-read as an exact model parameter and refused. The panic
line was the shared `eval` helper; the failure message carries the builtin argument index and the
lossy projection.

## 2. Implementation

All line references are to the final
`lovelace-lang/src/systems_family.rs`.

### 2.1 Emission

- `station_record` (`:570-595`) keeps the decimal fields `service_time`, `visits`,
  `service_demand` (`:578-580`) and adds the additive sidecar at `:587-594`:
  `exact_values` = `ExactValues(service_time, visits, service_demand)`, each an
  `ExactRational(numerator: Integer, denominator: Natural)` built with the existing
  `exact_rational`/`exact_values` helpers (`:546-564`).
- `network_record` (`:598-632`) keeps the decimal fields (`arrival_rate` at `:617`,
  `think_time` at `:625`) and adds:
  - open workload: `exact_values` with `arrival_rate` (`:618-621`);
  - closed workload: `exact_values` with `think_time` (`:626-629`).

The decimal fields are unchanged in value and position; the sidecar is additive.

### 2.2 Consumption

- `exact_value` (`:329-391`) is the authoritative reader:
  - it returns `Ok(None)` only when the whole `exact_values` field is absent (`:335-341`);
  - when the sidecar is present it requires `ExactValues`, the named entry, `ExactRational`,
    an `Integer` numerator (`:355-364`), a `Natural` denominator (`:365-374`), a non-zero
    denominator (`:375-381`), and a canonical `Q` (`:382-390`). Every malformed case is
    reported through `ctx.error` (`:196-212`), i.e. `InvalidArgument` / `TypeMismatch`.
- `model_number` (`:395-405`) calls `exact_value`; only `None` falls back to the legacy decimal
  `Real` through `ctx.number` (`:403`).
- `parse_station` uses it for `service_time` and `visits` (`:411-412`).
- `parse_network` uses it for `arrival_rate` and `think_time` (`:448` and `:452`).

### 2.3 No zero fallback

Observed search for fallbacks after the change:

```console
$ Select-String -Path lovelace-lang\src\systems_family.rs -Pattern 'unwrap_or_else'
443:        .unwrap_or_else(|| "network".to_string());
959:            .unwrap_or_else(|error| panic!("{source}: {}", error.message));
```

Neither is a numeric fallback. `unwrap_or_else(zero)` was not added anywhere; the only fallback in
the parse path is `model_number`'s `None => ctx.number(...)` (`:403`), reached only when the entire
`exact_values` record is absent.

### 2.4 Tests

New tests (none of the pre-existing 10 tests was deleted, ignored, or relaxed):

- `long_period_service_time_survives_station_and_open` (`:1017-1033`): the whole
  `sys_station → sys_open → sys_analyze` source runs, asserts
  `sys_station("cpu", 1/97, 1).exact_values.service_time.denominator == 97`, and asserts the
  analysis station's exact denominator is 97.
- `long_period_think_time_survives_closed_and_mva` (`:1035-1047`): the whole
  `sys_station → sys_closed → sys_mva` source runs and asserts
  `sys_mva(m).exact_values.think_time.denominator == 97`.
- `long_period_arrival_rate_survives_open_network` (`:1049-1056`): supplementary guard for the
  open workload sidecar; `sys_analyze(sys_open(1/97, [...]))` succeeds and the exact throughput
  denominator is 97. (This third guard was added after the implementation; its pre-fix failure was
  not captured.)
- `malformed_exact_values_are_refused_instead_of_falling_back` (`:1058-1102`): a present sidecar
  with a `Text` numerator on an otherwise lossless decimal record is refused with
  `InvalidArgument` / `TypeMismatch`. (Supplementary guard added after the implementation; it
  would have failed before the fix by silently falling back.)
- `decimal_only_records_still_parse_for_backward_compatibility` (`:1104-1142`): a `Station` and an
  open `QueueNetwork` without `exact_values` still parse from the decimal `Real` fields.

## 3. Post-fix verification (observed)

### 3.1 Long-period regressions

```console
$ cargo test -p lovelace-lang long_period
running 3 tests
test systems_family::tests::long_period_arrival_rate_survives_open_network ... ok
test systems_family::tests::long_period_service_time_survives_station_and_open ... ok
test systems_family::tests::long_period_think_time_survives_closed_and_mva ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 1739 filtered out; finished in 0.01s
EXITCODE=0
```

### 3.2 `systems_family` suite

```console
$ cargo test -p lovelace-lang systems_family
running 15 tests
test systems_family::tests::decimal_only_records_still_parse_for_backward_compatibility ... ok
test systems_family::tests::malformed_exact_values_are_refused_instead_of_falling_back ... ok
test systems_family::tests::unstable_and_unsupported_models_are_refused_with_named_codes ... ok
test systems_family::tests::a_precision_limited_real_is_refused_as_an_exact_model_input ... ok
test systems_family::tests::station_records_round_trip_into_a_network ... ok
test systems_family::tests::long_period_arrival_rate_survives_open_network ... ok
test systems_family::tests::the_same_source_is_deterministic ... ok
test systems_family::tests::mmc_erlang_c_and_infinite_server_are_exact ... ok
test systems_family::tests::mm1_metrics_are_exact_rationals ... ok
test systems_family::tests::long_period_think_time_survives_closed_and_mva ... ok
test systems_family::tests::long_period_service_time_survives_station_and_open ... ok
test systems_family::tests::sweeps_are_ordered_batches ... ok
test systems_family::tests::open_jackson_and_bottleneck_agree_with_the_hand_computation ... ok
test systems_family::tests::closed_mva_matches_the_golden_recurrence ... ok
test systems_family::tests::exact_values_cross_the_language_boundary_beside_their_decimal_projection ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 1727 filtered out; finished in 0.02s
EXITCODE=0
```

### 3.3 Whole `lovelace-lang` crate

```console
$ cargo test -p lovelace-lang
test result: ok. 1742 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.77s

     Running tests\cross_library.rs (...)
running 5 tests
...
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests lovelace_lang
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXITCODE=0
```

The preceding invocation of the same command hit a transient
`error[E0463]: can't find crate for lovelace_real/symbolics/systems` in the rustdoc phase while a
concurrent agent rebuilt dependencies; the immediate rerun above exited 0. The unit and
cross-library results were green in both runs.

### 3.4 Formatting

```console
$ cargo fmt --all -- --check
EXITCODE=0
```

No diff was printed. `rustfmt --edition 2024 --check` on the scoped file also exited 0 before this
workspace-wide run.

## 4. What could not be verified

- The literal `cargo test -p lovelace-lang systems_family long_period` is not a runnable cargo
  invocation on cargo 1.100.0 (observed rejection above). The equivalent single-filter commands
  are reported in §1.2 and §3.1.
- Only the requested `lovelace-lang` commands were run. `cargo test --workspace`, the corpus parity
  gate, `lovelace-run` wire/golden fixtures, and the documentation example gates were not run here.
  Adding the additive record fields changes the display/wire text of a full `Station` or
  `QueueNetwork` record; field-extracting corpus scripts (e.g. `parity/corpus/systems.json`
  `systems/012`) are unaffected in shape, but that was not re-run in this round.
- The malformed-sidecar regression test constructs the record in Rust and exercises
  `parse_station` directly. `parse_network` uses the same `exact_value`/`model_number` path but a
  malformed network sidecar is not separately tested.
- The workspace was shared with concurrently editing agents; the passing output above is the
  observed state at the time of the run, and no commit was made.
