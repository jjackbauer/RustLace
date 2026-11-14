//! Round-66 engine data: the two rows that were blocked only by missing engine data.
//!
//! RUN-005 — kernel sub-progress. [`crate::progress`] is the observer a long-running kernel
//! reports a human-readable phase plus fraction to, armed per evaluation by
//! [`crate::Engine::set_progress_sink`] exactly like the live cancellation flag. These tests pin
//! the observed sequence: increasing fractions, named phases, the sqrt/pi/factorial kernels that
//! produce them, the unchanged result on the unobserved path, and the measured cost on one input.
//!
//! SYM-005 — per-step applicability. `simplify_full`'s TransformResult publishes each applied
//! step's `applicability` and a `RuleAttempt` record (reference applicability + reason) for every
//! rule it tried and rejected. These tests pin the met case (the known `assume(x > 0)` +
//! `simplify_full(sqrt(x^2))` row), the unmet case (the reason, not just "not applied"), the
//! universal case, and the wire contract that `diagnostics` stays the last field.

use std::sync::{Arc, Mutex};

use super::*;

/// One reported (phase, fraction) pair.
type Report = (String, f32);

/// Collects the reports an armed sink observes.
fn collector() -> (crate::progress::Sink, Arc<Mutex<Vec<Report>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink: crate::progress::Sink = {
        let seen = Arc::clone(&seen);
        Arc::new(move |phase: &str, fraction: f32| {
            seen.lock()
                .unwrap_or_else(|error| error.into_inner())
                .push((phase.to_string(), fraction));
        })
    };
    (sink, seen)
}

