//! The `eig_sym` symmetric eigensolver: the Jacobi algorithm's acceptance criteria.
//!
//! Every test drives the public `Engine::eval` surface with a concrete matrix and asserts a
//! concrete value, error message or record field. The precision-tracking pair pins the algorithm's
//! central property: the residual follows the ambient computation precision.

use lovelace_real::Real;

use crate::registry;
use crate::{Engine, Value};

/// `10^-places`, the exact decimal tolerance the assertions compare against.
fn tolerance(places: usize) -> Real {
    Real::from_i64(10)
        .pow_int(-(places as i64))
        .expect("a power of ten has an exact reciprocal")
}

fn eval(source: &str) -> Value {
    Engine::new()
        .eval(source)
        .unwrap_or_else(|error| panic!("{source} must evaluate, got: {error}"))
}

fn field(record: &Value, name: &str) -> Value {
    match record {
        Value::Record(type_name, fields) => {
            assert_eq!(type_name, "EigenResult");
            fields
                .iter()
                .find(|(field, _)| field == name)
                .map(|(_, value)| value.clone())
                .unwrap_or_else(|| panic!("EigenResult must publish '{name}'"))
        }
        other => panic!("expected an EigenResult record, got {}", other.typed()),
    }
}

fn number(value: &Value) -> Real {
    match value {
        Value::Real(real) | Value::RealClamped(real, _, _) => real.clone(),
        other => panic!("expected a Real, got {}", other.typed()),
    }
}

fn eigenvalues(record: &Value) -> Vec<Real> {
    match field(record, "values") {
        Value::Vector(items) => items.iter().map(number).collect(),
        other => panic!("values must be a vector, got {}", other.typed()),
    }
}

fn sweeps(record: &Value) -> i64 {
    match field(record, "sweeps") {
        Value::Integer(value) => value
            .to_decimal()
            .parse()
            .expect("sweeps is a small integer"),
        other => panic!("sweeps must be an Integer, got {}", other.typed()),
    }
}

/// `vectors[i][j]` is component i of eigenvector j, as documented.
fn component(record: &Value, row: usize, column: usize) -> Real {
    match field(record, "vectors") {
        Value::Array(vectors) => vectors
            .get(&[row, column])
            .map(number)
            .unwrap_or_else(|| panic!("vectors has no entry [{row}][{column}]")),
        other => panic!("vectors must be an array, got {}", other.typed()),
    }
}

fn assert_close(actual: &Real, expected: &Real, tolerance: &Real, label: &str) {
    let difference = actual.sub(expected).abs();
    assert!(
        difference <= tolerance.clone(),
        "{label}: |{actual} - {expected}| = {difference} exceeds {tolerance}"
    );
}

fn assert_orthonormal(record: &Value, n: usize, tolerance: &Real) {
    for a in 0..n {
        for b in 0..n {
            let mut product = Real::zero();
            for i in 0..n {
                product = product.add(&component(record, i, a).mul(&component(record, i, b)));
            }
            let expected = if a == b {
                Real::from_i64(1)
            } else {
                Real::zero()
            };
            assert_close(&product, &expected, tolerance, &format!("v{a} . v{b}"));
        }
    }
}

#[test]
fn registry_descriptor_pins_the_declared_surface() {
    let info = registry::find("eig_sym").expect("eig_sym is registered");
    assert_eq!(info.parameters, &["a"]);
    assert_eq!(info.min_arity, 1);
    assert_eq!(info.parameter_count, 1);
    assert!(!info.variadic);
    assert_eq!(info.plugin, None);
    assert_eq!(info.category, "Linear Algebra");
    assert_eq!(
        info.summary,
        Some("Eigenvalues and eigenvectors of a real symmetric matrix by the Jacobi method.")
    );
    assert_eq!(info.return_kind, Some("EigenResult"));
    assert_eq!(info.related, &["det", "inv", "matrix_rank", "trace"]);
    assert_eq!(info.examples, &["eig_sym([[2, 1], [1, 2]])"]);
}

#[test]
fn two_by_two_returns_one_and_three_with_orthonormal_vectors() {
    let result = eval("setprecision(30); eig_sym([[2, 1], [1, 2]])");
    assert_eq!(
        eigenvalues(&result),
        vec![Real::from_i64(1), Real::from_i64(3)]
    );
    assert_orthonormal(&result, 2, &tolerance(25));
    assert!(number(&field(&result, "residual")) <= tolerance(28));
    assert_eq!(sweeps(&result), 1);
    assert_eq!(field(&result, "exact"), Value::Boolean(false));
}

#[test]
fn three_by_three_block_returns_two_three_four() {
    let result = eval("setprecision(30); eig_sym([[3, 1, 0], [1, 3, 0], [0, 0, 3]])");
    assert_eq!(
        eigenvalues(&result),
        vec![Real::from_i64(2), Real::from_i64(3), Real::from_i64(4)]
    );
    assert_orthonormal(&result, 3, &tolerance(25));
    assert!(number(&field(&result, "residual")) <= tolerance(28));
}

