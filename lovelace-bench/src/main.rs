//! Micro-benchmark harness shaped like the C# reference harness, so the two can be compared
//! on one machine with the same operations and the same operand sizes.
//!
//! Usage: lovelace-bench <op> <digits> [reps]
//!   op: add | sub | mul | div | factorial | tostring | parse | pi | sqrt | eig_sym_5 | eig_sym_8
//!       | systems_mmc | systems_jackson | systems_mva
//!       | rational_add | rational_mul | rational_div_neg | rational_pow | rational_normalize
//!       | symbolics_diff
//! Output: "ENV ..." (host, compiler, profile, sizes, repetitions), "RESULT <op> <digits>
//! <mean_ms> <median_ms>" and a DIGEST line so two runs can be compared byte for byte.

#![forbid(unsafe_code)]

use std::time::Instant;

use lovelace_int::Int;
use lovelace_lang::{Engine, Value};
use lovelace_nat::Nat;
use lovelace_rational::{Limits as RatLimits, NeverCancel as RatNeverCancel, Rat};
use lovelace_real::Real;
use lovelace_systems::{
    NeverCancel, Q, QueueNetwork, ServiceDiscipline, SolverOptions, Station, Workload, analyze, mmc,
};

fn random_digits(n: usize, seed: u64) -> String {
    let mut state = seed | 1;
    let mut out = String::with_capacity(n);
    for i in 0..n {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let digit = (state % 10) as u8;
        if i == 0 {
            out.push((b'1' + (digit % 9)) as char);
        } else {
            out.push((b'0' + digit) as char);
        }
    }
    out
}

enum Work {
    Add,
    Sub,
    Mul,
    Div,
    Factorial,
    ToString,
    ToStringCold,
    Parse,
    Pi,
    Sqrt,
    EigSym5,
    EigSym8,
    SystemsMmc,
    SystemsJackson,
    SystemsMva,
    RationalAdd,
    RationalMul,
    RationalDivNeg,
    RationalPow,
    RationalNormalize,
    SymbolicsDiff,
}

impl Work {
    fn parse(op: &str) -> Option<Work> {
        Some(match op {
            "add" => Work::Add,
            "sub" => Work::Sub,
            "mul" => Work::Mul,
            "div" => Work::Div,
            "factorial" => Work::Factorial,
            "tostring" => Work::ToString,
            "tostring_cold" => Work::ToStringCold,
            "parse" => Work::Parse,
            "pi" => Work::Pi,
            "sqrt" => Work::Sqrt,
            "eig_sym_5" => Work::EigSym5,
            "eig_sym_8" => Work::EigSym8,
            "systems_mmc" => Work::SystemsMmc,
            "systems_jackson" => Work::SystemsJackson,
            "systems_mva" => Work::SystemsMva,
            "rational_add" => Work::RationalAdd,
            "rational_mul" => Work::RationalMul,
            "rational_div_neg" => Work::RationalDivNeg,
            "rational_pow" => Work::RationalPow,
            "rational_normalize" => Work::RationalNormalize,
            "symbolics_diff" => Work::SymbolicsDiff,
            _ => return None,
        })
    }