fn reports(seen: &Arc<Mutex<Vec<Report>>>) -> Vec<Report> {
    seen.lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

/// Evaluates source on an engine whose kernel sub-progress observer is armed, returning the
/// committed value's display form alongside the reports the sink observed.
fn eval_observed(engine: &mut Engine, source: &str) -> (String, Vec<Report>) {
    let (sink, seen) = collector();
    engine.set_progress_sink(Some(sink));
    let value = engine.eval(source).expect("the statement must evaluate");
    let display = engine.format(&value);
    // The observer is per evaluation: after it returns, the thread-local slot is empty again.
    assert!(
        !crate::progress::active(),
        "the sink must not survive the evaluation that armed it"
    );
    (display, reports(&seen))
}

fn engine_with_precision(precision: usize) -> Engine {
    let mut engine = Engine::new();
    engine.set_precision(precision);
    engine
}

fn assert_monotone(seen: &[Report]) {
    for window in seen.windows(2) {
        assert!(
            window[1].1 >= window[0].1,
            "the observed fraction must never move backwards: {seen:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// RUN-005 — kernel sub-progress
// ---------------------------------------------------------------------------

/// The row's required proof: a sqrt run emits intermediate reports with named phases and a
/// fraction that increases (never the committed `1/1` label the desktop used to be alone with).
#[test]
fn a_sqrt_kernel_reports_phases_whose_fraction_increases() {
    let mut engine = engine_with_precision(2000);
    let (display, seen) = eval_observed(&mut engine, "sqrt(2)");
    assert!(
        display.starts_with("1.4142"),
        "sqrt(2) rendered as {display}"
    );
    assert!(
        seen.len() >= 2,
        "a long sqrt must publish at least one intermediate report, saw {seen:?}"
    );
    let phases: Vec<&str> = seen.iter().map(|(phase, _)| phase.as_str()).collect();
    assert_eq!(
        phases,
        vec![
            "sqrt:operand",
            "sqrt:integer-square-root",
            "sqrt:truncate"
        ],
        "the sqrt kernel's phases, in order"
    );
    assert_monotone(&seen);
    assert!(
        seen.first().map(|(_, fraction)| *fraction) < seen.last().map(|(_, fraction)| *fraction),
        "the fraction must increase across the kernel: {seen:?}"
    );
    assert_eq!(
        *seen.last().map(|(_, fraction)| fraction).unwrap(),
        0.90,
        "the last phase the kernel reports before the commit is its truncation"
    );
    println!(
        "RUN-005 sequence: sqrt(2) at 2000 digits -> {:?}",
        seen.iter()
            .map(|(phase, fraction)| format!("{phase} {:.0}%", fraction * 100.0))
            .collect::<Vec<String>>()
    );
}

/// π has the same two-stage shape, and the report names the arctangent series it is inside.
#[test]
fn a_pi_kernel_reports_its_series_and_truncation_phases() {
    let mut engine = engine_with_precision(300);
    let (display, seen) = eval_observed(&mut engine, "pi(120)");
    assert!(
        display.starts_with("3.14159"),
        "pi(120) rendered as {display}"
    );
    let phases: Vec<&str> = seen.iter().map(|(phase, _)| phase.as_str()).collect();
    assert_eq!(
        phases,
        vec!["pi:machin-arctan", "pi:truncate"],
        "the pi kernel's phases, in order"
    );
    assert!(
        seen[0].1 < seen[1].1,
        "the fraction must increase across the kernel: {seen:?}"
    );
}

/// A kernel the language crate OWNS iterates in a loop, so its reports are real in-kernel
/// progress: `n!`'s product tree reports the fraction of the range it has multiplied, strided.
#[test]
fn a_factorial_kernel_reports_in_kernel_product_tree_progress() {
    let mut engine = Engine::new();
    let (display, seen) = eval_observed(&mut engine, "200000!");
    assert!(
        display.len() > 1000,
        "200000! must publish its full value, not a label"
    );
    assert!(
        seen.len() > 10,
        "the product tree must report many times, saw {} report(s)",
        seen.len()
    );
    for (phase, _) in &seen {
        assert_eq!(phase, "factorial:product-tree");
    }
    assert_monotone(&seen);
    let last = seen.last().map(|(_, fraction)| *fraction).unwrap_or(0.0);
    assert!(
        last > 0.9,
        "the last in-kernel report must be near the end of the range, was {last}"
    );
    println!(
        "RUN-005 sequence: 200000! -> {} in-kernel reports, first {:?} last {:?}",
        seen.len(),
        seen.first().map(|(_, fraction)| *fraction),
        seen.last().map(|(_, fraction)| *fraction)
    );
}

/// The same binary exponentiation the cancellation rows drive reports per squaring, because its
/// ~30 iterations are each a full-width product (a stride would never fire).
#[test]
fn a_binary_exponentiation_reports_per_squaring() {
    let mut engine = Engine::new();
    let (display, seen) = eval_observed(&mut engine, "2^4096");
    let digits = display.chars().filter(char::is_ascii_digit).count();
    assert!(
        (1230..=1240).contains(&digits),
        "2^4096 has 1234 decimal digits, rendered {digits}"
    );
    assert!(seen.len() >= 8, "saw {} report(s)", seen.len());
    for (phase, _) in &seen {
        assert_eq!(phase, "pow:binary-exponentiation");
    }
    assert_monotone(&seen);
}

/// The unobserved path is unchanged: the same input, the same value, and no observer left armed.
#[test]
fn an_unobserved_run_produces_the_same_value_and_reports_nothing() {
    let mut observed = engine_with_precision(2000);
    let (observed_display, seen) = eval_observed(&mut observed, "sqrt(2)");
    assert!(!seen.is_empty());
    let mut plain = engine_with_precision(2000);
    let value = plain.eval("sqrt(2)").expect("sqrt evaluates");
    assert_eq!(
        plain.format(&value),
        observed_display,
        "an armed sink must not change the value"
    );
    assert!(!crate::progress::active());
}

/// RUN-005 cost: the same input measured with the sink armed and unarmed. The sink costs a
/// callback per phase (three for sqrt), so the kernel's own time must be indistinguishable at both
/// a small and a kernel-sized precision.
#[test]
fn the_progress_sink_costs_nothing_measurable_on_the_same_input() {
    const REPEATS: usize = 9;
    let mut unarmed = engine_with_precision(2000);
    let unarmed_start = std::time::Instant::now();
    for _ in 0..REPEATS {
        unarmed.eval("sqrt(2)").expect("sqrt evaluates");
    }
    let unarmed_elapsed = unarmed_start.elapsed();

    let mut armed = engine_with_precision(2000);
    let (sink, seen) = collector();
    armed.set_progress_sink(Some(sink));
    let armed_start = std::time::Instant::now();
    for _ in 0..REPEATS {
        armed.eval("sqrt(2)").expect("sqrt evaluates");
    }
    let armed_elapsed = armed_start.elapsed();
    let observed = reports(&seen).len();
    assert_eq!(
        observed,
        REPEATS * 3,
        "every run must deliver its three reports; saw {observed}"
    );

    // The proof-size input: one 50 000-digit root with and without the observer. This is the shape
    // the desktop probe measures (a ~1 s kernel in the debug profile), so the cost is reported on
    // the same work the row actually shows sub-progress for.
    let mut large_unarmed = engine_with_precision(50_000);
    let large_unarmed_start = std::time::Instant::now();
    large_unarmed.eval("sqrt(2)").expect("sqrt evaluates");
    let large_unarmed_elapsed = large_unarmed_start.elapsed();
    let mut large_armed = engine_with_precision(50_000);
    let (large_sink, large_seen) = collector();
    large_armed.set_progress_sink(Some(large_sink));
    let large_armed_start = std::time::Instant::now();
    large_armed.eval("sqrt(2)").expect("sqrt evaluates");
    let large_armed_elapsed = large_armed_start.elapsed();
    assert_eq!(reports(&large_seen).len(), 3);

    println!(
        "RUN-005 cost: sqrt(2) at 2000 digits x{REPEATS}: unarmed {:.3} ms, armed {:.3} ms \
         (+{:.1} %), {observed} callbacks; sqrt(2) at 50000 digits: unarmed {:.1} ms, armed \
         {:.1} ms ({:+.1} %)",
        unarmed_elapsed.as_secs_f64() * 1000.0,
        armed_elapsed.as_secs_f64() * 1000.0,
        (armed_elapsed.as_secs_f64() / unarmed_elapsed.as_secs_f64() - 1.0) * 100.0,
        large_unarmed_elapsed.as_secs_f64() * 1000.0,
        large_armed_elapsed.as_secs_f64() * 1000.0,
        (large_armed_elapsed.as_secs_f64() / large_unarmed_elapsed.as_secs_f64() - 1.0) * 100.0
    );
    assert!(
        armed_elapsed <= unarmed_elapsed * 2 + std::time::Duration::from_millis(25),
        "the sink must not measurably slow the kernel: unarmed {unarmed_elapsed:?}, \
         armed {armed_elapsed:?}"
    );
    assert!(
        large_armed_elapsed
            <= large_unarmed_elapsed * 2 + std::time::Duration::from_millis(250),
        "the sink must not measurably slow the 50 000-digit kernel: unarmed \
         {large_unarmed_elapsed:?}, armed {large_armed_elapsed:?}"
    );
}

/// A sink is armed by the EVALUATION, not installed globally: a second engine on the same thread
/// starts unobserved, and the first engine's sink is not reached by it.
#[test]
fn the_observer_is_scoped_to_one_evaluation() {
    let mut outer = engine_with_precision(500);
    let (sink, seen) = collector();
    outer.set_progress_sink(Some(sink));
    outer.eval("sqrt(2)").expect("sqrt evaluates");
    assert!(
        !crate::progress::active(),
        "the slot is empty between evaluations"
    );
    let mut inner = engine_with_precision(500);
    inner.eval("sqrt(2)").expect("sqrt evaluates");
    assert_eq!(
        reports(&seen).len(),
        3,
        "the second engine's evaluation must not reach the first engine's sink"
    );
}

// ---------------------------------------------------------------------------
// SYM-005 — per-step applicability
// ---------------------------------------------------------------------------

fn record_fields<'a>(value: &'a Value, expected: &str) -> &'a [(String, Value)] {
    match value {
        Value::Record(type_name, fields) => {
            assert_eq!(type_name, expected);
            fields
        }
        other => panic!("expected a {expected} record, got {}", other.kind_name()),
    }
}

fn field<'a>(fields: &'a [(String, Value)], name: &str) -> &'a Value {
    fields
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value)
        .unwrap_or_else(|| panic!("no field {name} in {fields:?}"))
}

