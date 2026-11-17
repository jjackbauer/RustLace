//! The M0 scientific workloads. See `parity/workloads/README.md`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use lovelace_lang::Engine;

fn workload_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("parity")
        .join("workloads")
}

fn run_workload(name: &str) {
    let dir = workload_dir();
    let source = std::fs::read_to_string(dir.join(format!("{name}.ls")))
        .unwrap_or_else(|error| panic!("read {name}.ls: {error}"));
    let expected = std::fs::read_to_string(dir.join(format!("{name}.expected")))
        .unwrap_or_else(|error| panic!("read {name}.expected: {error}"));
    let mut engine = Engine::new();
    engine
        .eval(&source)
        .unwrap_or_else(|error| panic!("{name}: {}", error.message));
    let observed = engine.take_output();
    let expected: Vec<String> = expected.lines().map(str::to_string).collect();
    assert_eq!(observed, expected, "{name} output");
}

#[test]
fn wl01_precision_tier_stress() {
    run_workload("wl01_precision");
}

#[test]
fn wl02_exact_algebra_and_symbolic_numerical() {
    run_workload("wl02_exact_algebra");
}

/// WL-16, current-functionality portion: a cancelled engine evaluation returns the typed
/// `Cancelled` error and leaves the engine usable. Solver/array/import/save interruptions are not
/// implemented and are named in `parity/workloads/README.md`.
#[test]
fn wl16_cancellation_is_typed_and_leaves_the_engine_usable() {
    let mut engine = Engine::new();
    let error = engine
        .eval_with_budget(
            "c = 0\nwhile (c < 100000000) { c = c + 1 }\nc",
            Duration::from_millis(25),
        )
        .expect_err("the loop must be cancelled by the budget");
    assert_eq!(error.code, "Cancelled");
    assert_eq!(error.category, "BudgetExceeded");

    let value = engine
        .eval("1 + 1")
        .expect("the engine is usable after cancellation");
    assert_eq!(engine.format_typed(&value), "2 (Natural)");
}

/// WL-16, array-reduction portion: a large range reduction is cancellable through the same typed
/// error path. Streaming import and saved-workspace interruption are not engine workloads and
/// remain named limitations.
#[test]
fn wl16_array_reduction_is_cancellable() {
    let mut engine = Engine::new();
    let error = engine
        .eval_with_budget("sum(1..100000000)", Duration::from_millis(25))
        .expect_err("the reduction must be cancelled by the budget");
    assert_eq!(error.code, "Cancelled");
    assert_eq!(error.category, "BudgetExceeded");
}

/// WL-16, solver portion: a high-degree symbolic solve is cancellable and returns the typed
/// `Cancelled` error. The solve kernel polls the engine's budget while it works.
#[test]
fn wl16_solver_is_cancellable() {
    let mut engine = Engine::new();
    let error = engine
        .eval_with_budget(
            "x = symbol(\"x\")\nsolve(x^50 - 2 == 0, x)",
            Duration::from_millis(5),
        )
        .expect_err("the solve must be cancelled by the budget");
    assert_eq!(error.code, "Cancelled");
    assert_eq!(error.category, "BudgetExceeded");
}
