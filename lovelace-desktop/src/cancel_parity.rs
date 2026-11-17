//! RUN-007 parity: the desktop's cancellation contract against the web Studio capture.
//!
//! [`REF-cancel.json`](../../../docs/goal-desktop-v4/round-64/REF-cancel.json) records the web
//! Studio cancelling the script
//!
//! ```text
//! a = 2
//! setprecision(20000)
//! c = sqrt(2)
//! d = c + 1
//! ```
//!
//! while statement 3 (`c = sqrt(2)`) is in flight. The Studio's runner checks its token only at
//! a statement boundary, so that statement runs to completion and is committed; the run ends
//! `cancelled` with 3 of 4 statements committed, variables `_`, `a` and `c` (and no `d`), and
//! `_` holding the 20 002-character `sqrt(2)` the in-flight statement produced.
//!
//! The tests below do not read the reference's values into the desktop or write the desktop's
//! output into the reference: the script, the committed count, the terminal status, the variable
//! names/kinds/displays and the `_`/result text are all read from the capture file at test time
//! and compared field by field with what the desktop's `ExecutionHost` produces.
//!
//! ## Why the parity harness synchronises the cancel request
//!
//! The reference capture is timing-based: the cancel is posted while statement 3 is running.
//! A test that sleeps for a fixed time before cancelling is a race against the parser's start-up
//! (`sqrt` takes ~0.6 s in a debug build, but the statement-boundary check happens microseconds
//! after the statement starts). [`MidFlightCancelEngine`] removes that race: it reports when the
//! third statement is inside `eval`, waits for the test to request cancellation, and only then
//! runs the real engine. Across that hand-off it masks the shared live flag, which is exactly the
//! state of a statement that was already past its last cancellation check when the request
//! arrived — the Studio's in-flight statement has no seam that observes the request at all, so a
//! statement that is uninterruptible in the Rust engine behaves identically.
//!
//! ## Mapping
//!
//! The script, the four statements and the two builtins (`sqrt`, `setprecision`) are the
//! reference's own, and the desktop's `sqrt(2)` renders the same 20 002-character Real, so no
//! value mapping is needed. The fields compared map 1:1:
//!
//! | REF-cancel.json | Desktop observation |
//! |---|---|
//! | `scenario.source` | the submitted source |
//! | `steps[0].final.status` | the terminal `DesktopEvent` variant (`Cancelled`) |
//! | `steps[0].final.totalStatements` | `WorkspaceSnapshot::total` |
//! | `steps[0].final.completedStatements` | `DesktopEvent::Cancelled::committed_steps` |
//! | `steps[0].final.response.variables[]` | `WorkspaceSnapshot::state.variables` (name, kind, display) |
//! | `steps[0].final.response.result.display` | `WorkspaceSnapshot::result` |
//! | `steps[0].final.response.precision` | `WorkspaceSnapshot::precision` |
//! | `steps[0].final.response.diagnostics` | `WorkspaceSnapshot::diagnostic` (`None`) |
//!
//! ## Interrupted statements are a different case
//!
//! The capture above is a **statement-boundary** cancellation: the web Studio's statement had
//! already completed when the cancel was observed, so it is committed (`Completed`). The desktop's
//! live-flag path (`RUN-008`) can stop an in-flight statement early; `COR-016` requires that
//! partial state to be **discarded**, so only the committed prefix is published. The RUN-008 test
//! below asserts that transactional behavior; it does not claim the web capture's in-flight
//! statement was rolled back.

#[cfg(test)]
mod tests {
    // The parity harness needs the real engine and its reference value; the deterministic fake
    // has no `sqrt(2)` and would make the comparison meaningless.
    #![cfg(feature = "real-engine")]

    use crate::engine::real::RealEngine;
    use crate::engine::{
        CachedValue, DEFAULT_PRECISION, EngineError, EngineState, Evaluator, PlotCapture,
    };
    use crate::execution::{
        DesktopEvent, DocumentId, ExecutionHost, InFlightCancellation, SequencedEvent,
        SourceRevision,
    };
    use lovelace_json::Json;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    /// The reference capture, loaded from the repository at test time.
    fn reference() -> Json {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../docs/goal-desktop-v4/round-64/REF-cancel.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        Json::parse(&text)
            .unwrap_or_else(|error| panic!("{} is not JSON: {error:?}", path.display()))
    }