fn vector(value: &Value) -> &[Value] {
    match value {
        Value::Vector(values) => values,
        other => panic!("expected a Vector, got {}", other.kind_name()),
    }
}

fn variant(value: &Value) -> String {
    match value {
        Value::Enum(_, variant) => variant.clone(),
        other => panic!("expected an Enum, got {}", other.kind_name()),
    }
}

fn text(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        other => panic!("expected Text, got {}", other.kind_name()),
    }
}

fn define_x() -> Engine {
    let mut engine = Engine::new();
    engine.eval("x = symbol(\"x\")").expect("define x");
    engine
}

/// The known SYM-005 case: under `assume(x > 0)`, `simplify_full(sqrt(x^2))` fires the conditional
/// rule. The step now says its preconditions were MET, with the condition text that licenses it.
#[test]
fn a_met_precondition_publishes_applicability_and_the_condition_text() {
    let mut engine = define_x();
    engine.eval("assume(x > 0)").expect("assume x > 0");
    let result = engine
        .eval("simplify_full(sqrt(x^2))")
        .expect("simplify_full evaluates");
    let fields = record_fields(&result, "TransformResult");
    assert_eq!(
        engine.format(field(fields, "expression")),
        "x",
        "the conditional rule must still rewrite to x"
    );
    let steps = vector(field(fields, "steps"));
    assert_eq!(steps.len(), 1);
    let step = record_fields(&steps[0], "RewriteStep");
    assert_eq!(text(field(step, "rule_id")), "pow.sqrt-square-nonnegative");
    assert_eq!(variant(field(step, "classification")), "Conditional");
    assert_eq!(
        variant(field(step, "applicability")),
        "ApplicableWithConditions",
        "a rewrite carrying conditions is not unconditionally applicable"
    );
    assert_eq!(
        field(step, "required_conditions").display(),
        "[x >= 0]",
        "the condition the rewrite requires must be the published condition text"
    );
    assert!(
        vector(field(fields, "attempts")).is_empty(),
        "a rule that fired produced no rejection"
    );
    println!(
        "SYM-005 met: rule={} classification={} applicability={} conditions={}",
        text(field(step, "rule_id")),
        variant(field(step, "classification")),
        variant(field(step, "applicability")),
        field(step, "required_conditions").display()
    );
}