#[test]
fn five_by_five_householder_matrix_has_spectrum_one_to_five() {
    // Q = I - (2/5)J, A = Q diag(1..5) Q: dense, non-diagonal, exact decimal entries, spectrum
    // exactly 1, 2, 3, 4, 5. The prototype's Route B matrix.
    let result = eval(
        "setprecision(30); eig_sym([[2.6, 1.2, 0.8, 0.4, 0], [1.2, 2.8, 0.4, 0, -0.4], [0.8, 0.4, 3, -0.4, -0.8], [0.4, 0, -0.4, 3.2, -1.2], [0, -0.4, -0.8, -1.2, 3.4]])",
    );
    let values = eigenvalues(&result);
    assert_eq!(values.len(), 5);
    for (index, value) in values.iter().enumerate() {
        assert_close(
            value,
            &Real::from_i64(index as i64 + 1),
            &tolerance(25),
            &format!("eigenvalue {index}"),
        );
    }
    assert!(number(&field(&result, "residual")) <= tolerance(28));
    assert!((1..=20).contains(&sweeps(&result)));
}

#[test]
fn one_by_one_matrix_is_the_trivial_case() {
    let result = eval("setprecision(30); eig_sym([[7]])");
    assert_eq!(eigenvalues(&result), vec![Real::from_i64(7)]);
    assert_eq!(component(&result, 0, 0), Real::from_i64(1));
    assert_eq!(sweeps(&result), 0);
    assert_eq!(number(&field(&result, "residual")), Real::zero());
    assert_eq!(field(&result, "exact"), Value::Boolean(false));
}

#[test]
fn residual_tracks_the_ambient_precision() {
    let at_thirty = eval("setprecision(30); eig_sym([[2, 1], [1, 2]])");
    let at_fifty = eval("setprecision(50); eig_sym([[2, 1], [1, 2]])");
    let thirty = number(&field(&at_thirty, "residual"));
    let fifty = number(&field(&at_fifty, "residual"));
    assert!(
        thirty <= tolerance(28),
        "p30 residual {thirty} exceeds 1e-28"
    );
    assert!(fifty <= tolerance(48), "p50 residual {fifty} exceeds 1e-48");
    assert!(
        fifty <= thirty,
        "the p50 residual {fifty} must not exceed the p30 residual {thirty}"
    );
}

#[test]
fn non_square_matrix_is_refused() {
    let error = Engine::new()
        .eval("eig_sym([[1, 2, 3], [4, 5, 6]])")
        .expect_err("a 2x3 matrix has no symmetric eigensystem");
    assert!(
        error.message.contains("square matrix"),
        "unexpected message: {}",
        error.message
    );
    assert_eq!(error.code, "InvalidArgument");
    assert_eq!(error.category, "TypeMismatch");
}

#[test]
fn non_symmetric_matrix_is_refused() {
    let error = Engine::new()
        .eval("eig_sym([[1, 2], [0, 1]])")
        .expect_err("an asymmetric matrix is not in the solver's domain");
    assert!(
        error.message.contains("symmetric matrix"),
        "unexpected message: {}",
        error.message
    );
    assert_eq!(error.code, "InvalidArgument");
}

#[test]
fn complex_entries_are_refused_as_real_only() {
    let error = Engine::new()
        .eval("eig_sym([[evalf(sqrt(-1), 30), 0], [0, 1]])")
        .expect_err("the symmetric solver is real-only");
    assert!(
        error.message.contains("complex entries are not supported"),
        "unexpected message: {}",
        error.message
    );
    assert_eq!(error.code, "InvalidArgument");
}

#[test]
fn empty_matrix_is_refused() {
    let error = Engine::new()
        .eval("eig_sym(zeros(0, 0))")
        .expect_err("a 0x0 matrix has no eigenvalues");
    assert!(
        error.message.contains("non-empty"),
        "unexpected message: {}",
        error.message
    );
    assert_eq!(error.code, "InvalidArgument");
}

#[test]
fn repeated_runs_are_bit_identical() {
    let source = "setprecision(30); eig_sym([[4, 1], [1, 3]])";
    let first = eval(source);
    let second = eval(source);
    assert_eq!(first, second);
}

#[test]
fn diagonal_matrix_is_sorted_with_permuted_identity_vectors_and_no_sweeps() {
    let result = eval("setprecision(30); eig_sym([[3, 0], [0, 1]])");
    assert_eq!(
        eigenvalues(&result),
        vec![Real::from_i64(1), Real::from_i64(3)]
    );
    assert_eq!(sweeps(&result), 0);
    // Eigenvalue 1 keeps e1, eigenvalue 3 keeps e0: vectors[i][j] is component i of eigenvector j.
    assert_eq!(component(&result, 0, 0), Real::zero());
    assert_eq!(component(&result, 0, 1), Real::from_i64(1));
    assert_eq!(component(&result, 1, 0), Real::from_i64(1));
    assert_eq!(component(&result, 1, 1), Real::zero());
    assert_eq!(number(&field(&result, "residual")), Real::zero());
}