    /// Runs the operation once. Returns a digest of the result for the last call, so a caller
    /// can compare runs without timing the conversion. `engine` is reused across repetitions for
    /// the engine-level cases (the warm-up creates it, so construction is outside the timed
    /// region); the scalar cases ignore it.
    fn run(
        &self,
        digits: usize,
        a: &Nat,
        b: &Nat,
        operands: (&Rat, &Rat),
        want_digest: bool,
        engine: &mut Option<Engine>,
    ) -> Option<String> {
        let (left, right) = operands;
        match self {
            Work::Add => {
                let _ = std::hint::black_box(a.add(b));
                None
            }
            Work::Sub => {
                let (big, small) = if a.cmp(b) == std::cmp::Ordering::Greater {
                    (a.clone(), b.clone())
                } else {
                    (b.clone(), a.clone())
                };
                let _ = std::hint::black_box(big.sub(&small));
                None
            }
            Work::Mul => {
                let value = std::hint::black_box(a.mul(b));
                if want_digest {
                    Some(value.to_decimal())
                } else {
                    None
                }
            }
            Work::Div => {
                let divisor =
                    Nat::from_decimal(&random_digits((digits / 4).max(1), 99999)).expect("c");
                let pair = std::hint::black_box(a.div_rem(&divisor));
                if want_digest {
                    pair.map(|(q, _)| q.to_decimal())
                } else {
                    None
                }
            }
            Work::Factorial => {
                let value = std::hint::black_box(Nat::factorial(digits as u64));
                if want_digest {
                    Some(value.to_decimal())
                } else {
                    None
                }
            }
            Work::ToString => {
                let text = std::hint::black_box(a.to_decimal());
                if want_digest { Some(text) } else { None }
            }
            Work::ToStringCold => {
                // Force a value that exists only as limbs (arithmetic materialises them), so the
                // rendering below is a real conversion rather than a string already in hand.
                let fresh = a.add(&Nat::from_u64(1));
                let text = std::hint::black_box(fresh.to_decimal());
                if want_digest { Some(text) } else { None }
            }
            Work::Parse => {
                let text = random_digits(digits, 4242);
                let value = std::hint::black_box(Nat::from_decimal(&text).unwrap());
                if want_digest {
                    Some(value.to_decimal())
                } else {
                    None
                }
            }
            Work::Pi => {
                let value = std::hint::black_box(Real::pi(digits)).to_string();
                if want_digest { Some(value) } else { None }
            }
            Work::Sqrt => {
                let value = std::hint::black_box(Real::from_i64(2).sqrt(digits)).to_string();
                if want_digest { Some(value) } else { None }
            }
            Work::EigSym5 | Work::EigSym8 => {
                let n = if matches!(self, Work::EigSym5) { 5 } else { 8 };
                if engine.is_none() {
                    let mut fresh = Engine::new();
                    fresh.set_precision(digits);
                    *engine = Some(fresh);
                }
                let engine = engine.as_mut().expect("the engine was just built");
                let source = format!("eig_sym({})", symmetric_matrix_source(n));
                let value = std::hint::black_box(engine.eval(&source).expect("eig_sym evaluates"));
                if want_digest {
                    Some(eigen_digest(&value))
                } else {
                    None
                }
            }
            Work::SystemsMmc => {
                let options = systems_options(digits);
                let result = std::hint::black_box(
                    mmc(&Q::from_i64(8), &Q::from_i64(5), 2, &options, &NeverCancel)
                        .expect("stable M/M/2"),
                );
                if want_digest {
                    let station = &result.stations[0];
                    Some(format!(
                        "{}|{}|{}",
                        station
                            .probability_waiting
                            .as_ref()
                            .map(|value| rational_decimal(value, digits))
                            .unwrap_or_default(),
                        rational_decimal(&station.utilization, digits),
                        rational_decimal(&station.response_time, digits)
                    ))
                } else {
                    None
                }
            }
            Work::SystemsJackson => {
                let options = systems_options(digits);
                let network = QueueNetwork::new(
                    "bench",
                    Workload::Open(lovelace_systems::OpenWorkload {
                        arrival_rate: Q::from_i64(20),
                    }),
                    vec![
                        Station::new(
                            "api",
                            ServiceDiscipline::FcfsSingleServer,
                            ratio(1, 50),
                            Q::one(),
                        )
                        .expect("station"),
                        Station::new(
                            "db",
                            ServiceDiscipline::FcfsSingleServer,
                            ratio(1, 100),
                            Q::from_i64(2),
                        )
                        .expect("station"),
                        Station::new(
                            "search",
                            ServiceDiscipline::FcfsSingleServer,
                            ratio(1, 200),
                            Q::from_i64(3),
                        )
                        .expect("station"),
                        Station::new("cache", ServiceDiscipline::Delay, ratio(1, 20), Q::one())
                            .expect("station"),
                    ],
                )
                .expect("valid open network");
                let result = std::hint::black_box(
                    analyze(&network, &options, &NeverCancel).expect("solved"),
                );
                if want_digest {
                    Some(format!(
                        "{}|{}|{}",
                        rational_decimal(&result.aggregate.response_time, digits),
                        rational_decimal(&result.aggregate.occupancy, digits),
                        rational_decimal(&result.stations[1].utilization, digits)
                    ))
                } else {
                    None
                }
            }
            Work::RationalAdd => {
                let value = std::hint::black_box(left.add(right));
                if want_digest {
                    Some(canonical_rat_digest(&value))
                } else {
                    None
                }
            }
            Work::RationalMul => {
                let value = std::hint::black_box(left.mul(right));
                if want_digest {
                    Some(canonical_rat_digest(&value))
                } else {
                    None
                }
            }
            Work::RationalDivNeg => {
                let value = std::hint::black_box(
                    left.checked_div(&right.neg())
                        .expect("a non-zero generated divisor"),
                );
                if want_digest {
                    Some(canonical_rat_digest(&value))
                } else {
                    None
                }
            }
            Work::RationalPow => {
                let value = std::hint::black_box(
                    left.checked_pow(17, &RatLimits::digits(1_000_000), &RatNeverCancel)
                        .expect("the exponent fits the benchmark budget"),
                );
                if want_digest {
                    Some(canonical_rat_digest(&value))
                } else {
                    None
                }
            }
            Work::RationalNormalize => {
                // A common factor in both parts makes the full-GCD normalization the measured
                // work, not a constructor fast path that returns an already-reduced pair.
                let numerator = a.mul(b);
                let denominator = b.mul(&a.add(&Nat::from_u64(1)));
                let value = std::hint::black_box(
                    Rat::from_parts(Int::from_nat(numerator), denominator)
                        .expect("a non-zero generated denominator"),
                );
                if want_digest {
                    Some(canonical_rat_digest(&value))
                } else {
                    None
                }
            }
            Work::SymbolicsDiff => {
                if engine.is_none() {
                    let mut fresh = Engine::new();
                    fresh.set_precision(digits);
                    *engine = Some(fresh);
                }
                let engine = engine.as_mut().expect("the engine was just built");
                let value = std::hint::black_box(
                    engine
                        .eval("x = symbol(\"x\"); diff(x^10 + 3*x^5 + 7*x, x)")
                        .expect("the symbolic differentiation benchmark evaluates"),
                );
                if want_digest {
                    Some(value.display())
                } else {
                    None
                }
            }
            Work::SystemsMva => {
                let options = systems_options(digits);
                let network = QueueNetwork::new(
                    "bench",
                    Workload::Closed(lovelace_systems::ClosedWorkload {
                        population: 20,
                        think_time: ratio(1, 10),
                    }),
                    vec![
                        Station::new(
                            "cpu",
                            ServiceDiscipline::FcfsSingleServer,
                            ratio(1, 100),
                            Q::one(),
                        )
                        .expect("station"),
                        Station::new(
                            "disk",
                            ServiceDiscipline::FcfsSingleServer,
                            ratio(1, 50),
                            Q::from_i64(2),
                        )
                        .expect("station"),
                        Station::new("net", ServiceDiscipline::Delay, ratio(1, 500), Q::one())
                            .expect("station"),
                    ],
                )
                .expect("valid closed network");
                let result = std::hint::black_box(
                    analyze(&network, &options, &NeverCancel).expect("solved"),
                );
                if want_digest {
                    Some(format!(
                        "{}|{}|{}",
                        rational_decimal(&result.aggregate.throughput, digits),
                        rational_decimal(&result.aggregate.response_time, digits),
                        rational_decimal(&result.aggregate.occupancy, digits)
                    ))
                } else {
                    None
                }
            }
        }
    }
}