/// The unmet case: with no assumption the same expression is left alone, and the result now says
/// WHY each rule was rejected instead of publishing an empty trace.
#[test]
fn an_unmet_precondition_publishes_the_rule_and_the_reason() {
    let mut engine = define_x();
    let result = engine
        .eval("simplify_full(sqrt(x^2))")
        .expect("simplify_full evaluates");
    let fields = record_fields(&result, "TransformResult");
    assert_eq!(engine.format(field(fields, "expression")), "sqrt(x^2)");
    assert!(
        vector(field(fields, "steps")).is_empty(),
        "without an assumption neither power rule may fire"
    );
    let attempts = vector(field(fields, "attempts"));
    assert_eq!(attempts.len(), 2, "both sqrt rules were tried: {attempts:?}");
    let first = record_fields(&attempts[0], "RuleAttempt");
    assert_eq!(text(field(first, "rule_id")), "pow.sqrt-square-nonnegative");
    assert!(matches!(
        field(first, "pattern_matched"),
        Value::Boolean(true)
    ));
    assert_eq!(variant(field(first, "precondition")), "NotApplicable");
    assert!(
        text(field(first, "rejection_reason")).contains("not known to be nonnegative"),
        "the reason must name the unmet premise, was {:?}",
        text(field(first, "rejection_reason"))
    );
    let second = record_fields(&attempts[1], "RuleAttempt");
    assert_eq!(text(field(second, "rule_id")), "pow.sqrt-square-real");
    assert_eq!(variant(field(second, "precondition")), "NotApplicable");
    assert!(text(field(second, "rejection_reason")).contains("not known to be real"));
    for attempt in attempts {
        let attempt = record_fields(attempt, "RuleAttempt");
        println!(
            "SYM-005 unmet: rule={} pattern_matched={} precondition={} reason={}",
            text(field(attempt, "rule_id")),
            field(attempt, "pattern_matched").display(),
            variant(field(attempt, "precondition")),
            text(field(attempt, "rejection_reason"))
        );
    }
}