    fn field<'a>(json: &'a Json, key: &str) -> &'a Json {
        json.get(key)
            .unwrap_or_else(|| panic!("the reference has no '{key}' field"))
    }

    fn text<'a>(json: &'a Json, key: &str) -> &'a str {
        field(json, key)
            .as_str()
            .unwrap_or_else(|| panic!("the reference's '{key}' is not a string"))
    }

    fn count(json: &Json, key: &str) -> usize {
        match field(json, key) {
            Json::Num(number) => number
                .parse()
                .unwrap_or_else(|error| panic!("'{key}' is not a count: {error}")),
            other => panic!("'{key}' is not a number: {other:?}"),
        }
    }

    /// The reference's variables as `(name, kind, display)` in capture order.
    fn reference_variables(final_step: &Json) -> Vec<(String, String, String)> {
        field(field(final_step, "response"), "variables")
            .as_arr()
            .expect("the reference's variables are an array")
            .iter()
            .map(|variable| {
                (
                    text(variable, "name").to_string(),
                    text(variable, "kind").to_string(),
                    text(variable, "display").to_string(),
                )
            })
            .collect()
    }

    /// One engine's evaluation of the fourth statement has not happened; the third blocks until
    /// the test releases it. See the module docs for why this makes the reference timing exact.
    struct MidFlightCancelEngine {
        inner: RealEngine,
        calls: usize,
        flag: Option<Arc<AtomicBool>>,
        entered: Arc<AtomicBool>,
        released: Arc<AtomicBool>,
    }

    impl Evaluator for MidFlightCancelEngine {
        fn eval(&mut self, src: &str) -> Result<String, String> {
            self.calls += 1;
            if self.calls != 3 {
                return self.inner.eval(src);
            }
            // Statement 3 is in flight. The worker has already emitted StepStarted(2) and is
            // blocked here inside eval; the test requests cancellation now.
            self.entered.store(true, Ordering::Release);
            let deadline = Instant::now() + Duration::from_secs(20);
            while !self.released.load(Ordering::Acquire) {
                assert!(
                    Instant::now() < deadline,
                    "the parity test never released the in-flight statement"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            // The request arrived while this statement was in flight, after the point where the
            // engine would have checked it. Mask the shared flag across the real evaluation:
            // `sqrt` has no poll seam, so this is indistinguishable from the flag being set while
            // it computes, and it prevents the engine's start-of-statement check from turning the
            // already-running statement into an early exit that never produced a value.
            let previous = self
                .flag
                .as_ref()
                .map(|flag| flag.swap(false, Ordering::AcqRel))
                .unwrap_or(false);
            let result = self.inner.eval(src);
            if let Some(flag) = &self.flag {
                flag.store(previous, Ordering::Release);
            }
            result
        }

        fn last_error(&self) -> Option<EngineError> {
            self.inner.last_error()
        }

        fn last_structured(&self) -> Option<Json> {
            self.inner.last_structured()
        }

        fn last_inspection(&self) -> Option<Json> {
            self.inner.last_inspection()
        }

        fn set_precision(&mut self, decimals: usize) {
            self.inner.set_precision(decimals);
        }

        fn set_cancel_flag(&mut self, flag: Option<Arc<AtomicBool>>) {
            self.flag = flag.clone();
            self.inner.set_cancel_flag(flag);
        }

        fn set_plot_dir(&mut self, dir: &str) {
            self.inner.set_plot_dir(dir);
        }

        fn precision(&self) -> usize {
            self.inner.precision()
        }

        fn variables(&self) -> Vec<(String, String)> {
            self.inner.variables()
        }

        fn functions(&self) -> Vec<(String, Vec<String>)> {
            self.inner.functions()
        }

        fn state(&self) -> Option<EngineState> {
            self.inner.state()
        }

        fn take_output(&mut self) -> Vec<String> {
            self.inner.take_output()
        }

        fn last_plot(&self) -> Option<PlotCapture> {
            self.inner.last_plot()
        }

        fn supports_cached_values(&self) -> bool {
            self.inner.supports_cached_values()
        }

        fn last_value(&self) -> Option<CachedValue> {
            self.inner.last_value()
        }

        fn apply_cached_value(&mut self, name: &str, value: &CachedValue) -> bool {
            self.inner.apply_cached_value(name, value)
        }

        fn tracks_result_variable(&self) -> bool {
            self.inner.tracks_result_variable()
        }
    }

    /// Reads events until a terminal one arrives, appending to `events`.
    fn drain_until_terminal(
        host: &ExecutionHost,
        events: &mut Vec<SequencedEvent>,
        timeout: Duration,
    ) -> SequencedEvent {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "timed out waiting for the terminal event; collected {:?}",
                events
                    .iter()
                    .map(|event| event.event.kind())
                    .collect::<Vec<_>>()
            );
            match host.next_event(remaining.min(Duration::from_millis(50))) {
                Some(event) => {
                    let terminal = matches!(
                        event.event,
                        DesktopEvent::Finished { .. }
                            | DesktopEvent::Cancelled { .. }
                            | DesktopEvent::Failed { .. }
                    );
                    events.push(event);
                    if terminal {
                        return events.last().expect("just pushed").clone();
                    }
                }
                None => continue,
            }
        }
    }

    /// RUN-007: the desktop's cancelled run against the captured web-Studio reference.
    ///
    /// Asserts the same statement count, the same variables present/absent, the same `_` value
    /// and the same terminal state, field by field against `REF-cancel.json` itself.
    #[test]
    fn cancel_parity_matches_the_web_studio_reference() {
        let reference = reference();
        let scenario = field(&reference, "scenario");
        let source = text(scenario, "source").to_string();
        let statements: Vec<String> = field(scenario, "statements")
            .as_arr()
            .expect("the reference's statements")
            .iter()
            .map(|statement| statement.as_str().expect("a statement string").to_string())
            .collect();
        assert_eq!(
            statements.len(),
            4,
            "the reference's scenario has four statements"
        );

        let steps = field(&reference, "steps")
            .as_arr()
            .expect("the reference steps");
        let final_step = field(&steps[0], "final");
        let reference_status = text(final_step, "status");
        let reference_total = count(final_step, "totalStatements");
        let reference_completed = count(final_step, "completedStatements");
        assert_eq!(
            reference_status, "cancelled",
            "the reference is a cancelled run"
        );
        assert_eq!(reference_total, 4);
        assert_eq!(reference_completed, 3);

        let response = field(final_step, "response");
        let reference_result = text(field(response, "result"), "display");
        let reference_precision = count(response, "precision");
        assert_eq!(
            field(response, "diagnostics")
                .as_arr()
                .expect("diagnostics")
                .len(),
            0,
            "the reference carries no diagnostic for a cancellation"
        );
        let reference_variables = reference_variables(final_step);
        assert_eq!(
            reference_variables
                .iter()
                .map(|(name, _, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["_", "a", "c"],
            "the reference's committed prefix holds _, a and c"
        );
        let reference_underscore = reference_variables
            .iter()
            .find(|(name, _, _)| name == "_")
            .map(|(_, _, display)| display.clone())
            .expect("the reference has _");
        assert_eq!(reference_underscore, reference_result);

        let entered = Arc::new(AtomicBool::new(false));
        let released = Arc::new(AtomicBool::new(false));
        let factory_entered = Arc::clone(&entered);
        let factory_released = Arc::clone(&released);
        let host = ExecutionHost::new(Box::new(move || {
            Box::new(MidFlightCancelEngine {
                inner: RealEngine::new(),
                calls: 0,
                flag: None,
                entered: Arc::clone(&factory_entered),
                released: Arc::clone(&factory_released),
            }) as Box<dyn Evaluator>
        }));
        let handle = host
            .submit(
                DocumentId(1),
                SourceRevision(1),
                source.clone(),
                DEFAULT_PRECISION,
            )
            .expect("submit the reference script");

        // Wait until statement 3 (`c = sqrt(2)`) is inside the engine. StepStarted(2) is emitted
        // before eval, so it is already in the channel when `entered` is set.
        let mut events: Vec<SequencedEvent> = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(20);
        while !entered.load(Ordering::Acquire) {
            assert!(
                Instant::now() < deadline,
                "statement 3 never entered the engine; collected {:?}",
                events
                    .iter()
                    .map(|event| event.event.kind())
                    .collect::<Vec<_>>()
            );
            if let Some(event) = host.next_event(Duration::from_millis(20)) {
                events.push(event);
            }
        }
        while !events
            .iter()
            .any(|event| matches!(event.event, DesktopEvent::StepStarted { index: 2, .. }))
        {
            let event = host
                .next_event(Duration::from_secs(5))
                .expect("StepStarted(2) was never delivered");
            events.push(event);
        }

        // This is the reference's cancel: statement 3 is in flight and the run is still active.
        assert!(
            host.cancel(&handle),
            "the in-flight run must accept the cancellation"
        );
        released.store(true, Ordering::Release);

        let terminal = drain_until_terminal(&host, &mut events, Duration::from_secs(30));
        let (committed_steps, in_flight, snapshot) = match &terminal.event {
            DesktopEvent::Cancelled {
                committed_steps,
                in_flight,
                snapshot,
                ..
            } => (*committed_steps, *in_flight, snapshot),
            other => panic!("terminal was {}, expected Cancelled", other.kind()),
        };
        assert_eq!(
            in_flight,
            InFlightCancellation::Completed,
            "the reference's in-flight statement completed; its value must be the committed one"
        );
        assert_eq!(
            committed_steps, reference_completed,
            "the desktop must commit the same number of statements as the reference"
        );
        assert_eq!(snapshot.committed_steps, reference_completed);
        assert_eq!(snapshot.total, reference_total);
        assert_eq!(snapshot.precision, reference_precision);
        assert_eq!(
            snapshot.result, reference_result,
            "the desktop's terminal result must be the reference's result, digit for digit"
        );
        assert_eq!(
            snapshot.diagnostic, None,
            "a cancellation is not a diagnostic"
        );
        assert!(snapshot.output.is_empty(), "the scenario prints nothing");

        // The variables present/absent, their kinds and their displays, compared exactly.
        let desktop_variables: Vec<(String, String, String)> = snapshot
            .state
            .variables
            .iter()
            .map(|variable| {
                (
                    variable.name.clone(),
                    variable.kind.clone(),
                    variable.display.clone(),
                )
            })
            .collect();
        assert_eq!(
            desktop_variables, reference_variables,
            "the committed variables (name, kind, display) must equal the reference's"
        );
        let desktop_underscore = &snapshot
            .state
            .variables
            .iter()
            .find(|variable| variable.name == "_")
            .expect("the desktop publishes _")
            .display;
        assert_eq!(
            desktop_underscore, &reference_underscore,
            "the _ value must be the reference's value, digit for digit"
        );
        assert!(
            !snapshot
                .state
                .variables
                .iter()
                .any(|variable| variable.name == "d"),
            "the statement after the in-flight one must never execute"
        );
        // The legacy pair list is the same prefix (its display carries the kind suffix, so only
        // the names are compared here; the kinds come from the state capture above).
        let legacy_names: Vec<&str> = snapshot
            .variables
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        let reference_names: Vec<&str> = reference_variables
            .iter()
            .map(|(name, _, _)| name.as_str())
            .collect();
        assert_eq!(legacy_names, reference_names);

        // Nothing after the in-flight statement may execute.
        let started: Vec<usize> = events
            .iter()
            .filter_map(|event| match event.event {
                DesktopEvent::StepStarted { index, .. } => Some(index),
                _ => None,
            })
            .collect();
        assert_eq!(
            started,
            [0, 1, 2],
            "no statement after the in-flight one may start"
        );
        let committed: Vec<usize> = events
            .iter()
            .filter_map(|event| match event.event {
                DesktopEvent::StepCommitted { index, .. } => Some(index),
                _ => None,
            })
            .collect();
        assert_eq!(
            committed,
            [0, 1, 2],
            "the committed prefix must be statements 1-3, including the in-flight one"
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event.event,
                    DesktopEvent::Finished { .. }
                        | DesktopEvent::Cancelled { .. }
                        | DesktopEvent::Failed { .. }
                ))
                .count(),
            1,
            "exactly one terminal event"
        );

        println!(
            "REF-cancel parity: status={} completed={}/{} variables={:?} underscore_len={} underscore_head={} underscore_tail={} result_len={} precision={} diagnostics=0 match=true",
            reference_status,
            committed_steps,
            reference_total,
            desktop_variables
                .iter()
                .map(|(name, kind, display)| format!("{name}[{kind}]={} chars", display.len()))
                .collect::<Vec<_>>(),
            desktop_underscore.len(),
            &desktop_underscore[..24],
            &desktop_underscore[desktop_underscore.len() - 16..],
            snapshot.result.len(),
            snapshot.precision
        );
    }

    /// RUN-007/RUN-008: a cancellation-aware in-flight statement stops through the live flag, and
    /// `COR-016` discards its partial state. Only the previously committed prefix (`a`, `c = 0`)
    /// is published, and `d = c + 1` must never run.
    ///
    /// The statement is one `while` loop that would take minutes; the live flag makes it stop in
    /// milliseconds (RUN-008). The loop's completed iterations are in the engine's state when it
    /// stops, but that state belongs to the interrupted statement and must not become workspace
    /// state.
    #[test]
    fn an_interrupted_in_flight_statement_is_rolled_back_and_the_next_never_starts() {
        let host = ExecutionHost::new(Box::new(|| {
            Box::new(RealEngine::new()) as Box<dyn Evaluator>
        }));
        let document = DocumentId(2);
        let source = "a = 2\nc = 0\nwhile (c < 100000000) { c = c + 1 }\nd = c + 1\n";
        let handle = host
            .submit(
                document,
                SourceRevision(3),
                source.to_string(),
                DEFAULT_PRECISION,
            )
            .expect("submit");

        // Wait until statement 3 is in flight, then give the loop enough time to make progress so
        // the committed partial state is distinguishable from the previous statement's `c = 0`.
        let mut events: Vec<SequencedEvent> = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(20);
        while !events
            .iter()
            .any(|event| matches!(event.event, DesktopEvent::StepStarted { index: 2, .. }))
        {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "statement 3 never started; collected {:?}",
                events
                    .iter()
                    .map(|event| event.event.kind())
                    .collect::<Vec<_>>()
            );
            if let Some(event) = host.next_event(remaining.min(Duration::from_millis(50))) {
                events.push(event);
            }
        }
        std::thread::sleep(Duration::from_millis(100));

        // RUN-008: the flag must stop the loop inside the two-second budget.
        let cancel_sent = Instant::now();
        assert!(
            host.cancel(&handle),
            "the loop run must accept the cancellation"
        );
        let terminal = drain_until_terminal(&host, &mut events, Duration::from_secs(2));
        let elapsed = cancel_sent.elapsed();
        assert!(
            elapsed < Duration::from_secs(2),
            "the cancellation-aware loop took {elapsed:?} to stop, over the 2 s budget"
        );

        let (committed_steps, in_flight, snapshot) = match &terminal.event {
            DesktopEvent::Cancelled {
                committed_steps,
                in_flight,
                snapshot,
                ..
            } => (*committed_steps, *in_flight, snapshot),
            other => panic!("terminal was {}, expected Cancelled", other.kind()),
        };
        assert_eq!(
            in_flight,
            InFlightCancellation::Interrupted,
            "the loop observed the live flag and stopped early"
        );
        assert_eq!(
            committed_steps, 2,
            "only the committed prefix, not the interrupted statement, is published"
        );
        assert_eq!(snapshot.committed_steps, 2);
        assert_eq!(
            snapshot.result, "0",
            "the interrupted statement produced no committed value"
        );
        assert_eq!(snapshot.diagnostic, None);
        let variable = |name: &str| {
            snapshot
                .state
                .variables
                .iter()
                .find(|variable| variable.name == name)
                .map(|variable| (variable.kind.as_str(), variable.display.as_str()))
        };
        assert_eq!(variable("a"), Some(("Natural", "2")));
        let (kind, display) = variable("c").expect("the committed prefix's c is published");
        assert_eq!(kind, "Natural");
        assert_eq!(
            display, "0",
            "the interrupted loop's partial c must not be committed"
        );
        assert_eq!(
            variable("d"),
            None,
            "the statement after the interrupted one must never execute"
        );
        // The interrupted statement must not produce a commit event.
        assert!(
            !events
                .iter()
                .any(|event| matches!(event.event, DesktopEvent::StepCommitted { index: 2, .. })),
            "the interrupted statement must not be committed"
        );
        assert!(
            !events
                .iter()
                .any(|event| matches!(event.event, DesktopEvent::StepStarted { index: 3, .. })),
            "nothing after the in-flight statement may start"
        );

        println!(
            "interrupted in-flight rollback: cancel-to-terminal={:.2} ms committed=2/4 c={} d=absent result=0",
            elapsed.as_secs_f64() * 1000.0,
            display
        );
    }
}
