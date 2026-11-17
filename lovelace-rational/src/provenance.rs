//! Exactness provenance: the four independent questions a numerical result must answer.
//!
//! RustLace's numerical philosophy distinguishes four different claims that are often collapsed
//! into the single word "exact":
//!
//! 1. **Computation exactness** — did the algorithm compute the model's value without an
//!    approximation step?
//! 2. **Input fidelity** — were the inputs themselves exact, or had a previous step already
//!    truncated them?
//! 3. **Representation exactness** — does the returned representation carry the value exactly, or
//!    is it a bounded-precision projection?
//! 4. **Render precision** — how many digits were used to *print* the value?
//!
//! A truncated [`crate::Rat`] to decimal is exact as a rational but lossy as a rendering of the
//! original; a [`lovelace_real::Real`] converted back to a rational is an exact representation of
//! its stored digits and still not a restoration of digits it never stored. Keeping the four
//! fields separate is what makes those statements checkable rather than rhetorical.

/// Whether a computation applied an approximation step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputationExactness {
    /// The algorithm's result is exact under its model.
    Exact,
    /// The algorithm used a bounded or iterative approximation.
    Approximate,
}

/// The fidelity of the value(s) a computation or conversion started from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputFidelity {
    /// The input was exact when it entered the operation.
    Exact,
    /// The input already carried a bounded number of digits when it entered.
    Truncated {
        /// The digit budget that produced the input.
        precision: usize,
    },
}

/// Whether a returned representation carries the value exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationExactness {
    /// The representation denotes the value exactly.
    Exact,
    /// The representation is a bounded-precision projection of the value.
    Truncated {
        /// The digit budget the representation was bounded to.
        precision: usize,
    },
}

/// The complete exactness report for one value or conversion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactnessProvenance {
    /// Whether the producing computation was exact.
    pub computation: ComputationExactness,
    /// The fidelity of the inputs the computation started from.
    pub input: InputFidelity,
    /// Whether the returned representation carries the value exactly.
    pub representation: RepresentationExactness,
    /// The digit budget used only to render the value, when a rendering happened.
    pub render_precision: Option<usize>,
}

impl ExactnessProvenance {
    /// An exact computation over exact inputs with an exact representation.
    pub fn exact() -> ExactnessProvenance {
        ExactnessProvenance {
            computation: ComputationExactness::Exact,
            input: InputFidelity::Exact,
            representation: RepresentationExactness::Exact,
            render_precision: None,
        }
    }

    /// Marks the computation as approximate.
    pub fn approximate(mut self) -> ExactnessProvenance {
        self.computation = ComputationExactness::Approximate;
        self
    }

    /// Records that an input had already lost digits.
    pub fn with_truncated_input(mut self, precision: usize) -> ExactnessProvenance {
        self.input = InputFidelity::Truncated { precision };
        self
    }

    /// Records that the returned representation is a bounded projection.
    pub fn with_truncated_representation(mut self, precision: usize) -> ExactnessProvenance {
        self.representation = RepresentationExactness::Truncated { precision };
        self
    }

    /// Records the separate digit budget used to render the value.
    pub fn with_render_precision(mut self, precision: usize) -> ExactnessProvenance {
        self.render_precision = Some(precision);
        self
    }

    /// True when the computation, its inputs and its representation are all exact.
    ///
    /// Render precision is deliberately not part of this predicate: printing two digits of an
    /// exact `1/3` does not make the value inexact.
    pub fn is_value_exact(&self) -> bool {
        self.computation == ComputationExactness::Exact
            && self.input == InputFidelity::Exact
            && self.representation == RepresentationExactness::Exact
    }

    /// True when every field, including the render precision, is exact/unbounded.
    pub fn is_fully_exact(&self) -> bool {
        self.is_value_exact() && self.render_precision.is_none()
    }
}

impl Default for ExactnessProvenance {
    fn default() -> Self {
        ExactnessProvenance::exact()
    }
}

/// A converted value together with its explicit loss-of-exactness report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conversion<T> {
    /// The converted value.
    pub value: T,
    /// True when neither the input fidelity nor the representation lost anything.
    pub lossless: bool,
    /// The four-part exactness report.
    pub provenance: ExactnessProvenance,
}

impl<T> Conversion<T> {
    /// A conversion that preserves the original value exactly.
    pub fn exact(value: T) -> Conversion<T> {
        Conversion {
            value,
            lossless: true,
            provenance: ExactnessProvenance::exact(),
        }
    }

    /// A conversion whose representation was bounded to `precision` digits.
    pub fn truncated(value: T, precision: usize) -> Conversion<T> {
        Conversion {
            value,
            lossless: false,
            provenance: ExactnessProvenance::exact().with_truncated_representation(precision),
        }
    }

    /// Marks the input as having already lost `precision` digits; the result is not lossless.
    pub fn from_truncated_input(value: T, precision: usize) -> Conversion<T> {
        Conversion {
            value,
            lossless: false,
            provenance: ExactnessProvenance::exact().with_truncated_input(precision),
        }
    }

    /// Maps the value while keeping the report.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Conversion<U> {
        Conversion {
            value: f(self.value),
            lossless: self.lossless,
            provenance: self.provenance,
        }
    }

    /// True when the conversion preserved both the input and the representation.
    pub fn is_lossless(&self) -> bool {
        self.lossless
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_fields_stay_independent() {
        let exact = ExactnessProvenance::exact();
        assert!(exact.is_value_exact());
        assert!(exact.is_fully_exact());

        let rendered = exact.clone().with_render_precision(2);
        assert!(
            rendered.is_value_exact(),
            "a render budget is not a value loss"
        );
        assert!(!rendered.is_fully_exact());

        let truncated_input = exact.clone().with_truncated_input(30);
        assert!(!truncated_input.is_value_exact());
        assert_eq!(
            truncated_input.input,
            InputFidelity::Truncated { precision: 30 }
        );

        let projected = exact.clone().with_truncated_representation(12);
        assert!(!projected.is_value_exact());
        assert_eq!(
            projected.representation,
            RepresentationExactness::Truncated { precision: 12 }
        );

        assert!(!exact.clone().approximate().is_value_exact());
    }

    #[test]
    fn a_conversion_is_lossless_only_when_nothing_was_dropped() {
        let exact = Conversion::exact(3);
        assert!(exact.is_lossless());
        let truncated = Conversion::truncated(3, 10);
        assert!(!truncated.is_lossless());
        let inherited = Conversion::from_truncated_input(3, 10);
        assert!(!inherited.is_lossless());
        assert_eq!(
            inherited.provenance.input,
            InputFidelity::Truncated { precision: 10 }
        );
        assert_eq!(Conversion::truncated(3, 10).map(|v| v + 1).value, 4);
    }
}
