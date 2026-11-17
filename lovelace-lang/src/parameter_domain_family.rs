//! ParameterDomain on the solution-family wire field.
//!
//! The deferred census row `ParameterDomainOf_MapsEveryDeclaredArm#0` is exercised directly by
//! `lovelace-symbolics`' own tests. The independent guarantee kept here is the language side: the
//! `SolutionFamily` record `solve_full` emits publishes the declared integer domain rather than a
//! hard-coded string. (The Modus payload projection this module once also covered is deleted with
//! the extension host; see `docs/goal-systems/deleted-tests.md`.)

use crate::{Engine, Value};

/// The solution family's wire field is the declared `Integers` domain every kernel family carries.
#[test]
fn solve_full_family_parameter_domain_is_the_projected_integer_domain() {
    let mut engine = Engine::new();
    engine.eval(r#"x = symbol("x")"#).expect("symbol");
    let solved = engine
        .eval("solve_full(sin(x) == 0, x)")
        .expect("solve_full");
    let Value::Record(type_name, fields) = &solved else {
        panic!("expected a SolveResult record, got {}", solved.kind_name());
    };
    assert_eq!(type_name, "SolveResult");

    let families = fields
        .iter()
        .find(|(name, _)| name == "families")
        .map(|(_, value)| value)
        .expect("families field");
    let Value::Vector(families) = families else {
        panic!("families must be a vector, got {}", families.kind_name());
    };
    let Some(Value::Record(_, family_fields)) = families.first() else {
        panic!("expected one SolutionFamily record");
    };
    let parameter_domain = family_fields
        .iter()
        .find(|(name, _)| name == "parameter_domain")
        .map(|(_, value)| value)
        .expect("parameter_domain field");
    assert_eq!(
        parameter_domain,
        &Value::Domain("integer".to_string()),
        "the family's parameter_domain must be the declared Domain value"
    );
}
