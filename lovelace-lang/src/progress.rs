//! RUN-005: cooperative kernel sub-progress observation.
//!
//! A top-level statement used to be the smallest progress the engine could publish: a single long
//! kernel — `sqrt(2)` at 50 000 digits, `pi(n)`, `2000000!`, `2^1000000000` — ran from
//! `StepStarted` to `StepCommitted` with nothing observable in between. This module is the
//! additive seam that lets such a kernel report where it is, as a human-readable phase plus a
//! fraction, to whoever armed the observer.
//!
//! The state is THREAD LOCAL for the same reason [`crate::cancel`] is: the observer belongs to the
//! evaluation that armed it, so two engines on two threads cannot see each other's reports and a
//! test that arms a sink cannot leak it into the rest of the process. When nothing is armed the
//! hot path is one thread-local `Option` read, so an unobserved run pays nothing measurable and
//! produces exactly the same values as before.
//!
//! The sink is a plain callback (`phase`, `fraction`) rather than a channel so the engine does not
//! choose a transport: [`Engine::set_progress_sink`](crate::Engine::set_progress_sink) stores one
//! per engine and arms it for the duration of [`Engine::eval`](crate::Engine::eval), exactly the
//! way the live cancellation flag is armed. A host that wants events sends them from the callback.
//!
//! Reports are clamped to `0.0..=1.0` and to a MONOTONE sequence inside one scope: a later report
//! never shows a smaller fraction than an earlier one, so a phase that begins after a heavier one
//! cannot make the progress bar jump backwards.

use std::cell::{Cell, RefCell};
use std::sync::Arc;

/// Where a kernel's reports go: a callback taking the phase and the fraction.
///
/// The `Send + Sync` bounds are what let one engine own the sink on its worker thread while a host
/// keeps a clone (the desktop does: the callback forwards into the run's event queue).
pub type Sink = Arc<dyn Fn(&str, f32) + Send + Sync>;

thread_local! {
    /// The observer armed for the current evaluation, if any.
    static SINK: RefCell<Option<Sink>> = const { RefCell::new(None) };
    /// The largest fraction reported so far in the current scope (monotonicity).
    static LAST: Cell<f32> = const { Cell::new(0.0) };
    /// How many reports the current scope delivered.
    static DELIVERED: Cell<u64> = const { Cell::new(0) };
}

/// The armed state a [`Scope`] restores: an evaluation never inherits or leaks an observer.
pub struct Scope {
    previous_sink: Option<Sink>,
    previous_last: f32,
    previous_delivered: u64,
}

impl Drop for Scope {
    fn drop(&mut self) {
        SINK.with(|cell| *cell.borrow_mut() = self.previous_sink.take());
        LAST.with(|cell| cell.set(self.previous_last));
        DELIVERED.with(|cell| cell.set(self.previous_delivered));
    }
}

/// Arms `sink` for the current thread and returns the guard that restores the previous observer on
/// drop. `None` disarms (the unobserved path).
pub fn arm(sink: Option<Sink>) -> Scope {
    let previous_sink = SINK.with(|cell| cell.replace(sink));
    let previous_last = LAST.with(|cell| cell.replace(0.0));
    let previous_delivered = DELIVERED.with(|cell| cell.replace(0));
    Scope {
        previous_sink,
        previous_last,
        previous_delivered,
    }
}

/// True when an observer is armed for this thread. One thread-local read; the cheap gate a kernel
/// checks before it computes anything to report.
#[inline]
pub fn active() -> bool {
    SINK.with(|cell| cell.borrow().is_some())
}

/// How many reports the current scope delivered.
#[inline]
pub fn delivered() -> u64 {
    DELIVERED.with(Cell::get)
}

/// The largest fraction reported so far in the current scope (0.0 before any report).
#[inline]
pub fn last() -> f32 {
    LAST.with(Cell::get)
}

/// Reports one kernel phase and the fraction of the kernel's work completed.
///
/// A no-op when no observer is armed. The fraction is clamped to `0.0..=1.0` and raised to the
/// scope's previous fraction, so the sequence a sink observes is always monotone.
#[inline]
pub fn report(phase: &str, fraction: f32) {
    if !active() {
        return;
    }
    let fraction = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let last = LAST.with(Cell::get);
    let fraction = fraction.max(last);
    LAST.with(|cell| cell.set(fraction));
    let sink = SINK.with(|cell| cell.borrow().clone());
    if let Some(sink) = sink {
        DELIVERED.with(|cell| cell.set(cell.get() + 1));
        sink(phase, fraction);
    }
}

