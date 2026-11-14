//! Cancellation-aware natural arithmetic for the two reference workloads that spend their whole
//! budget INSIDE one kernel call.
//!
//! The reference's `Natural.Pow` and `Natural.Factorial` poll `Cancellation.ThrowIfCancellationRequested`
//! between limbs and between product-tree nodes (Lovelace.Natural/Natural.cs:574-695). The Rust
//! natural crate has no poll seam, and this lane's scope excludes it, so the two operations the
//! cancellation census rows drive are re-implemented here over the crate's PUBLIC arithmetic
//! (`Nat::mul`, `Nat::from_u64`) with the same stride checks. The un-budgeted result is the same
//! value `Nat::pow` / `Nat::factorial` returns; the only difference is that an armed deadline can
//! stop the work before the value exists.
//!
//! Small inputs keep the native fast path: polling is only worth its cost where the work is
//! measurable. The cancellation rows use 2^1000000000 and 2000000!, both far above these bounds.

use lovelace_nat::Nat;

use crate::LangError;

/// Factorioal leaves below this size are multiplied sequentially without recursing further.
const FACTORIAL_LEAF: u64 = 64;

/// Inputs at or below this size use the natural crate's own product tree (no poll seam, but the
/// work is microseconds and the cancellation rows are orders of magnitude above it).
const NATIVE_FACTORIAL_LIMIT: u64 = 4096;

/// RUN-005: multiplications between sub-progress reports in the factorial product tree. One report
/// per merge node would be tens of thousands of callbacks for `2000000!`; one per 256 leaves is
/// ~0.03 % of the loop body and still moves the observer's fraction.
const FACTORIAL_PROGRESS_STRIDE: u64 = 256;

fn cancelled() -> LangError {
    crate::cancelled_error()
}

/// `base ^ exponent` by binary exponentiation, checking the caller's deadline between every
/// square and multiply. Returns the structured cancellation failure when the deadline is observed.
///
/// RUN-005: the exponentiation has few iterations but every one of them is a full-width product,
/// so the phase/fraction is reported per iteration (never strided: 2^1000000000 spends its whole
/// budget inside ~30 squares).
pub(crate) fn pow(base: &Nat, exponent: u64) -> Result<Nat, LangError> {
    if exponent == 0 {
        return Ok(Nat::from_u64(1));
    }
    if exponent == 1 {
        return Ok(base.clone());
    }
    let mut result = Nat::from_u64(1);
    let mut factor = base.clone();
    let mut remaining = exponent;
    // How many bits of the exponent have been consumed, for the observer's fraction.
    let total_bits = 64 - exponent.leading_zeros();
    loop {
        if lovelace_symbolics::cancel::check() {
            return Err(cancelled());
        }
        if crate::progress::active() {
            let remaining_bits = 64 - remaining.leading_zeros();
            let done_bits = total_bits.saturating_sub(remaining_bits);
            crate::progress::report(
                "pow:binary-exponentiation",
                done_bits as f32 / total_bits as f32,
            );
        }
        if remaining & 1 == 1 {
            result = result.mul(&factor);
        }
        remaining >>= 1;
        if remaining == 0 {
            break;
        }
        factor = factor.mul(&factor);
    }
    if lovelace_symbolics::cancel::check() {
        return Err(cancelled());
    }
    Ok(result)
}

/// `n!` by a balanced product tree, checking the caller's deadline per leaf range and per merge.
///
/// RUN-005: the tree visits every value from 2 to n, so the leaf loop reports its fraction of that
/// range (strided, see [`FACTORIAL_PROGRESS_STRIDE`]) and a kernel of seconds shows real progress
/// instead of one label.
pub(crate) fn factorial(n: u64) -> Result<Nat, LangError> {
    if n < 2 {
        return Ok(Nat::from_u64(1));
    }
    if n <= NATIVE_FACTORIAL_LIMIT {
        return Ok(Nat::factorial(n));
    }
    let mut progress = FactorialProgress {
        done: 0,
        total: n - 1,
    };
    product_range(2, n, &mut progress)
}

/// The product tree's observation state: values multiplied so far out of the whole range.
struct FactorialProgress {
    done: u64,
    total: u64,
}

impl FactorialProgress {
    /// Counts one multiplied value and reports the fraction on the stride.
    fn advance(&mut self) {
        self.done += 1;
        if self.done % FACTORIAL_PROGRESS_STRIDE == 0 {
            crate::progress::report(
                "factorial:product-tree",
                self.done as f32 / self.total.max(1) as f32,
            );
        }
    }
}

fn product_range(
    low: u64,
    high: u64,
    progress: &mut FactorialProgress,
) -> Result<Nat, LangError> {
    if lovelace_symbolics::cancel::check() {
        return Err(cancelled());
    }
    if high - low < FACTORIAL_LEAF {
        let mut product = Nat::from_u64(1);
        for value in low..=high {
            if lovelace_symbolics::cancel::check() {
                return Err(cancelled());
            }
            product = product.mul(&Nat::from_u64(value));
            progress.advance();
        }
        return Ok(product);
    }
    let middle = low + (high - low) / 2;
    let left = product_range(low, middle, progress)?;
    let right = product_range(middle + 1, high, progress)?;
    if lovelace_symbolics::cancel::check() {
        return Err(cancelled());
    }
    Ok(left.mul(&right))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The un-budgeted results are the natural crate's own values; the cancellation path is
    /// pinned by round92_runner_families' two census rows.
    #[test]
    fn the_unbudgeted_results_match_the_natural_crate() {
        assert_eq!(pow(&Nat::from_u64(2), 10), Ok(Nat::from_u64(1024)));
        assert_eq!(pow(&Nat::from_u64(7), 0), Ok(Nat::from_u64(1)));
        assert_eq!(factorial(20), Ok(Nat::factorial(20)));
        assert_eq!(factorial(0), Ok(Nat::from_u64(1)));
    }

    /// A spent deadline refuses instead of returning a partial value.
    #[test]
    fn a_spent_deadline_refuses() {
        let _scope = lovelace_symbolics::cancel::arm(Some(
            std::time::Instant::now() - std::time::Duration::from_secs(1),
        ));
        assert_eq!(
            pow(&Nat::from_u64(2), 10)
                .expect_err("a spent deadline refuses")
                .code,
            "Cancelled"
        );
        assert_eq!(
            factorial(20000).expect_err("a spent deadline refuses").code,
            "Cancelled"
        );
    }
}