/// The decimal projection of an exact systems rational under the benchmark's digit budget.
///
/// The benchmark digests the projected digits, while the value itself remains the canonical
/// `lovelace-rational` type; the projection is explicit and never feeds back into arithmetic.
fn rational_decimal(value: &Q, digits: usize) -> String {
    lovelace_real::Real::from_rational(value, digits.max(1))
        .value
        .to_string()
}

/// A rational `numerator / denominator` for a benchmark model.
fn ratio(numerator: i64, denominator: i64) -> Q {
    Q::from_i64(numerator)
        .div(&Q::from_i64(denominator))
        .expect("a nonzero denominator")
}

/// The solver options a systems benchmark runs under: the digit argument is the rendering
/// precision; the math is exact either way.
fn systems_options(digits: usize) -> SolverOptions {
    SolverOptions {
        precision: digits.max(1),
        ..SolverOptions::default()
    }
}

/// The decimal entry `numerator / denominator`, exactly, for denominators 5 and 8 (which divide
/// 1000). Used to write the benchmark matrix without a floating-point literal.
fn decimal_ratio(numerator: i64, denominator: i64) -> String {
    let scaled = numerator * 1000;
    let whole = scaled / denominator;
    let sign = if whole < 0 { "-" } else { "" };
    let magnitude = whole.unsigned_abs();
    let integer = magnitude / 1000;
    let fraction = magnitude % 1000;
    if fraction == 0 {
        format!("{sign}{integer}")
    } else {
        let mut digits = format!("{fraction:03}");
        while digits.ends_with('0') {
            digits.pop();
        }
        format!("{sign}{integer}.{digits}")
    }
}