/// Reports only every `stride`-th iteration of a hot loop: the strided form a kernel loop uses so
/// the observer's cost stays a fraction of a percent of the loop body.
///
/// `iteration` is the loop's own counter (0-based is fine); `done` is how much of the work the
/// caller has finished and `total` the whole. Returns whether a report was delivered.
#[inline]
pub fn report_strided(iteration: u64, stride: u64, phase: &str, done: u64, total: u64) -> bool {
    if !active() || stride == 0 || iteration % stride != 0 {
        return false;
    }
    let fraction = if total == 0 {
        1.0
    } else {
        done as f32 / total as f32
    };
    report(phase, fraction);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn collector() -> (Sink, Arc<Mutex<Vec<(String, f32)>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink: Sink = {
            let seen = Arc::clone(&seen);
            Arc::new(move |phase: &str, fraction: f32| {
                seen.lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .push((phase.to_string(), fraction));
            })
        };
        (sink, seen)
    }

    /// The un-armed path delivers nothing and stays cheap; the armed one delivers what is reported.
    #[test]
    fn an_armed_sink_observes_reports_and_the_unarmed_path_is_silent() {
        assert!(!active());
        report("ignored", 0.5);
        assert_eq!(delivered(), 0);
        let (sink, seen) = collector();
        {
            let _scope = arm(Some(sink));
            assert!(active());
            report("phase-a", 0.25);
            report("phase-b", 0.75);
            assert_eq!(delivered(), 2);
        }
        assert!(!active(), "the scope must disarm on drop");
        assert_eq!(
            *seen.lock().unwrap_or_else(|error| error.into_inner()),
            vec![
                ("phase-a".to_string(), 0.25),
                ("phase-b".to_string(), 0.75)
            ]
        );
    }

    /// The observed sequence is monotone and clamped, whatever a kernel reports.
    #[test]
    fn the_observed_fraction_never_moves_backwards_and_stays_in_range() {
        let (sink, seen) = collector();
        {
            let _scope = arm(Some(sink));
            report("first", 0.60);
            report("second", 0.20);
            report("third", 7.5);
        }
        let seen = seen.lock().unwrap_or_else(|error| error.into_inner());
        assert_eq!(seen[0].1, 0.60);
        assert_eq!(seen[1].1, 0.60, "a lower fraction must be raised");
        assert_eq!(seen[2].1, 1.0, "a fraction above one must be clamped");
    }

    /// A nested scope restores the outer observer AND its monotone watermark.
    #[test]
    fn a_nested_scope_restores_the_outer_observer() {
        let (outer, seen_outer) = collector();
        let outer_scope = arm(Some(outer));
        report("outer", 0.4);
        {
            let (inner, seen_inner) = collector();
            let _inner_scope = arm(Some(inner));
            assert_eq!(last(), 0.0, "a nested scope starts its own sequence");
            report("inner", 0.9);
            assert_eq!(
                *seen_inner.lock().unwrap_or_else(|error| error.into_inner()),
                vec![("inner".to_string(), 0.9)]
            );
        }
        assert_eq!(last(), 0.4, "the outer watermark is restored");
        report("outer-again", 0.5);
        drop(outer_scope);
        assert_eq!(
            *seen_outer.lock().unwrap_or_else(|error| error.into_inner()),
            vec![
                ("outer".to_string(), 0.4),
                ("outer-again".to_string(), 0.5)
            ]
        );
        assert!(!active());
    }

    /// The strided helper reports only on its stride and computes the fraction from the counters.
    #[test]
    fn the_strided_helper_reports_on_its_stride_only() {
        let (sink, seen) = collector();
        {
            let _scope = arm(Some(sink));
            assert!(!report_strided(1, 64, "loop", 1, 64));
            assert!(report_strided(0, 64, "loop", 0, 64));
            assert!(!report_strided(63, 64, "loop", 63, 64));
            assert!(report_strided(64, 64, "loop", 64, 64));
        }
        let seen = seen.lock().unwrap_or_else(|error| error.into_inner());
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0], ("loop".to_string(), 0.0));
        assert_eq!(seen[1], ("loop".to_string(), 1.0));
    }
}