/// The domain-specific rule's met case: a real symbol fires `pow.sqrt-square-real`, conditioned on
/// the domain assumption.
#[test]
fn a_domain_specific_met_precondition_publishes_its_domain_condition() {
    let mut engine = Engine::new();
    engine.eval("y = symbol(\"y\", real)").expect("define y");
    let result = engine
        .eval("simplify_full(sqrt(y^2))")
        .expect("simplify_full evaluates");
    let fields = record_fields(&result, "TransformResult");
    assert_eq!(engine.format(field(fields, "expression")), "abs(y)");
    let steps = vector(field(fields, "steps"));
    let step = record_fields(&steps[0], "RewriteStep");
    assert_eq!(text(field(step, "rule_id")), "pow.sqrt-square-real");
    assert_eq!(variant(field(step, "classification")), "DomainSpecific");
    assert_eq!(
        variant(field(step, "applicability")),
        "ApplicableWithConditions"
    );
    assert!(field(step, "required_conditions").display().contains("real"));
    assert!(vector(field(fields, "attempts")).is_empty());
}

/// A universal rewrite reports plain `Applicable`, and a conditional one that fired reports the
/// conditioned value: the two are distinguishable on the wire.
#[test]
fn universal_and_conditional_rewrites_publish_different_applicabilities() {
    let mut engine = define_x();
    let identity = engine
        .eval("simplify_full(sin(x)^2 + cos(x)^2)")
        .expect("simplify_full evaluates");
    let fields = record_fields(&identity, "TransformResult");
    let step = record_fields(&vector(field(fields, "steps"))[0], "RewriteStep");
    assert_eq!(text(field(step, "rule_id")), "trig.pythagorean-sin2-cos2");
    assert_eq!(variant(field(step, "applicability")), "Applicable");

    let cancel = engine
        .eval("simplify_full(x/x)")
        .expect("simplify_full evaluates");
    let fields = record_fields(&cancel, "TransformResult");
    let step = record_fields(&vector(field(fields, "steps"))[0], "RewriteStep");
    assert_eq!(text(field(step, "rule_id")), "rat.cancel-x-over-x");
    assert_eq!(
        variant(field(step, "applicability")),
        "ApplicableWithConditions"
    );
}

/// A reciprocal factor with no numerator to cancel against is rejected with the factor named.
#[test]
fn a_rat_rejection_names_the_unmatched_reciprocal() {
    let mut engine = define_x();
    engine.eval("y = symbol(\"y\")").expect("define y");
    let result = engine
        .eval("simplify_full(x^-1 * y)")
        .expect("simplify_full evaluates");
    let fields = record_fields(&result, "TransformResult");
    let attempts = vector(field(fields, "attempts"));
    assert!(!attempts.is_empty(), "the rat rule matched and was rejected");
    let attempt = record_fields(&attempts[0], "RuleAttempt");
    assert_eq!(text(field(attempt, "rule_id")), "rat.cancel-x-over-x");
    assert_eq!(variant(field(attempt, "precondition")), "NotApplicable");
    assert!(text(field(attempt, "rejection_reason")).contains("x^-1"));
}

/// The catalog of applicability values the engine can publish, in the reference's spelling.
#[test]
fn the_published_applicability_vocabulary_is_the_reference_enum() {
    use lovelace_symbolics::rewrite_registry::RuleApplicability;
    assert_eq!(RuleApplicability::Applicable.name(), "Applicable");
    assert_eq!(
        RuleApplicability::ApplicableWithConditions.name(),
        "ApplicableWithConditions"
    );
    assert_eq!(RuleApplicability::NotApplicable.name(), "NotApplicable");
    assert_eq!(RuleApplicability::Unknown.name(), "Unknown");
}

/// The diagnostics-last wire contract survives the additive fields: `attempts` is inserted before
/// `budget_exceeded`, so `diagnostics` remains the final field of every TransformResult.
#[test]
fn the_transform_record_keeps_diagnostics_last_with_the_new_fields() {
    let mut engine = define_x();
    engine.eval("assume(x > 0)").expect("assume x > 0");
    let result = engine
        .eval("simplify_full(sqrt(x^2))")
        .expect("simplify_full evaluates");
    let fields = record_fields(&result, "TransformResult");
    let names: Vec<&str> = fields.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names.last().copied(),
        Some("diagnostics"),
        "diagnostics must stay the last field: {names:?}"
    );
    assert!(
        names.contains(&"attempts"),
        "the new field is present: {names:?}"
    );
    // And the step record's applicability is a real enum, not a stringly-typed guess.
    let step = record_fields(&vector(field(fields, "steps"))[0], "RewriteStep");
    assert!(matches!(field(step, "applicability"), Value::Enum(_, _)));
}