/// The source literal for the dense matrix `A = Q diag(1..n) Q` with `Q = I - (2/n) J`, whose
/// spectrum is exactly `1..=n`: `A_ij = d_i [i == j] - (2/n)(d_i + d_j) + 2(n+1)/n`. Written over
/// the common denominator `n`, every entry is `numerator / n`, which is an exact decimal for the
/// benchmark sizes 5 and 8.
fn symmetric_matrix_source(n: usize) -> String {
    let mut rows = Vec::with_capacity(n);
    for i in 0..n {
        let mut row = Vec::with_capacity(n);
        for j in 0..n {
            let diagonal = if i == j { (n * (i + 1)) as i64 } else { 0 };
            let numerator = diagonal - 2 * ((i + 1) as i64 + (j + 1) as i64) + 2 * (n as i64 + 1);
            row.push(decimal_ratio(numerator, n as i64));
        }
        rows.push(format!("[{}]", row.join(", ")));
    }
    format!("[{}]", rows.join(", "))
}

/// The digest of an `EigenResult`: sweeps, residual and the ascending eigenvalues.
fn eigen_digest(value: &Value) -> String {
    let Value::Record(type_name, fields) = value else {
        panic!(
            "eig_sym must answer an EigenResult, got {}",
            value.kind_name()
        );
    };
    assert_eq!(type_name, "EigenResult");
    let field = |name: &str| {
        fields
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value.display())
            .unwrap_or_else(|| panic!("EigenResult must publish '{name}'"))
    };
    format!(
        "{}|{}|{}",
        field("sweeps"),
        field("residual"),
        field("values")
    )
}

/// The canonical numerator/denominator digest, never a decimal projection.
fn canonical_rat_digest(value: &Rat) -> String {
    format!(
        "{}/{}",
        value.numerator().to_decimal(),
        value.denominator().to_decimal()
    )
}

/// The recorded environment for a benchmark run: host, compiler, profile, sizes and repetitions.
///
/// These lines make a timing comparable or not; a number without them is an anecdote.
fn print_environment(op: &str, digits: usize, reps: usize) {
    let rustc_program = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let rustc = std::process::Command::new(rustc_program)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| format!("rustc (minimum {})", env!("CARGO_PKG_RUST_VERSION")));
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    println!(
        "ENV op={op} digits={digits} reps={reps} os={} arch={} profile={profile} compiler={rustc}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: lovelace-bench <op> <digits> [reps]");
        std::process::exit(2);
    }
    let op = args[0].clone();
    let digits: usize = args[1].parse().expect("digits");
    let reps: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20);
    let Some(work) = Work::parse(&op) else {
        eprintln!("unknown op {op}");
        std::process::exit(2);
    };

    let a = Nat::from_decimal(&random_digits(digits, 12345)).expect("a");
    let b = Nat::from_decimal(&random_digits((digits / 2).max(2), 67890)).expect("b");

    // Warm up outside the timed region and capture the digest there, so conversion cost never
    // pollutes the measurement. The engine-level cases build their engine here too.
    let mut engine: Option<Engine> = None;
    let left = Rat::from_parts(Int::from_nat(a.clone()), b.clone())
        .expect("the generated denominator is non-zero");
    let right = Rat::from_parts(Int::from_nat(b.clone()), a.add(&Nat::from_u64(1)))
        .expect("the generated denominator is non-zero");
    print_environment(&op, digits, reps);
    let digest = work.run(digits, &a, &b, (&left, &right), true, &mut engine);

    let mut times: Vec<f64> = Vec::with_capacity(reps);
    for _ in 0..reps {
        let start = Instant::now();
        let _ = work.run(digits, &a, &b, (&left, &right), false, &mut engine);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let median = times[times.len() / 2];
    println!("RESULT {op} {digits} {mean:.3} {median:.3}");
    if let Some(text) = digest {
        let head: String = text.chars().take(24).collect();
        println!("DIGEST {op} {}:{head}", text.len());
    }
}
