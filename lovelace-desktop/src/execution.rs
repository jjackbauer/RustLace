//! Background, per-document execution of sliced Lovelace statements.
//!
//! This is the framework-independent core the desktop UI will drive: it knows nothing about
//! Win32, GDI or the application state. A run executes on a worker thread, publishes typed events
//! through [ExecutionHost::next_event], numbers them on delivery, and terminates exactly once with
//! [DesktopEvent::Finished], [DesktopEvent::Cancelled] or [DesktopEvent::Failed].
//!
//! Guarantees this module owns:
//!
//! - one active run per document, while other documents remain runnable ([SubmitError::Busy]);
//! - a run's final state — the published snapshot and the document's incremental cache — is settled
//!   *before* its terminal event is observable, so a client that submits the next run the moment it
//!   sees `Finished`/`Failed`/`Cancelled` cannot race the cache hand-back;
//! - one engine instance per run, created on the worker thread from the host's factory, evaluated
//!   once per sliced statement so definitions made earlier persist and `take_output` is drained;
//! - a cooperative cancellation token checked at every statement boundary;
//! - panic isolation: a panicking engine fails only its own run, never the host;
//! - typed events carrying the document, run and source revision, applied through [EventSink],
//!   which ignores results from a stale revision.
//!
//! Diagnostics: [crate::engine::Evaluator] surfaces only a message string, so a position is
//! recovered from the message when the engine names one ("... at position N.") and translated
//! back into the full source; otherwise the diagnostic points at the statement start. An engine
//! that publishes its structured error record (code, category, suggestions) contributes those
//! fields through [crate::engine::Evaluator::last_error] (RUN-014).

use std::any::Any;
use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::engine::{CachedValue, DEFAULT_PRECISION, EngineState, Evaluator, PlotCapture};
use crate::incremental::{CachedEntry, DocumentCache, ProgramAnalysis, StatementKey, StepDecision};
use crate::statements::Statement;
use lovelace_json::Json;

/// Identifies one document buffer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentId(pub u64);

/// Identifies one run of one document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RunId(pub u64);

/// Identifies the source revision a run was requested for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceRevision(pub u64);

/// Everything the panes need to update together, plus the revision it belongs to.
///
/// RUN-003: this is the atomic unit of pane state. It is published **at every commit** (carried
/// by [`DesktopEvent::StepCommitted`]) and once more with the terminal event, always rebuilt from
/// one `Progress` capture. A consumer that adopts the snapshot has, at the same instant, the
/// variables, functions, state capture, plot, print output and revision of exactly the committed
/// statements it names in `committed_steps`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    /// Display form of the last value-producing committed statement ("Void" when none did).
    pub result: String,
    /// The engine's structured projection of that same last committed result, alongside its
    /// display text.
    ///
    /// RES-001: this is `structured::project_value`'s Json, published by the engine at the
    /// moment the value existed. The window renders it directly; it never parses [`Self::result`]
    /// to recover arrays, records or symbolic fields.
    pub result_structured: Option<Json>,
    /// SYM-001: the engine's symbolic inspection payload captured for that same result.
    ///
    /// This is read from [`Evaluator::last_inspection`] at the same commit point as the display
    /// and the structured projection, so the Inspector pane is bound to the last **committed**
    /// result and never to the editor buffer. `None` when the engine publishes no inspection
    /// (for example the deterministic fake).
    pub result_inspection: Option<Json>,
    /// Variables visible after the committed statements, sorted by the engine.
    pub variables: Vec<(String, String)>,
    /// Functions visible after the committed statements.
    ///
    /// WKS-002: this is rebuilt from [`Self::state`] at every commit, so the list and its count
    /// are the engine's current registry (builtins, built-in families and user definitions), not
    /// the legacy [`Evaluator::functions`] pair list that names user definitions only.
    pub functions: Vec<(String, Vec<String>)>,
    /// The richer capture the completion catalogue reads: variables with their kinds and
    /// every function with its declared parameters and builtin flag.
    ///
    /// Taken from [`Evaluator::state`] at the same moment as `variables`/`functions`, so it
    /// describes exactly the committed prefix the panes display.
    pub state: EngineState,
    /// The most recent plot produced by the committed statements.
    pub plot: Option<PlotCapture>,
    /// Every print line produced by the committed statements, in order.
    pub output: Vec<String>,
    /// The source revision this snapshot belongs to.
    pub revision: SourceRevision,
    /// How many statements were committed when the snapshot was taken.
    pub committed_steps: usize,
    /// How many sliced statements the run that produced this snapshot visited.
    pub total: usize,
    /// RUN-020: the engine's display precision when the snapshot was taken.
    ///
    /// A script's `setprecision(n)` changes the engine's precision mid-run, so this is the only
    /// honest source for what the app should show as the active precision: the value submitted
    /// with the run is what the run *started* at. A script that never calls `setprecision`
    /// reports the submitted value back unchanged.
    pub precision: usize,
    /// The failure that ended the run, when it ended in one.
    pub diagnostic: Option<String>,
}

/// What happened to the statement that was in flight when a run was cancelled.
///
/// RUN-007: the web Studio observes cancellation only at a statement boundary, so its in-flight
/// statement always completes and is committed. The desktop also polls a live flag inside long
/// kernels (RUN-008), so its in-flight statement can either complete normally or stop early
/// through that flag. This type names the two outcomes so the terminal event and the log can say
/// which one happened instead of pretending they are the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InFlightCancellation {
    /// Cancellation was accepted before the next statement started; nothing was in flight.
    None,
    /// The in-flight statement returned a value and that value is committed.
    Completed,
    /// The in-flight statement stopped early through the engine's live cancellation flag. Its
    /// partial state is **discarded**: only the previously committed prefix is published.
    Interrupted,
}

/// One kernel sub-progress report (RUN-005): the phase the engine's kernel is in and how much of
/// its own work it has finished.
///
/// This is deliberately distinct from the statement counter the strip already shows: `1/1` says
/// one statement committed, while `sqrt:integer-square-root 10%` says where the statement that is
/// still running is. The strip paints both, side by side, and a reader (or a UIA capture) can tell
/// which is which because one is a count and the other names the engine's phase.
#[derive(Clone, Debug, PartialEq)]
pub struct KernelProgress {
    /// The engine's human-readable phase (`sqrt:integer-square-root`, `pi:machin-arctan`, ...).
    pub phase: String,
    /// Fraction of the kernel's work completed, from 0.0 to 1.0.
    pub fraction: f32,
}

impl KernelProgress {
    /// The whole-number percentage the strip paints.
    pub fn percent(&self) -> u32 {
        (self.fraction.clamp(0.0, 1.0) * 100.0).round() as u32
    }

    /// The strip's label: the engine's phase and the percentage, e.g. `sqrt:truncate 90%`.
    pub fn label(&self) -> String {
        format!("{} {}%", self.phase, self.percent())
    }
}

/// A typed notification from a running document.
#[derive(Clone, Debug, PartialEq)]
pub enum DesktopEvent {
    /// A run started; `total` is the number of sliced top-level statements.
    RunStarted {
        /// The document being run.
        document: DocumentId,
        /// The run's identity.
        run: RunId,
        /// The source revision that was submitted.
        revision: SourceRevision,
        /// The number of statements the run will visit.
        total: usize,
    },
    /// A statement is about to be evaluated.
    StepStarted {
        /// The document being run.
        document: DocumentId,
        /// The run's identity.
        run: RunId,
        /// 0-based statement index.
        index: usize,
        /// Short display label for the statement.
        label: String,
    },
    /// A statement was evaluated successfully and its effects are part of the workspace.
    ///
    /// RUN-003: the event carries the complete [`WorkspaceSnapshot`] for the committed prefix
    /// through `index`, taken at the same commit point as the variable/function/state/plot
    /// captures. The consumer applies the snapshot, the statement's log lines and the committed
    /// counter from this one event, so no pane can advance alone.
    StepCommitted {
        /// The document being run.
        document: DocumentId,
        /// The run's identity.
        run: RunId,
        /// 0-based statement index.
        index: usize,
        /// Whether the value was reused from the incremental cache.
        ///
        /// True exactly when the statement was **not** evaluated and its cached typed value was
        /// applied to the engine (INC-001/INC-008). A statement whose cached value could not be
        /// applied is evaluated and reported as `false`; the flag is never a prediction.
        reused: bool,
        /// The coherent workspace through this statement.
        snapshot: WorkspaceSnapshot,
    },
    /// One print line produced by a statement.
    Output {
        /// The document being run.
        document: DocumentId,
        /// The run's identity.
        run: RunId,
        /// The printed line.
        text: String,
    },
    /// Progress within the run. The evaluator boundary exposes no finer granularity, so this
    /// reports committed statements over the total.
    SubProgress {
        /// The document being run.
        document: DocumentId,
        /// The run's identity.
        run: RunId,
        /// Committed fraction of the run, from 0.0 to 1.0.
        fraction: f32,
        /// Human-readable progress label.
        label: String,
    },
    /// RUN-005: progress *inside* the statement that is running, reported by the kernel itself.
    ///
    /// [`DesktopEvent::SubProgress`] is statement granularity (committed over total); this is the
    /// kernel's own phase plus the fraction of its work it has finished, so a window watching a
    /// single long `sqrt(2)`, `pi(n)` or `n!` statement can show that the kernel is inside its
    /// integer square root instead of appearing hung. The phase is the engine's own wording; the
    /// desktop never derives one.
    KernelProgress {
        /// The document being run.
        document: DocumentId,
        /// The run's identity.
        run: RunId,
        /// The engine's human-readable phase (`sqrt:integer-square-root`, ...).
        phase: String,
        /// Fraction of the kernel's work completed, from 0.0 to 1.0.
        fraction: f32,
    },
    /// The run completed normally.
    Finished {
        /// The document that ran.
        document: DocumentId,
        /// The run's identity.
        run: RunId,
        /// The coherent state after the last committed statement.
        snapshot: WorkspaceSnapshot,
    },
    /// The run stopped because its cancellation token was signalled.
    ///
    /// RUN-003: `snapshot` describes exactly the `committed_steps` committed statements; it is
    /// the same value the terminal publish wrote, carried in the event so the consumer applies
    /// the terminal panes from one object instead of looking the snapshot up separately.
    ///
    /// RUN-007: the statement that was in flight when cancellation was observed is part of that
    /// prefix — it is never rolled back and never silently dropped. [`InFlightCancellation`]
    /// says whether it completed with a value or stopped early through the live flag.
    Cancelled {
        /// The document that ran.
        document: DocumentId,
        /// The run's identity.
        run: RunId,
        /// The number of statements committed when cancellation was observed, including the
        /// in-flight statement it was observed inside.
        committed_steps: usize,
        /// What happened to the statement in flight when cancellation was requested.
        in_flight: InFlightCancellation,
        /// The coherent state of the committed prefix.
        snapshot: WorkspaceSnapshot,
    },
    /// The run stopped because a statement failed or the engine panicked.
    ///
    /// RUN-003: `snapshot` is the committed prefix, with the rendered diagnostic in its
    /// `diagnostic` field; `diagnostic` carries the structured position for the editor.
    Failed {
        /// The document that ran.
        document: DocumentId,
        /// The run's identity.
        run: RunId,
        /// The structured failure: message plus its position in the full source.
        diagnostic: Diagnostic,
        /// The coherent state of the committed prefix plus the failure.
        snapshot: WorkspaceSnapshot,
    },
}

impl DesktopEvent {
    /// The document this event belongs to.
    pub fn document(&self) -> DocumentId {
        match self {
            DesktopEvent::RunStarted { document, .. }
            | DesktopEvent::StepStarted { document, .. }
            | DesktopEvent::StepCommitted { document, .. }
            | DesktopEvent::Output { document, .. }
            | DesktopEvent::SubProgress { document, .. }
            | DesktopEvent::KernelProgress { document, .. }
            | DesktopEvent::Finished { document, .. }
            | DesktopEvent::Cancelled { document, .. }
            | DesktopEvent::Failed { document, .. } => *document,
        }
    }

    /// The run this event belongs to.
    pub fn run(&self) -> RunId {
        match self {
            DesktopEvent::RunStarted { run, .. }
            | DesktopEvent::StepStarted { run, .. }
            | DesktopEvent::StepCommitted { run, .. }
            | DesktopEvent::Output { run, .. }
            | DesktopEvent::SubProgress { run, .. }
            | DesktopEvent::KernelProgress { run, .. }
            | DesktopEvent::Finished { run, .. }
            | DesktopEvent::Cancelled { run, .. }
            | DesktopEvent::Failed { run, .. } => *run,
        }
    }

    /// ARC-004: the event's typed kind name.
    ///
    /// This match is deliberately exhaustive and the enum is deliberately not `#[non_exhaustive]`:
    /// adding a variant makes this method (and the variant-enumeration test) fail to compile
    /// until the new event has a name and a consumer. No stringly-typed `kind` field exists on
    /// the wire; the kind is a method over the enum.
    #[allow(dead_code)] // used by the ARC-004 variant test; the window matches variants directly
    pub fn kind(&self) -> &'static str {
        match self {
            DesktopEvent::RunStarted { .. } => "RunStarted",
            DesktopEvent::StepStarted { .. } => "StepStarted",
            DesktopEvent::StepCommitted { .. } => "StepCommitted",
            DesktopEvent::Output { .. } => "Output",
            DesktopEvent::SubProgress { .. } => "SubProgress",
            DesktopEvent::KernelProgress { .. } => "KernelProgress",
            DesktopEvent::Finished { .. } => "Finished",
            DesktopEvent::Cancelled { .. } => "Cancelled",
            DesktopEvent::Failed { .. } => "Failed",
        }
    }

    /// ARC-004: every variant name, in declaration order.
    ///
    /// A test constructs one of each variant and asserts this list has no duplicate and that
    /// each event's [`Self::kind`] is in it, so the variant set and its consumers cannot drift.
    #[allow(dead_code)] // used by the ARC-004 variant test
    pub const KIND_NAMES: [&'static str; 9] = [
        "RunStarted",
        "StepStarted",
        "StepCommitted",
        "Output",
        "SubProgress",
        "KernelProgress",
        "Finished",
        "Cancelled",
        "Failed",
    ];
}

/// An event with the delivery-order sequence number and the revision it was produced for.
#[derive(Clone, Debug, PartialEq)]
pub struct SequencedEvent {
    /// Strictly increasing delivery order, starting at 1.
    pub sequence: u64,
    /// The source revision the event belongs to.
    pub revision: SourceRevision,
    /// The typed event.
    pub event: DesktopEvent,
}

/// Why a run could not be submitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmitError {
    /// That document already has an active run.
    Busy,
    /// The worker thread could not be created.
    Spawn(String),
}

/// A position in the full source, attached to an engine failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// The engine's message, unchanged.
    pub message: String,
    /// RUN-014: the engine's stable wire code, when it publishes one.
    ///
    /// `None` for an engine without the [`crate::engine::Evaluator::last_error`] seam; the log and
    /// the editor header then show the message and position without inventing a code (the C#
    /// oracle itself leaves `diagnostics[].code` empty — reference/oracle-defects.md OD-2).
    pub code: Option<String>,
    /// RUN-014: the engine's stable wire category, when it publishes one.
    pub category: Option<String>,
    /// RUN-014: the closest registered names the engine offered for an unresolved name.
    pub suggestions: Vec<String>,
    /// The 0-based index of the statement that failed.
    pub statement_index: usize,
    /// The 1-based line the failing statement starts on.
    pub statement_line: usize,
    /// The absolute byte offset of the failure in the full source.
    pub byte_offset: usize,
    /// The 1-based line of the failure in the full source.
    pub line: usize,
    /// The 1-based column of the failure in the full source.
    pub column: usize,
}

impl Diagnostic {
    /// Builds a diagnostic for a statement, translating an engine position when the message has one.
    ///
    /// The engine reports a position as a **zero-based UTF-16 code-unit offset** inside the source
    /// it was given (`lovelace_lang::LangError::position`). When the message names one (for
    /// example "at position 6.") the code-unit offset is converted to a byte offset inside the
    /// statement and added to the statement's absolute offset. The conversion clamps to the
    /// statement text so an out-of-range position can never point outside the full source. A
    /// message without a position points at the statement start.
    pub fn for_statement(
        statement: &Statement,
        source: &str,
        message: impl Into<String>,
    ) -> Diagnostic {
        let message = message.into();
        let relative = position_in_message(&message)
            .map(|position| byte_offset_utf16(statement.text(), position))
            .unwrap_or(0);
        let byte_offset = statement.byte_offset() + relative;
        let (line, column) = crate::statements::line_column(source, byte_offset);
        Diagnostic {
            message,
            code: None,
            category: None,
            suggestions: Vec::new(),
            statement_index: statement.index(),
            statement_line: statement.start_line(),
            byte_offset,
            line,
            column,
        }
    }

    /// Attaches the engine's structured classification (RUN-014).
    ///
    /// The classification is only ever copied from the engine that produced the failure, so a
    /// diagnostic either names the engine's own code and category or names none.
    pub fn classify(&mut self, error: Option<crate::engine::EngineError>) {
        let Some(error) = error else {
            return;
        };
        if error.is_empty() {
            return;
        }
        if !error.code.is_empty() {
            self.code = Some(error.code);
        }
        if !error.category.is_empty() {
            self.category = Some(error.category);
        }
        self.suggestions = error.suggestions;
    }

    /// Renders the diagnostic for a [WorkspaceSnapshot] and the log pane.
    pub fn render(&self) -> String {
        format!(
            "line {}, column {}: {}",
            self.line, self.column, self.message
        )
    }

    /// Renders every field RUN-014 requires: code, category, message, line and column, plus the
    /// engine's suggestions when it offered any.
    ///
    /// The field names are spelled out so a log line or a hover text is self-describing and a
    /// reader (or a UIA capture) can assert each field independently.
    pub fn render_fully(&self) -> String {
        let mut text = String::new();
        match (&self.code, &self.category) {
            (Some(code), Some(category)) => {
                text.push_str(&format!("code {code}, category {category}, "));
            }
            (Some(code), None) => text.push_str(&format!("code {code}, ")),
            (None, Some(category)) => text.push_str(&format!("category {category}, ")),
            (None, None) => {}
        }
        text.push_str(&format!(
            "line {}, column {}, message: {}",
            self.line, self.column, self.message
        ));
        if !self.suggestions.is_empty() {
            text.push_str(&format!(" suggestions: {}.", self.suggestions.join(", ")));
        }
        text
    }
}

/// Converts a zero-based UTF-16 code-unit offset into a byte offset in `text`.
///
/// An offset inside a surrogate pair clamps back to the start of that code point; an offset past
/// the end clamps to the text length.
fn byte_offset_utf16(text: &str, units: usize) -> usize {
    let mut consumed = 0usize;
    for (byte_index, character) in text.char_indices() {
        if consumed >= units {
            return byte_index;
        }
        let next = consumed + character.len_utf16();
        if units < next {
            return byte_index;
        }
        consumed = next;
    }
    text.len()
}

/// The first decimal number following "position" in an engine message, when there is one.
fn position_in_message(message: &str) -> Option<usize> {
    let marker = "position";
    let after_marker = message.find(marker)? + marker.len();
    let digits = message[after_marker..]
        .trim_start()
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// Creates one fresh evaluator for one run. It is called on the worker thread.
pub type EvaluatorFactory = Box<dyn Fn() -> Box<dyn Evaluator> + Send + Sync>;

struct RawEvent {
    revision: SourceRevision,
    event: DesktopEvent,
}

#[derive(Debug)]
struct RunControl {
    cancelled: AtomicBool,
    finished: AtomicBool,
}

impl RunControl {
    fn new() -> RunControl {
        RunControl {
            cancelled: AtomicBool::new(false),
            finished: AtomicBool::new(false),
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}

#[derive(Default)]
struct HostState {
    active: HashMap<DocumentId, ActiveRun>,
}

/// One document's in-flight run, with both cancellation seams the host owns.
struct ActiveRun {
    run: RunId,
    control: Arc<RunControl>,
    /// The live flag handed to the engine for this run. `cancel` and `shutdown` set it from the
    /// caller's thread while the kernel is running.
    cancel_flag: Arc<AtomicBool>,
}

/// A submitted run: its identity, its revision and its cancellation token.
#[derive(Clone)]
pub struct RunHandle {
    document: DocumentId,
    run: RunId,
    /// The submitted revision, exposed through [RunHandle::revision] for hosts that label runs;
    /// the execution core itself does not need to read it back.
    #[allow(dead_code)]
    revision: SourceRevision,
    control: Arc<RunControl>,
    /// The OS thread the worker was spawned on. ARC-005: a host can prove evaluation happened on
    /// this thread and not on the thread that owns the window.
    #[allow(dead_code)] // read through the host-facing accessor and the ARC-005 test
    worker_thread: std::thread::ThreadId,
}

impl RunHandle {
    /// The document this handle belongs to.
    #[allow(dead_code)] // host-facing accessor used by the execution tests
    pub fn document(&self) -> DocumentId {
        self.document
    }

    /// The run's identity.
    pub fn run(&self) -> RunId {
        self.run
    }

    /// ARC-005: the identity of the worker thread that owns this run's engine.
    #[allow(dead_code)] // host-facing proof accessor used by the ARC-005 test
    pub fn worker_thread(&self) -> std::thread::ThreadId {
        self.worker_thread
    }

    /// The source revision the run was submitted for.
    #[allow(dead_code)] // part of the host-facing handle API; the core tracks the revision itself
    pub fn revision(&self) -> SourceRevision {
        self.revision
    }

    /// True while the run has not delivered its terminal event.
    pub fn is_active(&self) -> bool {
        !self.control.is_finished()
    }
}

/// Owns the worker threads and the event queue for every document.
pub struct ExecutionHost {
    factory: Arc<dyn Fn() -> Box<dyn Evaluator> + Send + Sync>,
    sender: Sender<RawEvent>,
    receiver: Mutex<Receiver<RawEvent>>,
    sequence: AtomicU64,
    state: Arc<Mutex<HostState>>,
    next_run: AtomicU64,
    snapshots: Arc<Mutex<HashMap<DocumentId, WorkspaceSnapshot>>>,
    /// INC-001: one incremental cache per document, alive for the host's lifetime.
    ///
    /// A run takes its document's cache for the duration of the run (runs of one document are
    /// exclusive, so no other worker can want it) and puts it back *before* its terminal event is
    /// published, so that event never describes a state in which the cache is still out with the
    /// worker. A [`ExecutionHost::clear_cache`] during a run installs a fresh empty slot instead;
    /// the finishing worker then leaves that slot alone, so "Run All Fresh" always wins.
    caches: Arc<Mutex<HashMap<DocumentId, DocumentCache>>>,
    /// ARC-008: every worker `JoinHandle`, keyed by run. `next_event` joins handles that have
    /// already finished; [`ExecutionHost::shutdown`] joins all remaining ones after cancelling
    /// their runs, and `Drop` calls it. No handle is dropped unjoined.
    workers: Mutex<HashMap<RunId, thread::JoinHandle<()>>>,
}

impl ExecutionHost {
    /// Creates a host with no active runs.
    pub fn new(factory: EvaluatorFactory) -> ExecutionHost {
        let (sender, receiver) = mpsc::channel();
        ExecutionHost {
            factory: Arc::from(factory),
            sender,
            receiver: Mutex::new(receiver),
            sequence: AtomicU64::new(0),
            state: Arc::new(Mutex::new(HostState::default())),
            next_run: AtomicU64::new(0),
            snapshots: Arc::new(Mutex::new(HashMap::new())),
            caches: Arc::new(Mutex::new(HashMap::new())),
            workers: Mutex::new(HashMap::new()),
        }
    }

    /// Submits a run for one document.
    ///
    /// Returns [SubmitError::Busy] when that document already has an active run; other documents
    /// are unaffected. The call only records the run and spawns its worker; it never evaluates
    /// source, so a slow engine cannot block the caller.
    pub fn submit(
        &self,
        document: DocumentId,
        revision: SourceRevision,
        source: String,
        precision: usize,
    ) -> Result<RunHandle, SubmitError> {
        crate::run_trace::mark("submit_enter", format!("document={}", document.0));
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.active.contains_key(&document) {
            return Err(SubmitError::Busy);
        }
        let run = RunId(self.next_run.fetch_add(1, Ordering::SeqCst) + 1);
        let control = Arc::new(RunControl::new());
        // One live flag per run: the worker gives it to its engine, the handle keeps a clone so
        // `cancel()` can set it from the UI thread while the kernel is running.
        let cancel_flag = Arc::new(AtomicBool::new(false));
        state.active.insert(
            document,
            ActiveRun {
                run,
                control: Arc::clone(&control),
                cancel_flag: Arc::clone(&cancel_flag),
            },
        );
        drop(state);

        let statements = crate::statements::slice(&source);
        let worker = RunWorker {
            factory: Arc::clone(&self.factory),
            sender: self.sender.clone(),
            state: Arc::clone(&self.state),
            snapshots: Arc::clone(&self.snapshots),
            caches: Arc::clone(&self.caches),
            document,
            run,
            revision,
            source,
            precision,
            statements,
            control: Arc::clone(&control),
            cancel_flag: Arc::clone(&cancel_flag),
        };
        let spawned = thread::Builder::new()
            .name(format!("lovelace-run-{}-{}", document.0, run.0))
            .spawn(move || worker.run());
        match spawned {
            Ok(handle) => {
                let worker_thread = handle.thread().id();
                // ARC-008: the handle is retained until `shutdown` (or a non-blocking reap after
                // the thread has finished). Dropping it here would detach the worker.
                self.workers
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .insert(run, handle);
                crate::run_trace::mark(
                    "submit_exit",
                    format!("document={} run={}", document.0, run.0),
                );
                Ok(RunHandle {
                    document,
                    run,
                    revision,
                    control,
                    worker_thread,
                })
            }
            Err(error) => {
                let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
                if let Some(active) = state.active.get(&document) {
                    if active.run == run {
                        state.active.remove(&document);
                    }
                }
                control.finished.store(true, Ordering::Release);
                Err(SubmitError::Spawn(error.to_string()))
            }
        }
    }

    /// Requests cancellation of a run.
    ///
    /// Sets both cancellation seams: the statement-boundary token and the live flag the engine
    /// polls inside a long kernel. Returns true when the token was accepted, false when the run
    /// already finished or was already cancelled.
    pub fn cancel(&self, handle: &RunHandle) -> bool {
        if handle.control.is_finished() {
            return false;
        }
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        match state.active.get(&handle.document) {
            Some(active)
                if active.run == handle.run
                    && !active.control.is_finished()
                    && !active.control.is_cancelled() =>
            {
                active.control.cancelled.store(true, Ordering::Release);
                active.cancel_flag.store(true, Ordering::Release);
                true
            }
            _ => false,
        }
    }

    /// Waits up to `timeout` for the next event and assigns it a delivery sequence number.
    pub fn next_event(&self, timeout: Duration) -> Option<SequencedEvent> {
        // ARC-008: a finished worker's handle is joined here without blocking; it is never
        // detached. A still-running worker is left in the table for `shutdown`.
        self.reap_finished_workers();
        let receiver = self
            .receiver
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match receiver.recv_timeout(timeout) {
            Ok(raw) => {
                let sequence = self.sequence.fetch_add(1, Ordering::SeqCst) + 1;
                Some(SequencedEvent {
                    sequence,
                    revision: raw.revision,
                    event: raw.event,
                })
            }
            Err(_) => None,
        }
    }

    /// Joins every worker whose OS thread has already finished, without waiting for a live one.
    fn reap_finished_workers(&self) {
        let mut workers = self
            .workers
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let finished: Vec<RunId> = workers
            .iter()
            .filter(|(_, handle)| handle.is_finished())
            .map(|(run, _)| *run)
            .collect();
        for run in finished {
            if let Some(handle) = workers.remove(&run) {
                let _ = handle.join();
            }
        }
    }

    /// ARC-008: cancels every active run and joins every worker the host ever spawned.
    ///
    /// Returns how many workers this call joined. The call is idempotent: a second call after the
    /// table is empty returns 0. `Drop for ExecutionHost` calls it, so a closed window or a
    /// dropped [`crate::workbench::Workbench`] never leaves a run thread behind.
    ///
    /// A worker whose engine ignores the cancellation flag and never returns from one kernel
    /// delays this join; that case is the documented uninterruptible-kernel budget (RUN-008),
    /// not a detached thread.
    pub fn shutdown(&self) -> usize {
        {
            let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            for active in state.active.values() {
                active.control.cancelled.store(true, Ordering::Release);
                active.cancel_flag.store(true, Ordering::Release);
            }
        }
        self.reap_finished_workers();
        let handles: Vec<thread::JoinHandle<()>> = {
            let mut workers = self
                .workers
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            workers.drain().map(|(_, handle)| handle).collect()
        };
        let joined = handles.len();
        for handle in handles {
            let _ = handle.join();
        }
        joined
    }

    /// ARC-008: workers not yet joined. Used by the shutdown tests to prove the join path.
    #[allow(dead_code)] // test-facing invariant accessor
    pub fn live_worker_count(&self) -> usize {
        self.workers
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }

    /// The last snapshot published for a document, if any.
    ///
    /// Terminal events now carry the same snapshot, so the workbench does not need this lookup;
    /// it stays as the host-facing proof that publishing and the event agree (execution tests).
    #[allow(dead_code)] // host-facing accessor exercised by the execution tests
    pub fn last_snapshot(&self, document: DocumentId) -> Option<WorkspaceSnapshot> {
        let snapshots = self
            .snapshots
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        snapshots.get(&document).cloned()
    }

    /// Drops the document's incremental cache (INC-009, "Run All Fresh").
    ///
    /// The next run of that document evaluates every statement and re-fills the cache. A run
    /// already in flight keeps its own copy and does not repopulate this slot when it finishes:
    /// the fresh request wins over the older run's cache. Nothing else changes, so the fresh run
    /// yields the same values and output as an incremental one.
    pub fn clear_cache(&self, document: DocumentId) {
        let mut caches = self
            .caches
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        // Drop the memoised statements but keep the document's function revisions, so the fresh
        // run builds exactly the keys an incremental run would have built.
        caches.entry(document).or_default().clear_entries();
    }

    /// How many statement outcomes the document's cache currently holds.
    #[allow(dead_code)] // host-facing accessor exercised by the incremental tests
    pub fn cached_entries(&self, document: DocumentId) -> usize {
        let caches = self
            .caches
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        caches.get(&document).map(DocumentCache::len).unwrap_or(0)
    }

    /// Drops exactly the cached statements a removed global affects (WKS-003).
    ///
    /// The workspace deletion changes the document's engine state without a source edit, so the
    /// memoised outcomes that read or re-create that global must not be reused by the next run.
    /// Returns how many entries were dropped; a document with no cache drops none.
    pub fn invalidate_variable(&self, document: DocumentId, name: &str) -> usize {
        let mut caches = self
            .caches
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        caches
            .entry(document)
            .or_default()
            .invalidate_variable(name)
    }
}

impl Drop for ExecutionHost {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Everything one worker accumulates while it visits the sliced statements.
struct Progress {
    /// Index of the statement currently being evaluated.
    current: usize,
    /// Statements whose effects are part of the workspace.
    committed: usize,
    /// Display form of the last committed value ("Void" before any).
    result: String,
    /// The engine's structured projection of the last committed value, when it publishes one.
    result_structured: Option<Json>,
    /// SYM-001: the engine's inspection payload for that same committed value.
    result_inspection: Option<Json>,
    /// Print lines drained from the engine, in order.
    output: Vec<String>,
    /// Variables observed after the last committed statement.
    variables: Vec<(String, String)>,
    /// Functions observed after the last committed statement.
    functions: Vec<(String, Vec<String>)>,
    /// The engine's kind-aware symbol capture for the last committed statement.
    state: EngineState,
    /// Most recent plot observed during the run. A statement that produces no plot does not
    /// erase an earlier one: the real engine clears its own capture at the start of every
    /// `eval`, and this core evaluates one statement at a time.
    plot: Option<PlotCapture>,
    /// RUN-020: the engine's current display precision, captured at each commit point.
    ///
    /// It starts at the precision submitted with the run and follows a `setprecision(n)`
    /// statement, so the terminal snapshot names the precision the engine actually rendered at.
    precision: usize,
    /// True once a terminal event has been published for the run.
    ended: bool,
}

impl Default for Progress {
    fn default() -> Progress {
        Progress {
            current: 0,
            committed: 0,
            result: "Void".to_string(),
            result_structured: None,
            result_inspection: None,
            output: Vec::new(),
            variables: Vec::new(),
            functions: Vec::new(),
            state: EngineState::default(),
            plot: None,
            precision: DEFAULT_PRECISION,
            ended: false,
        }
    }
}

/// Owns one run on its worker thread.
struct RunWorker {
    factory: Arc<dyn Fn() -> Box<dyn Evaluator> + Send + Sync>,
    sender: Sender<RawEvent>,
    state: Arc<Mutex<HostState>>,
    snapshots: Arc<Mutex<HashMap<DocumentId, WorkspaceSnapshot>>>,
    /// The host's per-document cache table; this run takes its document's entry from it.
    caches: Arc<Mutex<HashMap<DocumentId, DocumentCache>>>,
    document: DocumentId,
    run: RunId,
    revision: SourceRevision,
    source: String,
    precision: usize,
    statements: Vec<Statement>,
    control: Arc<RunControl>,
    /// The live flag handed to the engine for this run; [`ExecutionHost::cancel`] sets the other
    /// clone.
    cancel_flag: Arc<AtomicBool>,
}

impl RunWorker {
    /// Runs the whole script, turning every exit path into exactly one terminal event.
    fn run(self) {
        crate::run_trace::mark(
            "worker_start",
            format!("document={} run={}", self.document.0, self.run.0),
        );
        // The document's incremental cache is owned by this run for its duration. Runs of one
        // document are exclusive (`SubmitError::Busy`), and the cache goes back before the
        // terminal event on every exit path, so the next run reuses across runs and no observer
        // of the terminal event can catch the cache still out with this worker.
        let mut cache = self.take_cache();
        let mut progress = Progress::default();
        progress.precision = self.precision;
        match panic::catch_unwind(AssertUnwindSafe(|| self.execute(&mut progress, &mut cache))) {
            Ok(()) => {}
            Err(payload) => {
                if !progress.ended {
                    let diagnostic = self.diagnostic_for(progress.current, panic_text(payload));
                    let snapshot = self.snapshot(&progress, Some(diagnostic.render_fully()));
                    self.publish(snapshot.clone());
                    // Same order as every other terminal path: the settled state first, the event
                    // last.
                    self.return_cache(&mut cache);
                    self.retire();
                    self.emit(DesktopEvent::Failed {
                        document: self.document,
                        run: self.run,
                        diagnostic,
                        snapshot,
                    });
                    progress.ended = true;
                }
            }
        }
        // Every terminal path hands the cache back before it publishes the terminal event, so a
        // second hand-back from here is not harmless: it can run after the next run's
        // `take_cache` and install an empty slot, which makes the run after that miss every
        // entry. Only a path that never reached a terminal hand-back needs this fallback.
        if !progress.ended {
            self.return_cache(&mut cache);
        }
    }

    /// Takes this document's cache out of the host table for the run.
    fn take_cache(&self) -> DocumentCache {
        let mut caches = self
            .caches
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        caches.remove(&self.document).unwrap_or_default()
    }

    /// Puts the cache back unless a Clear/`clear_cache` installed a fresh slot meanwhile.
    ///
    /// Idempotent, because every terminal path calls it and the worker's exit calls it again: the
    /// first call moves the entries out, and a slot that already exists is a fresh slot installed
    /// by "Run All Fresh" (or a previous call) and is left untouched.
    ///
    /// It is called *before* the terminal event is emitted. The document's cache is part of the
    /// run's settled state, so a client that reads it — or, far more importantly, submits the next
    /// run — the moment it observes `Finished`/`Failed`/`Cancelled` must find it already back.
    /// Returning it after the terminal event left the submitter racing the hand-back: the next run
    /// took the removed slot as an empty cache and silently re-evaluated every statement instead
    /// of reusing, which is what the release suite's `left: 0, right: 4` cache-entry failures were.
    fn return_cache(&self, cache: &mut DocumentCache) {
        let mut caches = self
            .caches
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !caches.contains_key(&self.document) {
            caches.insert(self.document, std::mem::take(cache));
        }
    }

    /// True when either cancellation seam has been signalled for this run.
    fn cancel_requested(&self) -> bool {
        self.control.is_cancelled() || self.cancel_flag.load(Ordering::Acquire)
    }

    /// Ends the run as cancelled: publishes the committed snapshot, hands the cache back, retires
    /// the document and emits exactly one terminal `Cancelled` event.
    ///
    /// RUN-007: a statement that completed before the cancellation was observed is committed by
    /// the caller (`Completed`). An in-flight statement stopped by the live flag (`Interrupted`) is
    /// **not** committed: its partial state is discarded and only the committed prefix is
    /// published, which is the transactional contract for `COR-016`. `None` means cancellation was
    /// observed before the next statement started.
    ///
    /// The cache goes back before the event (see [`RunWorker::return_cache`]): the event is the
    /// signal a client acts on, so everything the run leaves behind must be settled by then.
    fn finish_cancelled(
        &self,
        progress: &mut Progress,
        cache: &mut DocumentCache,
        in_flight: InFlightCancellation,
    ) {
        let snapshot = self.snapshot(progress, None);
        self.publish(snapshot.clone());
        self.return_cache(cache);
        self.retire();
        self.emit(DesktopEvent::Cancelled {
            document: self.document,
            run: self.run,
            committed_steps: snapshot.committed_steps,
            in_flight,
            snapshot,
        });
        progress.ended = true;
    }

    /// Evaluates (or reuses) every statement, publishing typed events on the way.
    fn execute(&self, progress: &mut Progress, cache: &mut DocumentCache) {
        crate::run_trace::mark("engine_build_enter", "engine factory");
        let engine = match panic::catch_unwind(AssertUnwindSafe(|| (self.factory)())) {
            Ok(engine) => engine,
            Err(payload) => {
                let diagnostic = self.diagnostic_for(progress.current, panic_text(payload));
                self.fail(progress, diagnostic, cache);
                return;
            }
        };
        crate::run_trace::mark("engine_constructed", "engine factory returned");
        let mut engine = engine;
        let _ = panic::catch_unwind(AssertUnwindSafe(|| engine.set_precision(self.precision)));
        // The live kernel seam: the engine keeps this handle for the whole run and its kernels
        // poll it beside their deadline, so Cancel does not have to wait for a statement boundary.
        let cancel_flag = Arc::clone(&self.cancel_flag);
        let _ = panic::catch_unwind(AssertUnwindSafe(|| {
            engine.set_cancel_flag(Some(cancel_flag));
        }));
        // RUN-005: the kernel sub-progress sink. The engine arms it for each evaluation below, so
        // a kernel inside the statement reports its phase to the run's event queue while the
        // statement is still in flight. An engine without the seam ignores it (the trait default).
        let _ = panic::catch_unwind(AssertUnwindSafe(|| {
            engine.set_kernel_progress_sink(Some(self.kernel_progress_sink()));
        }));
        // INC-001: the run's source-level dependency tables are built once, and the engine's own
        // registry (a fresh engine, so builtins only) supplies the pure-callable set the
        // effect classification consults.
        let builtins = self.engine_builtins(&engine);
        let analysis = ProgramAnalysis::new(&self.statements, builtins);
        let cache_supported =
            panic::catch_unwind(AssertUnwindSafe(|| engine.supports_cached_values()))
                .unwrap_or(false);
        self.emit(DesktopEvent::RunStarted {
            document: self.document,
            run: self.run,
            revision: self.revision,
            total: self.statements.len(),
        });
        crate::run_trace::mark(
            "run_started_emitted",
            format!(
                "total={} document={}",
                self.statements.len(),
                self.document.0
            ),
        );

        for (index, statement) in self.statements.iter().enumerate() {
            progress.current = index;
            // RUN-007: the boundary is checked once, before the statement starts. A cancellation
            // that is already pending means this statement never runs; one that arrives while it
            // is in flight is handled at the commit below, so the statement's outcome is never
            // thrown away just because the request beat it to the engine.
            if self.cancel_requested() {
                self.finish_cancelled(progress, cache, InFlightCancellation::None);
                return;
            }
            self.emit(DesktopEvent::StepStarted {
                document: self.document,
                run: self.run,
                index,
                label: statement.label(),
            });

            // INC-001/INC-002: a memoizable, effect-free statement is keyed from the typed
            // values it reads *right now* (before it can change them). A hit is applied to the
            // engine and committed without evaluating; a miss evaluates and stores under the
            // same pre-evaluation key.
            let mut store: Option<(StatementKey, Option<String>)> = None;
            if cache_supported {
                let effective = self.engine_precision(&engine);
                let decision = cache.plan(&analysis, index, self.precision, effective, &*engine);
                if let StepDecision::Keyed { key, write } = decision {
                    let cached = cache.get(&key).cloned();
                    if let Some(entry) = cached {
                        if self.apply_cached(&mut engine, progress, &entry) {
                            self.commit_step(progress, index, true);
                            // RUN-007: a reused value is committed in the same place an
                            // evaluated one is, so a cancellation that arrived during the reuse
                            // stops the run *after* this statement, not before its commit.
                            if self.cancel_requested() {
                                self.finish_cancelled(
                                    progress,
                                    cache,
                                    InFlightCancellation::Completed,
                                );
                                return;
                            }
                            continue;
                        }
                    }
                    store = Some((key, write));
                }
            }

            match panic::catch_unwind(AssertUnwindSafe(|| engine.eval(statement.text()))) {
                Ok(Ok(value)) => {
                    progress.result = value;
                    // RES-001: the engine's projection of the value this statement returned,
                    // taken at the same commit point as the display the panes show. An engine
                    // that publishes no projection leaves the field as None; the desktop does
                    // not substitute a structure parsed out of the display.
                    if let Ok(structured) =
                        panic::catch_unwind(AssertUnwindSafe(|| engine.last_structured()))
                    {
                        progress.result_structured = structured;
                    } else {
                        progress.result_structured = None;
                    }
                    // SYM-001: the engine's inspection of the same value, captured at the same
                    // commit point. An engine that publishes none leaves it None; the Inspector
                    // then shows its explicit "the engine published no inspection" state.
                    if let Ok(inspection) =
                        panic::catch_unwind(AssertUnwindSafe(|| engine.last_inspection()))
                    {
                        progress.result_inspection = inspection;
                    } else {
                        progress.result_inspection = None;
                    }
                    self.drain_output(&mut engine, progress);
                    progress.committed += 1;
                    // RUN-020: a `setprecision(n)` statement changes the engine's precision; the
                    // snapshot carries what the engine actually uses, not what was submitted.
                    progress.precision = self.engine_precision(&engine);
                    // The kind-aware capture the completion catalogue reads. It is taken at the
                    // same commit point as the pane lists, so the popup can never offer a
                    // variable the displayed workspace does not have.
                    self.capture_symbols(&mut engine, progress);
                    if let Ok(Some(plot)) =
                        panic::catch_unwind(AssertUnwindSafe(|| engine.last_plot()))
                    {
                        // Keep the last plot the run produced. The engine resets its capture
                        // at the start of each `eval`, so a later statement legitimately
                        // reports None; that must not erase the plot from an earlier one.
                        progress.plot = Some(plot);
                    }
                    // Store the evaluated outcome under the key built from the inputs the
                    // statement actually had. The typed value is the engine's own, never a
                    // display string re-parsed on reuse.
                    if let Some((key, write)) = store.take() {
                        if let Some(entry) = self.cached_entry(&engine, progress, write) {
                            cache.insert(key, entry);
                        }
                    }
                    // INC-003: definitions always evaluate; the revision moves only when the
                    // combined normalized definition text changed, which invalidates call sites.
                    if let Some(name) = analysis.definition_name(index) {
                        if let Some(signature) = analysis.definition_signature(name) {
                            cache.note_definition(name, signature);
                        }
                    }
                    self.commit_step(progress, index, false);
                    // RUN-007: the in-flight statement has finished and its value is committed;
                    // only now does the run stop, before the next statement can start. This is
                    // the web Studio's statement-boundary contract, made explicit here instead
                    // of being left to the next loop-top check.
                    if self.cancel_requested() {
                        self.finish_cancelled(progress, cache, InFlightCancellation::Completed);
                        return;
                    }
                }
                Ok(Err(message)) => {
                    self.drain_output(&mut engine, progress);
                    // RUN-014: the engine's own code/category/suggestions for exactly this
                    // failure, captured from the evaluator that returned it.
                    let classification =
                        panic::catch_unwind(AssertUnwindSafe(|| engine.last_error()))
                            .ok()
                            .flatten();
                    // RUN-007/RUN-008: an engine that observed the live cancellation flag stops
                    // the statement early and reports the structured `Cancelled` refusal. That is
                    // the in-flight statement's early-exit result, not a diagnostic, and it is
                    // committed like any other statement outcome. An engine without a
                    // classification seam (the deterministic fake and third-party evaluators)
                    // cannot publish the code, so the pending request itself is the discriminator
                    // it already used before this seam existed. A *different* classified failure
                    // still wins over a pending cancellation, matching the web Studio's
                    // "error wins" edge.
                    let stopped_by_cancellation = match &classification {
                        Some(error) => error.code == "Cancelled",
                        None => self.cancel_requested(),
                    };
                    if self.cancel_requested() && stopped_by_cancellation {
                        // COR-016: the interrupted statement's partial engine state is discarded.
                        // The engine is dropped with the run and only the committed prefix is
                        // published; no partial workspace, output or stack entry is committed.
                        self.finish_cancelled(progress, cache, InFlightCancellation::Interrupted);
                        return;
                    }
                    let mut diagnostic =
                        Diagnostic::for_statement(statement, &self.source, message);
                    diagnostic.classify(classification);
                    self.fail(progress, diagnostic, cache);
                    return;
                }
                Err(payload) => {
                    let diagnostic =
                        Diagnostic::for_statement(statement, &self.source, panic_text(payload));
                    self.fail(progress, diagnostic, cache);
                    return;
                }
            }
        }

        // A cancellation observed after the last statement committed but before the terminal
        // event is still honoured: the run ends `Cancelled` with the full prefix, never
        // `Finished` while a request is pending (RUN-007).
        if self.cancel_requested() {
            self.finish_cancelled(progress, cache, InFlightCancellation::Completed);
            return;
        }

        let snapshot = self.snapshot(progress, None);
        self.publish(snapshot.clone());
        // INC-001: the cache is part of the snapshot of settled state the terminal event reports,
        // so it goes back before that event can be observed. A client that submits the next run on
        // `Finished` must not be able to take an empty cache and re-evaluate every statement.
        self.return_cache(cache);
        self.retire();
        self.emit(DesktopEvent::Finished {
            document: self.document,
            run: self.run,
            snapshot,
        });
        progress.ended = true;
    }

    /// Captures the engine's variables, functions and kind-aware state at a commit point.
    ///
    /// Shared by the evaluate and reuse paths so a reused commit publishes the same coherent
    /// pane data a fresh evaluation would have. WKS-002: the registry capture is the one source
    /// of truth for the function table — rebuilding the legacy pair list from it keeps the
    /// snapshot's function count equal to the count the Workspace table displays.
    fn capture_symbols(&self, engine: &mut Box<dyn Evaluator>, progress: &mut Progress) {
        if let Ok(variables) = panic::catch_unwind(AssertUnwindSafe(|| engine.variables())) {
            progress.variables = variables;
        }
        if let Ok(functions) = panic::catch_unwind(AssertUnwindSafe(|| engine.functions())) {
            progress.functions = functions;
        }
        if let Ok(Some(state)) = panic::catch_unwind(AssertUnwindSafe(|| engine.state())) {
            progress.state = state;
        }
        if !progress.state.functions.is_empty() {
            progress.functions = progress
                .state
                .functions
                .iter()
                .map(|function| (function.name.clone(), function.parameters.clone()))
                .collect();
        }
    }

    /// Emits the commit and its progress event for one statement.
    fn commit_step(&self, progress: &Progress, index: usize, reused: bool) {
        self.emit(DesktopEvent::StepCommitted {
            document: self.document,
            run: self.run,
            index,
            reused,
            // RUN-003: the whole committed prefix travels with the commit event, so the panes
            // and the log advance from one object.
            snapshot: self.snapshot(progress, None),
        });
        let total = self.statements.len().max(1);
        self.emit(DesktopEvent::SubProgress {
            document: self.document,
            run: self.run,
            fraction: progress.committed as f32 / total as f32,
            label: format!("{}/{}", progress.committed, self.statements.len()),
        });
    }

    /// Applies a cached outcome to the engine and publishes the commit fields.
    ///
    /// INC-007: a reuse is genuine only when the cached typed value reaches the engine — the
    /// statement's write target and, for a non-`Void` value, the engine's `_` result binding (the
    /// engine stores `_` after every statement, so a fresh run would have set it too). Returning
    /// false makes the caller evaluate the statement instead, so no reuse is ever reported for a
    /// statement whose value was not applied.
    fn apply_cached(
        &self,
        engine: &mut Box<dyn Evaluator>,
        progress: &mut Progress,
        entry: &CachedEntry,
    ) -> bool {
        if let Some(write) = &entry.write {
            if !self.engine_apply(engine, write, &entry.value) {
                return false;
            }
        }
        // The engine's `_` binding is part of the workspace: a fresh run sets it after every
        // statement, so a reused non-Void value must set it too. An engine that publishes no
        // result variable says so (`tracks_result_variable`), and its reused runs stay identical
        // to fresh ones.
        if !entry.value.is_void()
            && self.engine_tracks_result_variable(engine)
            && !self.engine_apply(engine, "_", &entry.value)
        {
            return false;
        }
        progress.result = entry.display.clone();
        progress.result_structured = entry.structured.clone();
        progress.result_inspection = entry.inspection.clone();
        progress.committed += 1;
        progress.precision = self.engine_precision(engine);
        self.capture_symbols(engine, progress);
        true
    }

    /// Calls [`Evaluator::tracks_result_variable`] with panic isolation.
    fn engine_tracks_result_variable(&self, engine: &Box<dyn Evaluator>) -> bool {
        panic::catch_unwind(AssertUnwindSafe(|| engine.tracks_result_variable())).unwrap_or(false)
    }

    /// Calls [`Evaluator::apply_cached_value`] with panic isolation.
    fn engine_apply(
        &self,
        engine: &mut Box<dyn Evaluator>,
        name: &str,
        value: &CachedValue,
    ) -> bool {
        panic::catch_unwind(AssertUnwindSafe(|| engine.apply_cached_value(name, value)))
            .unwrap_or(false)
    }

    /// The typed value the engine just produced, as a cache entry, when the engine publishes one.
    fn cached_entry(
        &self,
        engine: &Box<dyn Evaluator>,
        progress: &Progress,
        write: Option<String>,
    ) -> Option<CachedEntry> {
        let value = panic::catch_unwind(AssertUnwindSafe(|| engine.last_value()))
            .ok()
            .flatten()?;
        Some(CachedEntry {
            write,
            display: progress.result.clone(),
            structured: progress.result_structured.clone(),
            inspection: progress.result_inspection.clone(),
            value,
        })
    }

    /// The engine's current display precision, or the submitted value when it publishes none.
    fn engine_precision(&self, engine: &Box<dyn Evaluator>) -> usize {
        panic::catch_unwind(AssertUnwindSafe(|| engine.precision())).unwrap_or(self.precision)
    }

    /// The names of the engine's pure-callable registry, for the effect classification.
    fn engine_builtins(&self, engine: &Box<dyn Evaluator>) -> std::collections::HashSet<String> {
        match panic::catch_unwind(AssertUnwindSafe(|| engine.state())) {
            Ok(Some(state)) => state
                .functions
                .iter()
                .filter(|function| function.is_builtin)
                .map(|function| function.name.clone())
                .collect(),
            _ => std::collections::HashSet::new(),
        }
    }

    /// Drains the engine's print output and publishes one event per line.
    fn drain_output(&self, engine: &mut Box<dyn Evaluator>, progress: &mut Progress) {
        let lines =
            panic::catch_unwind(AssertUnwindSafe(|| engine.take_output())).unwrap_or_default();
        for line in lines {
            self.emit(DesktopEvent::Output {
                document: self.document,
                run: self.run,
                text: line.clone(),
            });
            progress.output.push(line);
        }
    }

    /// Publishes a final snapshot and the single Failed event for the run.
    ///
    /// The cache goes back before the event (see [`RunWorker::return_cache`]), so even a failed
    /// run leaves its document's settled state behind the signal rather than in front of it.
    fn fail(&self, progress: &mut Progress, diagnostic: Diagnostic, cache: &mut DocumentCache) {
        // RUN-014: the committed snapshot carries every diagnostic field, not just the message.
        let snapshot = self.snapshot(progress, Some(diagnostic.render_fully()));
        self.publish(snapshot.clone());
        self.return_cache(cache);
        self.retire();
        self.emit(DesktopEvent::Failed {
            document: self.document,
            run: self.run,
            diagnostic,
            snapshot,
        });
        progress.ended = true;
    }

    /// Builds the coherent snapshot for the committed prefix plus an optional diagnostic.
    fn snapshot(&self, progress: &Progress, diagnostic: Option<String>) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            result: progress.result.clone(),
            result_structured: progress.result_structured.clone(),
            result_inspection: progress.result_inspection.clone(),
            variables: progress.variables.clone(),
            functions: progress.functions.clone(),
            state: progress.state.clone(),
            plot: progress.plot.clone(),
            output: progress.output.clone(),
            revision: self.revision,
            committed_steps: progress.committed,
            total: self.statements.len(),
            precision: progress.precision,
            diagnostic,
        }
    }

    /// Builds a diagnostic at `index`, or at the start of the source when there is no statement.
    fn diagnostic_for(&self, index: usize, message: String) -> Diagnostic {
        match self.statements.get(index) {
            Some(statement) => Diagnostic::for_statement(statement, &self.source, message),
            None => Diagnostic {
                message,
                code: None,
                category: None,
                suggestions: Vec::new(),
                statement_index: index,
                statement_line: 1,
                byte_offset: 0,
                line: 1,
                column: 1,
            },
        }
    }

    /// Removes the run from the host's active map and marks its handle inactive.
    fn retire(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(active) = state.active.get(&self.document) {
            if active.run == self.run {
                state.active.remove(&self.document);
            }
        }
        drop(state);
        self.control.finished.store(true, Ordering::Release);
    }

    fn publish(&self, snapshot: WorkspaceSnapshot) {
        let mut snapshots = self
            .snapshots
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        snapshots.insert(self.document, snapshot);
    }

    fn emit(&self, event: DesktopEvent) {
        let _ = self.sender.send(RawEvent {
            revision: self.revision,
            event,
        });
    }

    /// RUN-005: the callback this run's engine reports kernel sub-progress through.
    ///
    /// The callback runs on the worker thread INSIDE the evaluation, so it forwards the report
    /// straight into the run's event queue with the run identity attached: a report can never be
    /// attributed to another document or another run. Reports are decimated to a resolution the
    /// strip can use (one event per 0.5 % of kernel work, plus the first one and anything the
    /// engine reports once the work is done): `2000000!`'s product tree reports every 256
    /// multiplications, and forwarding all of those would flood the queue with values no pane
    /// could ever paint. The engine still sees every report — only the queue is decimated, and
    /// the fraction the strip shows is the engine's own.
    fn kernel_progress_sink(&self) -> crate::engine::KernelProgressSink {
        /// Permille between forwarded reports.
        const RESOLUTION_PERMILLE: u32 = 5;
        let sender = self.sender.clone();
        let document = self.document;
        let run = self.run;
        let revision = self.revision;
        let last = Arc::new(AtomicU32::new(0));
        let first = Arc::new(AtomicBool::new(true));
        Arc::new(move |phase: &str, fraction: f32| {
            let permille = (fraction.clamp(0.0, 1.0) * 1000.0) as u32;
            let previous = last.load(Ordering::Relaxed);
            if !first.swap(false, Ordering::Relaxed)
                && permille < previous.saturating_add(RESOLUTION_PERMILLE)
                && permille < 1000
            {
                return;
            }
            last.store(permille, Ordering::Relaxed);
            let _ = sender.send(RawEvent {
                revision,
                event: DesktopEvent::KernelProgress {
                    document,
                    run,
                    phase: phase.to_string(),
                    fraction,
                },
            });
        })
    }
}

/// Extracts the text out of a panic payload.
fn panic_text(payload: Box<dyn Any + Send>) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "engine panicked".to_string()
    }
}

/// Applies sequenced events to the workspace, ignoring results from a stale revision.
#[derive(Clone, Debug, Default)]
pub struct EventSink {
    revision: SourceRevision,
    snapshot: Option<WorkspaceSnapshot>,
}

impl EventSink {
    /// Creates an empty sink at revision zero.
    pub fn new() -> EventSink {
        EventSink::default()
    }

    /// Applies one event. Returns true when it was accepted, false when it was stale.
    ///
    /// Only [DesktopEvent::Finished] replaces the snapshot and advances the sink's revision; an
    /// event whose revision is older than the applied revision never changes the sink.
    pub fn apply(&mut self, event: &SequencedEvent) -> bool {
        if event.revision < self.revision {
            return false;
        }
        if let DesktopEvent::Finished { snapshot, .. } = &event.event {
            if snapshot.revision < self.revision {
                return false;
            }
            self.revision = snapshot.revision;
            self.snapshot = Some(snapshot.clone());
        }
        true
    }

    /// The revision of the applied snapshot.
    #[allow(dead_code)] // host-facing accessor used by the execution tests
    pub fn revision(&self) -> SourceRevision {
        self.revision
    }

    /// The applied snapshot, if a non-stale run finished.
    #[allow(dead_code)] // host-facing accessor used by the execution tests
    pub fn snapshot(&self) -> Option<&WorkspaceSnapshot> {
        self.snapshot.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::Instant;

    /// An engine whose every statement sleeps, so submission and cancellation timing are visible.
    struct SleepEngine {
        delay: Duration,
        calls: usize,
    }

    impl Evaluator for SleepEngine {
        fn eval(&mut self, _src: &str) -> Result<String, String> {
            thread::sleep(self.delay);
            self.calls += 1;
            Ok(format!("v{}", self.calls))
        }
    }

    fn sleep_factory(delay: Duration) -> EvaluatorFactory {
        Box::new(move || Box::new(SleepEngine { delay, calls: 0 }))
    }

    /// An engine that reports its executed-statement count as a variable, so a snapshot can be
    /// compared against the committed step count.
    struct ProgressEngine {
        delay: Duration,
        steps: usize,
    }

    impl Evaluator for ProgressEngine {
        fn eval(&mut self, _src: &str) -> Result<String, String> {
            thread::sleep(self.delay);
            self.steps += 1;
            Ok(self.steps.to_string())
        }

        fn variables(&self) -> Vec<(String, String)> {
            vec![("step".to_string(), self.steps.to_string())]
        }
    }

    fn progress_factory(delay: Duration) -> EvaluatorFactory {
        Box::new(move || Box::new(ProgressEngine { delay, steps: 0 }))
    }

    /// Panics on the first evaluation that reaches it, then succeeds. The flag is shared across
    /// engine instances, so the second run on the same document is healthy.
    struct PanicOnceEngine {
        panicked: Arc<AtomicBool>,
    }

    impl Evaluator for PanicOnceEngine {
        fn eval(&mut self, _src: &str) -> Result<String, String> {
            if !self.panicked.swap(true, Ordering::SeqCst) {
                panic!("boom");
            }
            Ok("ok".to_string())
        }
    }

    /// Produces two print lines per evaluation.
    #[derive(Default)]
    struct PrintingEngine {
        pending: Vec<String>,
    }

    impl Evaluator for PrintingEngine {
        fn eval(&mut self, _src: &str) -> Result<String, String> {
            self.pending.push("first".to_string());
            self.pending.push("second".to_string());
            Ok("done".to_string())
        }

        fn take_output(&mut self) -> Vec<String> {
            std::mem::take(&mut self.pending)
        }
    }

    /// Counts evaluations and returns a distinct display value for each.
    #[derive(Default)]
    struct CountingEngine {
        calls: usize,
    }

    impl Evaluator for CountingEngine {
        fn eval(&mut self, _src: &str) -> Result<String, String> {
            self.calls += 1;
            Ok(format!("v{}", self.calls))
        }
    }

    fn counting_factory() -> EvaluatorFactory {
        Box::new(|| Box::new(CountingEngine::default()))
    }

    /// An engine that publishes a cacheable typed value, so every committed statement leaves one
    /// entry in the document's incremental cache (INC-001).
    ///
    /// It exists to make the cache observable from the outside: `cached_entries` only tells "the
    /// cache is back" apart from "the cache is still out with the worker" when there is an entry
    /// in it, so a minimal evaluator with no reuse seam would make the ordering assertion below
    /// vacuous.
    #[derive(Default)]
    struct CacheableEngine {
        calls: usize,
    }

    impl Evaluator for CacheableEngine {
        fn eval(&mut self, _src: &str) -> Result<String, String> {
            self.calls += 1;
            Ok(self.calls.to_string())
        }

        fn supports_cached_values(&self) -> bool {
            true
        }

        fn last_value(&self) -> Option<CachedValue> {
            Some(CachedValue::Fake(crate::engine::FakeValue::Int(
                self.calls as i64,
            )))
        }

        fn apply_cached_value(&mut self, _name: &str, _value: &CachedValue) -> bool {
            true
        }
    }

    fn cacheable_factory() -> EvaluatorFactory {
        Box::new(|| Box::new(CacheableEngine::default()))
    }

    fn event_summary(event: &DesktopEvent) -> String {
        match event {
            DesktopEvent::RunStarted { total, .. } => format!("RunStarted({total})"),
            DesktopEvent::StepStarted { index, .. } => format!("StepStarted({index})"),
            DesktopEvent::StepCommitted { index, .. } => format!("StepCommitted({index})"),
            DesktopEvent::Output { text, .. } => format!("Output({text:?})"),
            DesktopEvent::SubProgress { .. } => "SubProgress".to_string(),
            DesktopEvent::KernelProgress {
                phase, fraction, ..
            } => format!("KernelProgress({phase} {:.0}%)", fraction * 100.0),
            DesktopEvent::Finished { .. } => "Finished".to_string(),
            DesktopEvent::Cancelled {
                committed_steps,
                in_flight,
                ..
            } => {
                format!("Cancelled({committed_steps}, {in_flight:?})")
            }
            DesktopEvent::Failed { diagnostic, .. } => format!("Failed({})", diagnostic.render()),
        }
    }

    fn summaries(events: &[SequencedEvent]) -> Vec<String> {
        events
            .iter()
            .map(|event| event_summary(&event.event))
            .collect()
    }

    /// Reads events until a terminal one arrives, failing with the collected names on timeout.
    fn drain_until_terminal(host: &ExecutionHost, timeout: Duration) -> Vec<SequencedEvent> {
        let deadline = Instant::now() + timeout;
        let mut events = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                panic!(
                    "timed out waiting for a terminal event; collected {events:?}",
                    events = summaries(&events)
                );
            }
            match host.next_event(remaining) {
                Some(event) => {
                    let terminal = matches!(
                        event.event,
                        DesktopEvent::Finished { .. }
                            | DesktopEvent::Cancelled { .. }
                            | DesktopEvent::Failed { .. }
                    );
                    events.push(event);
                    if terminal {
                        return events;
                    }
                }
                None => panic!(
                    "next_event returned None before a terminal event; collected {events:?}",
                    events = summaries(&events)
                ),
            }
        }
    }

    fn test_snapshot(result: &str, revision: SourceRevision) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            result: result.to_string(),
            result_structured: None,
            result_inspection: None,
            variables: vec![("x".to_string(), result.to_string())],
            functions: Vec::new(),
            state: EngineState {
                variables: vec![crate::engine::EngineVariable {
                    name: "x".to_string(),
                    kind: "Natural".to_string(),
                    display: result.to_string(),
                    structured: None,
                }],
                functions: Vec::new(),
            },
            plot: None,
            output: vec![result.to_string()],
            revision,
            committed_steps: 1,
            total: 1,
            precision: DEFAULT_PRECISION,
            diagnostic: None,
        }
    }

    fn sequenced(sequence: u64, revision: SourceRevision, event: DesktopEvent) -> SequencedEvent {
        SequencedEvent {
            sequence,
            revision,
            event,
        }
    }

    #[test]
    fn t2_submit_does_not_block_and_run_started_arrives_quickly() {
        let host = ExecutionHost::new(sleep_factory(Duration::from_millis(300)));
        let started = Instant::now();
        let handle = host
            .submit(
                DocumentId(1),
                SourceRevision(7),
                "a = 1\nb = 2\n".to_string(),
                1000,
            )
            .expect("submit must accept an idle document");
        let submit_elapsed = started.elapsed();
        assert!(
            submit_elapsed < Duration::from_millis(100),
            "submit took {submit_elapsed:?}, expected under 100 ms"
        );
        eprintln!("T2 submit returned in {submit_elapsed:?}");

        let first = host
            .next_event(Duration::from_millis(100))
            .unwrap_or_else(|| panic!("RunStarted was not available within 100 ms of submit"));
        eprintln!(
            "T2 RunStarted received {:?} after submit",
            started.elapsed()
        );
        match &first.event {
            DesktopEvent::RunStarted {
                document,
                run,
                revision,
                total,
            } => {
                assert_eq!(*document, DocumentId(1));
                assert_eq!(*run, handle.run());
                assert_eq!(*revision, SourceRevision(7));
                assert_eq!(*total, 2);
            }
            other => panic!(
                "first event was {}, expected RunStarted",
                event_summary(other)
            ),
        }

        assert!(
            host.cancel(&handle),
            "cancel must be accepted on an active run"
        );
        let events = drain_until_terminal(&host, Duration::from_secs(2));
        assert!(
            matches!(
                events.last().map(|event| &event.event),
                Some(DesktopEvent::Cancelled { .. })
            ),
            "terminal event was {:?}, expected Cancelled",
            summaries(&events)
        );
    }

    #[test]
    fn t3_events_are_sequenced_and_ordered_with_run_identity() {
        let host = ExecutionHost::new(counting_factory());
        let handle = host
            .submit(
                DocumentId(4),
                SourceRevision(9),
                "a = 1\nb = 2\nc = 3\n".to_string(),
                1000,
            )
            .expect("submit");
        let events = drain_until_terminal(&host, Duration::from_secs(5));
        assert_eq!(
            events.len(),
            11,
            "expected RunStarted + 3*(StepStarted, StepCommitted, SubProgress) + Finished; got {:?}",
            summaries(&events)
        );

        let mut ordered = Vec::new();
        for event in &events {
            assert_eq!(
                event.event.document(),
                DocumentId(4),
                "wrong document on {}",
                event_summary(&event.event)
            );
            assert_eq!(
                event.event.run(),
                handle.run(),
                "wrong run on {}",
                event_summary(&event.event)
            );
            assert_eq!(
                event.revision,
                SourceRevision(9),
                "wrong revision on {}",
                event_summary(&event.event)
            );
            if !matches!(event.event, DesktopEvent::SubProgress { .. }) {
                ordered.push(event_summary(&event.event));
            }
        }
        assert_eq!(
            ordered,
            vec![
                "RunStarted(3)",
                "StepStarted(0)",
                "StepCommitted(0)",
                "StepStarted(1)",
                "StepCommitted(1)",
                "StepStarted(2)",
                "StepCommitted(2)",
                "Finished",
            ]
        );
        for pair in events.windows(2) {
            assert!(
                pair[0].sequence < pair[1].sequence,
                "sequence did not increase: {} then {}",
                pair[0].sequence,
                pair[1].sequence
            );
        }

        let finished = match &events.last().unwrap().event {
            DesktopEvent::Finished { snapshot, .. } => snapshot,
            other => panic!("terminal was {}, expected Finished", event_summary(other)),
        };
        assert_eq!(finished.result, "v3");
        assert_eq!(finished.committed_steps, 3);
    }

    #[test]
    fn t4_cancellation_stops_a_real_run_and_keeps_the_committed_snapshot() {
        let host = ExecutionHost::new(progress_factory(Duration::from_millis(20)));
        let source = (0..200)
            .map(|index| format!("step = {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let handle = host
            .submit(DocumentId(2), SourceRevision(1), source, 1000)
            .expect("submit");
        let mut events = Vec::new();
        loop {
            match host.next_event(Duration::from_secs(5)) {
                Some(event) => {
                    let committed = matches!(event.event, DesktopEvent::StepCommitted { .. });
                    events.push(event);
                    if committed {
                        break;
                    }
                }
                None => panic!(
                    "no StepCommitted event within 5 s; collected {:?}",
                    summaries(&events)
                ),
            }
        }

        let cancel_sent = Instant::now();
        assert!(
            host.cancel(&handle),
            "cancel must be accepted while the run is active"
        );
        loop {
            match host.next_event(Duration::from_secs(2)) {
                Some(event) => {
                    let terminal = matches!(
                        event.event,
                        DesktopEvent::Finished { .. }
                            | DesktopEvent::Cancelled { .. }
                            | DesktopEvent::Failed { .. }
                    );
                    events.push(event);
                    if terminal {
                        break;
                    }
                }
                None => panic!(
                    "no terminal event within 2 s after cancel; collected {:?}",
                    summaries(&events)
                ),
            }
        }
        let elapsed = cancel_sent.elapsed();
        assert!(
            elapsed < Duration::from_secs(2),
            "cancellation took {elapsed:?}, expected under 2 s"
        );
        eprintln!("T4 cancel-to-terminal took {elapsed:?}");

        let terminal = events.last().unwrap();
        match &terminal.event {
            DesktopEvent::Cancelled {
                document,
                run,
                committed_steps,
                snapshot,
                ..
            } => {
                assert_eq!(*document, DocumentId(2));
                assert_eq!(*run, handle.run());
                assert!(
                    *committed_steps < 200,
                    "committed_steps was {committed_steps}, expected fewer than 200"
                );
                assert!(*committed_steps >= 1, "expected at least the first step");
                let published = host
                    .last_snapshot(DocumentId(2))
                    .expect("a cancelled run must publish its last snapshot");
                assert_eq!(
                    snapshot, &published,
                    "the Cancelled event must carry the snapshot the run published"
                );
                assert_eq!(snapshot.committed_steps, *committed_steps);
                assert_eq!(
                    snapshot.variables,
                    vec![("step".to_string(), committed_steps.to_string())],
                    "snapshot state must contain exactly the committed statements"
                );
                assert_eq!(snapshot.result, committed_steps.to_string());
                assert_eq!(snapshot.revision, SourceRevision(1));
            }
            other => panic!(
                "terminal after cancel was {}, expected Cancelled",
                event_summary(other)
            ),
        }
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.event, DesktopEvent::Finished { .. }))
                .count(),
            0,
            "a cancelled run must not also finish: {:?}",
            summaries(&events)
        );
    }

    /// RUN-008's kernel proof, with RUN-007's prefix: Cancel while the worker is inside ONE
    /// kernel call.
    ///
    /// `matmul(eye(400), eye(400))` is a single statement whose whole cost is one array kernel
    /// (a triple loop that polls the cancellation seam); there is no statement boundary to
    /// observe for many seconds. Cancel must reach the engine's live flag and the run must
    /// publish a terminal `Cancelled` inside the two-second budget. The measured latency is
    /// printed. The kernel stops early (`Interrupted`), and that early exit is the in-flight
    /// statement's committed result: the prefix is one statement, not zero. A single opaque
    /// primitive with no poll seam (`sqrt(2)` at precision 200000) is budgeted as
    /// uninterruptible and documented in `docs/goal-desktop-v4/round-9/kernel-cancel.md`.
    #[cfg(feature = "real-engine")]
    #[test]
    fn t9_cancel_inside_one_kernel_is_terminal_within_two_seconds() {
        let host = ExecutionHost::new(Box::new(|| {
            Box::new(crate::engine::RealEngine::new()) as Box<dyn Evaluator>
        }));
        let document = DocumentId(9);
        let handle = host
            .submit(
                document,
                SourceRevision(1),
                "matmul(eye(400), eye(400))".to_string(),
                1000,
            )
            .expect("submit");
        // Wait for the worker to enter the kernel statement (the only statement), then give the
        // kernel 200 ms so Cancel cannot win at a boundary the run does not reach anyway.
        let mut events = Vec::new();
        loop {
            let event = host.next_event(Duration::from_secs(5)).unwrap_or_else(|| {
                panic!(
                    "no StepStarted event for the kernel statement; collected {:?}",
                    summaries(&events)
                )
            });
            let entered_kernel = matches!(event.event, DesktopEvent::StepStarted { index: 0, .. });
            events.push(event);
            if entered_kernel {
                break;
            }
        }
        thread::sleep(Duration::from_millis(200));
        assert!(
            handle.is_active(),
            "the one long kernel finished before it could be cancelled"
        );

        let cancel_sent = Instant::now();
        assert!(
            host.cancel(&handle),
            "cancel must be accepted while the kernel is running"
        );
        let deadline = cancel_sent + Duration::from_secs(2);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                panic!(
                    "no terminal event within 2 s after cancel; collected {:?}",
                    summaries(&events)
                );
            }
            match host.next_event(remaining) {
                Some(event) => {
                    let terminal = matches!(
                        event.event,
                        DesktopEvent::Finished { .. }
                            | DesktopEvent::Cancelled { .. }
                            | DesktopEvent::Failed { .. }
                    );
                    events.push(event);
                    if terminal {
                        break;
                    }
                }
                None => panic!(
                    "no terminal event within 2 s after cancel; collected {:?}",
                    summaries(&events)
                ),
            }
        }
        let elapsed = cancel_sent.elapsed();
        eprintln!(
            "T9 single-kernel cancel-to-terminal took {:.3} ms",
            elapsed.as_secs_f64() * 1000.0
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "single-kernel cancellation took {elapsed:?}, expected under 2 s"
        );
        let terminal = events.last().expect("a terminal event was appended");
        match &terminal.event {
            DesktopEvent::Cancelled {
                document: stopped,
                run,
                committed_steps,
                in_flight,
                snapshot: event_snapshot,
            } => {
                assert_eq!(*stopped, document);
                assert_eq!(*run, handle.run());
                // COR-016: the statement that was in flight when Cancel was pressed is discarded,
                // not committed. Only the previously committed prefix is published.
                assert_eq!(
                    *committed_steps, 0,
                    "an interrupted in-flight statement must not be committed"
                );
                assert_eq!(
                    *in_flight,
                    InFlightCancellation::Interrupted,
                    "the kernel observed the live flag and stopped early"
                );
                let published = host
                    .last_snapshot(document)
                    .expect("a cancelled run must publish its last snapshot");
                assert_eq!(
                    event_snapshot, &published,
                    "the Cancelled event must carry the snapshot the run published"
                );
                assert_eq!(event_snapshot.committed_steps, 0);
                assert_eq!(event_snapshot.result, "Void");
                assert_eq!(event_snapshot.revision, SourceRevision(1));
            }
            other => panic!(
                "terminal after cancel was {}, expected Cancelled; collected {:?}",
                event_summary(other),
                summaries(&events)
            ),
        }
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.event, DesktopEvent::Finished { .. }))
                .count(),
            0,
            "a cancelled run must not also finish: {:?}",
            summaries(&events)
        );
    }

    #[test]
    fn t5_duplicate_submit_is_busy_and_other_documents_run_concurrently() {
        let host = ExecutionHost::new(sleep_factory(Duration::from_millis(300)));
        let doc1 = DocumentId(1);
        let doc2 = DocumentId(2);
        let handle1 = host
            .submit(
                doc1,
                SourceRevision(3),
                "a = 1\nb = 2\nc = 3\nd = 4\ne = 5\n".to_string(),
                1000,
            )
            .expect("first submit for document 1");
        match host.submit(doc1, SourceRevision(4), "x = 1".to_string(), 1000) {
            Err(SubmitError::Busy) => {}
            Err(other) => {
                panic!("second submit for document 1 failed with {other:?}, expected Busy")
            }
            Ok(_) => panic!("second submit for document 1 succeeded, expected Busy"),
        }

        let handle2 = host
            .submit(doc2, SourceRevision(3), "y = 1\n".to_string(), 1000)
            .expect("document 2 must stay runnable while document 1 runs");
        assert!(
            handle1.is_active(),
            "document 1 must still be active when document 2 starts"
        );

        let finished2_at = loop {
            match host.next_event(Duration::from_secs(5)) {
                Some(event) => match &event.event {
                    DesktopEvent::Finished { document, .. } if *document == doc2 => {
                        break Instant::now();
                    }
                    DesktopEvent::Finished { document, .. } => {
                        panic!("document {document:?} finished before document 2")
                    }
                    _ => {}
                },
                None => panic!("document 2 did not finish within 5 s"),
            }
        };
        assert!(
            handle1.is_active(),
            "document 1 must still be running when document 2 finishes"
        );
        let cancel_at = Instant::now();
        assert!(
            finished2_at < cancel_at,
            "document 2's Finished ({finished2_at:?}) must precede document 1's cancellation ({cancel_at:?})"
        );
        assert!(
            host.cancel(&handle1),
            "document 1 must be cancellable after document 2 finished"
        );

        let events = drain_until_terminal(&host, Duration::from_secs(2));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.event, DesktopEvent::Finished { .. }))
                .count(),
            0,
            "document 1 must not have finished before cancellation: {:?}",
            summaries(&events)
        );
        match &events.last().unwrap().event {
            DesktopEvent::Cancelled {
                document,
                run,
                committed_steps,
                ..
            } => {
                assert_eq!(*document, doc1);
                assert_eq!(*run, handle1.run());
                assert!(
                    *committed_steps < 5,
                    "committed_steps was {committed_steps}"
                );
            }
            other => panic!(
                "terminal for document 1 was {}, expected Cancelled",
                event_summary(other)
            ),
        }
        assert_eq!(handle2.document(), doc2);
    }

    #[test]
    fn t6_panicking_engine_fails_one_run_and_the_host_stays_usable() {
        let panicked = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&panicked);
        let host = ExecutionHost::new(Box::new(move || {
            Box::new(PanicOnceEngine {
                panicked: Arc::clone(&flag),
            })
        }));
        let doc = DocumentId(9);
        let _first = host
            .submit(doc, SourceRevision(1), "a = 1\nb = 2\n".to_string(), 1000)
            .expect("first submit");
        let events = drain_until_terminal(&host, Duration::from_secs(5));
        let diagnostic = match &events.last().unwrap().event {
            DesktopEvent::Failed { diagnostic, .. } => diagnostic.clone(),
            other => panic!(
                "first run terminal was {}, expected Failed",
                event_summary(other)
            ),
        };
        assert!(
            diagnostic.message.contains("boom"),
            "diagnostic must carry the panic message: {diagnostic:?}"
        );
        assert!(
            diagnostic.render().contains("line 1"),
            "diagnostic must point into the full source: {diagnostic:?}"
        );
        let after_panic = host
            .last_snapshot(doc)
            .expect("a failed run still records its last committed snapshot");
        assert_eq!(after_panic.committed_steps, 0);

        let _second = host
            .submit(doc, SourceRevision(1), "a = 1\nb = 2\n".to_string(), 1000)
            .expect("the host must accept a new run on the same document after a panic");
        let events = drain_until_terminal(&host, Duration::from_secs(5));
        match &events.last().unwrap().event {
            DesktopEvent::Finished { snapshot, .. } => {
                assert_eq!(snapshot.result, "ok");
                assert_eq!(snapshot.committed_steps, 2);
            }
            other => panic!(
                "second run terminal was {}, expected Finished",
                event_summary(other)
            ),
        }
    }

    #[test]
    fn t7_stale_finished_events_are_ignored_by_the_sink() {
        let document = DocumentId(1);
        let run = RunId(1);
        let mut sink = EventSink::new();
        assert_eq!(sink.revision(), SourceRevision(0));

        let applied = sink.apply(&sequenced(
            1,
            SourceRevision(2),
            DesktopEvent::Finished {
                document,
                run,
                snapshot: test_snapshot("new", SourceRevision(2)),
            },
        ));
        assert!(applied, "the first Finished must be applied");
        assert_eq!(sink.revision(), SourceRevision(2));
        let state_before = sink.snapshot().cloned();
        assert_eq!(
            state_before
                .as_ref()
                .map(|snapshot| snapshot.result.as_str()),
            Some("new")
        );

        let stale = sink.apply(&sequenced(
            2,
            SourceRevision(1),
            DesktopEvent::Finished {
                document,
                run,
                snapshot: test_snapshot("stale", SourceRevision(1)),
            },
        ));
        assert!(!stale, "a Finished from an older revision must be ignored");
        assert_eq!(
            sink.snapshot(),
            state_before.as_ref(),
            "stale Finished must not change the sink state"
        );
        assert_eq!(
            sink.revision(),
            SourceRevision(2),
            "stale Finished must not advance the sink revision"
        );

        let newer = sink.apply(&sequenced(
            3,
            SourceRevision(3),
            DesktopEvent::Finished {
                document,
                run,
                snapshot: test_snapshot("newest", SourceRevision(3)),
            },
        ));
        assert!(newer, "a newer Finished must be applied");
        assert_eq!(
            sink.snapshot().map(|snapshot| snapshot.result.as_str()),
            Some("newest")
        );
        assert_eq!(sink.revision(), SourceRevision(3));
    }

    #[test]
    fn t8_output_events_are_exactly_the_printed_lines_and_do_not_accumulate_across_runs() {
        let host = ExecutionHost::new(Box::new(|| Box::new(PrintingEngine::default())));
        let doc = DocumentId(5);
        for run_number in 0..2 {
            let _handle = host
                .submit(doc, SourceRevision(1), "print twice".to_string(), 1000)
                .expect("submit");
            let events = drain_until_terminal(&host, Duration::from_secs(5));
            let outputs = events
                .iter()
                .filter_map(|event| match &event.event {
                    DesktopEvent::Output { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                outputs,
                vec!["first", "second"],
                "run {run_number} output events"
            );

            let finished = events
                .iter()
                .find_map(|event| match &event.event {
                    DesktopEvent::Finished { snapshot, .. } => Some(snapshot.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("run {run_number} had no Finished event"));
            assert_eq!(
                finished.output,
                vec!["first".to_string(), "second".to_string()],
                "run {run_number} snapshot output"
            );
        }
    }

    /// INC-001: the run's final state is settled *before* its terminal event is observable.
    ///
    /// The document's incremental cache is part of that state. A client may legitimately do
    /// either of the two things this test does the moment it sees `Finished` — read the cache, or
    /// submit the next run — so the worker must have handed the cache back already. When it had
    /// not (the hand-back ran after the terminal emit), the read below saw the removed slot as an
    /// empty cache and the next run started with nothing to reuse, silently re-evaluating every
    /// statement: the full release suite reproduced exactly that as `left: 0, right: 4` in
    /// `incremental::engine_tests::reuse_counts_are_exactly_the_cache_hits`.
    ///
    /// The loop is the falsifier, not decoration: the bug was a lost race, so one iteration would
    /// only expose it when the worker happened to lose the CPU between the emit and the hand-back.
    /// After the fix the assertion is ordered after the hand-back by the cache mutex, so it can
    /// never observe the slot missing, however the threads are scheduled.
    #[test]
    fn the_documents_cache_is_settled_before_the_terminal_event_is_observable() {
        let host = ExecutionHost::new(cacheable_factory());
        let document = DocumentId(1);
        for run in 1..=64u64 {
            let _handle = host
                .submit(document, SourceRevision(run), "x = 1".to_string(), 1000)
                .expect("an idle document accepts the run");
            let events = drain_until_terminal(&host, Duration::from_secs(30));
            assert!(
                matches!(
                    events.last().map(|event| &event.event),
                    Some(DesktopEvent::Finished { .. })
                ),
                "run {run}: terminal event was {:?}",
                summaries(&events)
            );
            assert_eq!(
                host.cached_entries(document),
                1,
                "run {run}: the document's cache is back in the host table by the time its \
                 terminal event is observable"
            );
            // The second way a client reads the settled state: the run submitted straight after
            // the previous terminal event must be served from the cache that run filled. Only the
            // first run evaluates, so a lost hand-back shows up here as zero reused commits.
            let reused = events
                .iter()
                .filter(|event| {
                    matches!(
                        &event.event,
                        DesktopEvent::StepCommitted { reused: true, .. }
                    )
                })
                .count();
            assert_eq!(
                reused,
                usize::from(run > 1),
                "run {run}: a run submitted on the previous terminal event is served from the \
                 cache"
            );
        }
    }

    /// LOG-001 byte clause: print() text containing CRLF, LF and a bare CR must reach the typed
    /// Output events byte-for-byte, in order, with no normalisation or separator added by the
    /// desktop.
    ///
    /// The three control bytes are embedded literally in the script's string literals, so the
    /// engine stores them verbatim and the slicer (which only splits outside strings) preserves
    /// them. The expected bytes come from a second FakeEngine evaluating the same script: the
    /// assertion is event bytes == engine bytes, not a hand-written rendering of them.
    #[test]
    fn t10_output_events_preserve_crlf_lf_and_bare_cr_bytes_in_order() {
        let source = "print(\"a\r\nb\")\nprint(\"c\nd\")\nprint(\"e\rf\")\n";
        let mut reference = crate::engine::FakeEngine::new();
        reference.eval(source).expect("the byte script evaluates");
        let expected = reference.take_output();
        assert_eq!(
            expected
                .iter()
                .map(|line| line.as_bytes())
                .collect::<Vec<_>>(),
            vec![b"a\r\nb".as_slice(), b"c\nd".as_slice(), b"e\rf".as_slice()],
            "the engine itself must hold the control bytes verbatim"
        );

        let host = ExecutionHost::new(Box::new(|| {
            Box::new(crate::engine::FakeEngine::new()) as Box<dyn Evaluator>
        }));
        let _handle = host
            .submit(DocumentId(12), SourceRevision(1), source.to_string(), 1000)
            .expect("submit");
        let events = drain_until_terminal(&host, Duration::from_secs(5));
        assert!(
            matches!(
                events.last().map(|event| &event.event),
                Some(DesktopEvent::Finished { .. })
            ),
            "terminal event was {:?}, expected Finished",
            summaries(&events)
        );

        let actual = events
            .iter()
            .filter_map(|event| match &event.event {
                DesktopEvent::Output { text, .. } => Some(text.as_bytes().to_vec()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual.len(),
            expected.len(),
            "one Output event per printed line"
        );
        for (index, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                actual.as_slice(),
                expected.as_bytes(),
                "Output event {index} must be the engine's line byte-for-byte"
            );
        }

        let actual_bytes = actual.iter().flatten().copied().collect::<Vec<u8>>();
        let expected_bytes = expected
            .iter()
            .flat_map(|line| line.bytes())
            .collect::<Vec<u8>>();
        assert_eq!(
            actual_bytes, expected_bytes,
            "the concatenated Output bytes must equal the engine's output bytes in order"
        );
        assert_eq!(
            actual_bytes,
            b"a\r\nbc\nde\rf".to_vec(),
            "CRLF, LF and the bare CR must survive unchanged and no separator may be added"
        );
    }

    #[test]
    fn diagnostic_position_is_translated_back_to_the_full_source() {
        let source = "first = 1\nsecond = 2\nthird = 3\nfourth = 4\n";
        let statements = crate::statements::slice(source);
        assert_eq!(statements.len(), 4, "{statements:?}");
        let statement = &statements[2];
        assert_eq!(statement.start_line(), 3);

        let diagnostic =
            Diagnostic::for_statement(statement, source, "Unexpected token 'x' at position 6.");
        assert_eq!(diagnostic.statement_index, 2);
        assert_eq!(diagnostic.statement_line, 3);
        assert_eq!(diagnostic.byte_offset, statement.byte_offset() + 6);
        assert_eq!(diagnostic.line, 3);
        assert_eq!(diagnostic.column, 7);
        assert_eq!(diagnostic.message, "Unexpected token 'x' at position 6.");
        assert_eq!(
            diagnostic.render(),
            "line 3, column 7: Unexpected token 'x' at position 6."
        );

        let fallback = Diagnostic::for_statement(statement, source, "Cannot divide by zero.");
        assert_eq!(fallback.byte_offset, statement.byte_offset());
        assert_eq!(fallback.line, 3);
        assert_eq!(fallback.column, 1);
    }

    #[test]
    fn diagnostic_position_is_a_utf16_offset_not_a_byte_offset() {
        // "s = \"<emoji>\" + x": the emoji is four UTF-8 bytes but two UTF-16 code units, so the
        // engine's position 11 for 'x' is byte 13 and column 12 (1-based code units).
        let source = "s = \"\u{1f600}\" + x\n";
        let statements = crate::statements::slice(source);
        assert_eq!(statements.len(), 1);
        assert_eq!(statements[0].text(), "s = \"\u{1f600}\" + x");

        let diagnostic = Diagnostic::for_statement(
            &statements[0],
            source,
            "Unexpected token 'x' at position 11.",
        );
        assert_eq!(diagnostic.byte_offset, 13);
        assert_eq!(diagnostic.line, 1);
        assert_eq!(diagnostic.column, 12);
        assert_eq!(byte_offset_utf16("s = \"\u{1f600}\" + x", 11), 13);
        assert_eq!(byte_offset_utf16("\u{1f600}x", 99), 5, "clamped to the end");
        assert_eq!(
            byte_offset_utf16("\u{1f600}x", 1),
            0,
            "inside a surrogate pair clamps to the character start"
        );
    }

    #[test]
    fn real_engine_executes_sliced_statements_and_snapshots_variables() {
        let names = Arc::new(Mutex::new(Vec::new()));
        let engine_factory = crate::engine::default_engine_factory();
        let recorded = Arc::clone(&names);
        let factory: EvaluatorFactory = Box::new(move || {
            let engine = engine_factory();
            recorded
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(engine.name());
            engine
        });
        let host = ExecutionHost::new(factory);
        let _handle = host
            .submit(
                DocumentId(11),
                SourceRevision(1),
                "a = 2\nb = a + 1\nb\n".to_string(),
                1000,
            )
            .expect("submit");
        let events = drain_until_terminal(&host, Duration::from_secs(30));
        let snapshot = match &events.last().unwrap().event {
            DesktopEvent::Finished { snapshot, .. } => snapshot,
            other => panic!(
                "real-engine run terminal was {}, expected Finished",
                event_summary(other)
            ),
        };
        assert_eq!(snapshot.result, "3");
        assert_eq!(snapshot.committed_steps, 3);
        // The real engine's variables() is contractually (name, typed display): CONTRACT.md
        // "Real API" says "name, typed display (sorted)". It also records the display of a
        // top-level expression under "_" (the reference behavior asserted by the engine's own
        // suite_parity_batch test evaluate_given_result_stores_in_underscore_variable), so the
        // script above leaves exactly a, b and _. These assertions therefore check the typed
        // contract and the underscore entry explicitly instead of the fake engine's plain form.
        #[cfg(feature = "real-engine")]
        {
            assert_eq!(snapshot.variables.len(), 3, "{:?}", snapshot.variables);
            assert!(
                snapshot
                    .variables
                    .contains(&("a".to_string(), "2 (Natural)".to_string())),
                "variables were {:?}",
                snapshot.variables
            );
            assert!(
                snapshot
                    .variables
                    .contains(&("b".to_string(), "3 (Natural)".to_string())),
                "variables were {:?}",
                snapshot.variables
            );
            assert!(
                snapshot
                    .variables
                    .contains(&("_".to_string(), "3 (Natural)".to_string())),
                "variables were {:?}",
                snapshot.variables
            );
        }
        #[cfg(not(feature = "real-engine"))]
        {
            assert_eq!(snapshot.variables.len(), 2, "{:?}", snapshot.variables);
            assert!(
                snapshot
                    .variables
                    .contains(&("a".to_string(), "2".to_string())),
                "variables were {:?}",
                snapshot.variables
            );
            assert!(
                snapshot
                    .variables
                    .contains(&("b".to_string(), "3".to_string())),
                "variables were {:?}",
                snapshot.variables
            );
        }
        #[cfg(feature = "real-engine")]
        assert_eq!(
            *names.lock().unwrap_or_else(|error| error.into_inner()),
            vec!["lovelace-lang"],
            "the default build must use the real engine"
        );
    }

    /// RUN-005 proof: a long kernel publishes sub-progress while the statement is in flight.
    ///
    /// The web Studio reference publishes kernel sub-progress (`Interpreter.SubProgress("sqrt")`
    /// passed into `Real.SqrtAsync`). The engine now does too, through the additive
    /// `lovelace_lang::progress` observer the adapter arms for each evaluation, so this test
    /// asserts the events the probe used to find absent: at least one `KernelProgress` between
    /// `StepStarted` and `StepCommitted`, each carrying the engine's phase and a fraction that
    /// never moves backwards. The committed `1/1` statement fraction still arrives as its own,
    /// separate `SubProgress` event — the two are distinguishable on the wire and in the strip.
    #[test]
    #[cfg(feature = "real-engine")]
    fn t10_a_long_sqrt_kernel_publishes_kernel_sub_progress() {
        let host = ExecutionHost::new(Box::new(|| Box::new(crate::engine::RealEngine::new())));
        let document = DocumentId(7);
        let started = Instant::now();
        let _handle = host
            .submit(document, SourceRevision(1), "sqrt(2)\n".to_string(), 50_000)
            .expect("the long kernel must be submitted");
        let mut events = Vec::new();
        loop {
            let Some(event) = host.next_event(Duration::from_secs(120)) else {
                panic!("the long kernel delivered no terminal event");
            };
            let terminal = matches!(
                event.event,
                DesktopEvent::Finished { .. }
                    | DesktopEvent::Cancelled { .. }
                    | DesktopEvent::Failed { .. }
            );
            events.push(event);
            if terminal {
                break;
            }
        }
        let elapsed = started.elapsed();
        let summary = |events: &[SequencedEvent]| {
            events
                .iter()
                .map(|event| event_summary(&event.event))
                .collect::<Vec<_>>()
        };
        assert!(
            matches!(
                events.last().map(|event| &event.event),
                Some(DesktopEvent::Finished { .. })
            ),
            "the probe expects a completed kernel: {:?}",
            summary(&events)
        );
        let step_started = events
            .iter()
            .position(|event| matches!(event.event, DesktopEvent::StepStarted { .. }))
            .expect("one StepStarted");
        let step_committed = events
            .iter()
            .position(|event| matches!(event.event, DesktopEvent::StepCommitted { .. }))
            .expect("one StepCommitted");
        assert!(
            step_committed > step_started,
            "the commit must follow the start: {:?}",
            summary(&events)
        );
        let between = &events[step_started + 1..step_committed];
        let kernel_progress: Vec<(String, f32)> = between
            .iter()
            .filter_map(|event| match &event.event {
                DesktopEvent::KernelProgress {
                    phase, fraction, ..
                } => Some((phase.clone(), *fraction)),
                _ => None,
            })
            .collect();
        assert!(
            !kernel_progress.is_empty(),
            "the engine published no kernel sub-progress during the kernel: {:?}",
            summary(between)
        );
        let phases: Vec<&str> = kernel_progress
            .iter()
            .map(|(phase, _)| phase.as_str())
            .collect();
        assert_eq!(
            phases,
            vec!["sqrt:operand", "sqrt:integer-square-root", "sqrt:truncate"],
            "the sqrt kernel's phases, in order: {kernel_progress:?}"
        );
        for window in kernel_progress.windows(2) {
            assert!(
                window[1].1 >= window[0].1,
                "the fraction must never move backwards: {kernel_progress:?}"
            );
        }
        assert!(
            kernel_progress.first().map(|(_, fraction)| *fraction)
                < kernel_progress.last().map(|(_, fraction)| *fraction),
            "at least one report must advance the fraction: {kernel_progress:?}"
        );
        // The statement-level SubProgress is still exactly the committed fraction, distinct from
        // the kernel's phases: one event, label "1/1", after the commit.
        let sub_progress: Vec<(f32, String)> = events
            .iter()
            .filter_map(|event| match &event.event {
                DesktopEvent::SubProgress {
                    fraction, label, ..
                } => Some((*fraction, label.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(
            sub_progress,
            vec![(1.0, "1/1".to_string())],
            "the statement fraction must stay the committed fraction, not the kernel's"
        );
        let finished = events
            .iter()
            .find_map(|event| match &event.event {
                DesktopEvent::Finished { snapshot, .. } => Some(snapshot),
                _ => None,
            })
            .expect("the probe expects a completed kernel");
        assert_eq!(
            finished.precision, 50_000,
            "the kernel must run at the precision that makes it long, not a shorter one"
        );
        let committed_digits = finished
            .result
            .chars()
            .filter(|character| character.is_ascii_digit())
            .count();
        assert!(
            committed_digits >= 50_000,
            "the committed result must carry the 50 000 digits the kernel was asked for, not a \
             short value; it carried {committed_digits}"
        );
        println!(
            "RUN-005 proof: sqrt(2) at 50000 digits took {:.0} ms; the kernel published {} \
             progress event(s) between StepStarted and StepCommitted: {:?}; the statement \
             SubProgress is still {:?}",
            elapsed.as_secs_f64() * 1000.0,
            kernel_progress.len(),
            kernel_progress
                .iter()
                .map(|(phase, fraction)| format!("{phase} {:.0}%", fraction * 100.0))
                .collect::<Vec<String>>(),
            sub_progress[0].1
        );
    }

    /// A snapshot with all fields explicit, for the ARC-004 variant-enumeration test.
    fn sample_snapshot() -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            result: "42".to_string(),
            result_structured: None,
            result_inspection: None,
            variables: vec![("x".to_string(), "42".to_string())],
            functions: Vec::new(),
            state: EngineState::default(),
            plot: None,
            output: Vec::new(),
            revision: SourceRevision(9),
            committed_steps: 1,
            total: 1,
            precision: DEFAULT_PRECISION,
            diagnostic: None,
        }
    }

    /// A minimal typed diagnostic for the `Failed` variant.
    fn sample_diagnostic() -> Diagnostic {
        Diagnostic {
            message: "boom".to_string(),
            code: Some("E_TEST".to_string()),
            category: Some("TestError".to_string()),
            suggestions: Vec::new(),
            statement_index: 0,
            statement_line: 1,
            byte_offset: 0,
            line: 1,
            column: 1,
        }
    }

    /// ARC-004: every variant is constructed, named, given ids, and matched without a wildcard.
    ///
    /// The `match` below has no `_` arm on purpose. Adding a variant to `DesktopEvent` fails this
    /// test at compile time until the variant is named in [`DesktopEvent::kind`], listed in
    /// [`DesktopEvent::KIND_NAMES`] and given a document/run id and a consumer. There is no
    /// `#[non_exhaustive]` escape hatch on the enum.
    #[test]
    fn every_desktop_event_variant_is_constructed_named_and_id_bound() {
        let document = DocumentId(11);
        let run = RunId(22);
        let snapshot = sample_snapshot();
        let events = vec![
            DesktopEvent::RunStarted {
                document,
                run,
                revision: SourceRevision(9),
                total: 3,
            },
            DesktopEvent::StepStarted {
                document,
                run,
                index: 0,
                label: "a = 1".to_string(),
            },
            DesktopEvent::StepCommitted {
                document,
                run,
                index: 0,
                reused: true,
                snapshot: snapshot.clone(),
            },
            DesktopEvent::Output {
                document,
                run,
                text: "hello".to_string(),
            },
            DesktopEvent::SubProgress {
                document,
                run,
                fraction: 1.0 / 3.0,
                label: "1/3".to_string(),
            },
            DesktopEvent::KernelProgress {
                document,
                run,
                phase: "sqrt:integer-square-root".to_string(),
                fraction: 0.25,
            },
            DesktopEvent::Finished {
                document,
                run,
                snapshot: snapshot.clone(),
            },
            DesktopEvent::Cancelled {
                document,
                run,
                committed_steps: 1,
                in_flight: InFlightCancellation::None,
                snapshot: snapshot.clone(),
            },
            DesktopEvent::Failed {
                document,
                run,
                diagnostic: sample_diagnostic(),
                snapshot,
            },
        ];
        let observed: Vec<&'static str> = events.iter().map(DesktopEvent::kind).collect();
        println!(
            "ARC-004 variants ({}): {}",
            observed.len(),
            observed.join(", ")
        );
        assert_eq!(observed, DesktopEvent::KIND_NAMES);
        let mut unique = observed.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(
            unique.len(),
            DesktopEvent::KIND_NAMES.len(),
            "kind names must be distinct: {observed:?}"
        );
        for event in &events {
            assert_eq!(
                event.document(),
                document,
                "event {event:?} lost its document id"
            );
            assert_eq!(event.run(), run, "event {event:?} lost its run id");
            // Exhaustive, wildcard-free match: the compile-time tripwire for a new variant.
            match event {
                DesktopEvent::RunStarted {
                    document: d,
                    run: r,
                    ..
                }
                | DesktopEvent::StepStarted {
                    document: d,
                    run: r,
                    ..
                }
                | DesktopEvent::StepCommitted {
                    document: d,
                    run: r,
                    ..
                }
                | DesktopEvent::Output {
                    document: d,
                    run: r,
                    ..
                }
                | DesktopEvent::SubProgress {
                    document: d,
                    run: r,
                    ..
                }
                | DesktopEvent::KernelProgress {
                    document: d,
                    run: r,
                    ..
                }
                | DesktopEvent::Finished {
                    document: d,
                    run: r,
                    ..
                }
                | DesktopEvent::Cancelled {
                    document: d,
                    run: r,
                    ..
                }
                | DesktopEvent::Failed {
                    document: d,
                    run: r,
                    ..
                } => {
                    assert_eq!((*d, *r), (document, run));
                }
            }
        }
    }

    /// ARC-004: the event enum's source shape is typed — no `serde_json::Value`, no stringly
    /// `kind`, no `#[non_exhaustive]` hole.
    #[test]
    fn the_event_enum_has_no_json_or_stringly_kind_and_is_not_non_exhaustive() {
        let source = include_str!("execution.rs");
        let start = source
            .find("pub enum DesktopEvent {")
            .expect("DesktopEvent declaration");
        let end = source[start..]
            .find("impl DesktopEvent {")
            .map(|offset| start + offset)
            .expect("impl DesktopEvent follows the enum");
        let declaration = &source[start..end];
        assert!(
            !declaration.contains("#[non_exhaustive]"),
            "DesktopEvent must stay exhaustively matched"
        );
        assert!(
            !declaration.contains("serde_json") && !declaration.contains("Value"),
            "the UI boundary event must not carry an untyped JSON value: {declaration}"
        );
        assert!(
            !declaration.contains("kind: String") && !declaration.contains("kind: &'static str"),
            "the event must not carry a stringly-typed kind field: {declaration}"
        );
        for name in DesktopEvent::KIND_NAMES {
            assert!(
                declaration.contains(name),
                "variant {name} is missing from the enum declaration"
            );
        }
        println!(
            "ARC-004 enum shape: variants={} declaration_bytes={} non_exhaustive=false serde_json_value=false stringly_kind=false typed_ids_and_payloads=true",
            DesktopEvent::KIND_NAMES.len(),
            declaration.len()
        );
    }

    /// ARC-004: one run's stream is sequenced without gaps, bound to one run and one revision.
    #[test]
    fn the_event_stream_is_gap_free_and_bound_to_the_submitted_run_and_revision() {
        let host = ExecutionHost::new(Box::new(|| Box::new(crate::engine::FakeEngine::new())));
        let document = DocumentId(3);
        let revision = SourceRevision(7);
        let handle = host
            .submit(
                document,
                revision,
                "a = 1\nb = a + 1\nb\n".to_string(),
                DEFAULT_PRECISION,
            )
            .expect("submit");
        let run = handle.run();
        let mut events = Vec::new();
        loop {
            let event = host
                .next_event(Duration::from_secs(10))
                .expect("the run delivered no terminal event");
            let terminal = matches!(
                event.event,
                DesktopEvent::Finished { .. }
                    | DesktopEvent::Cancelled { .. }
                    | DesktopEvent::Failed { .. }
            );
            events.push(event);
            if terminal {
                break;
            }
        }
        assert_eq!(
            events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            (1..=events.len() as u64).collect::<Vec<_>>(),
            "sequence numbers must be gap-free and monotonic"
        );
        for event in &events {
            assert_eq!(
                event.revision, revision,
                "event {event:?} lost its revision"
            );
            assert_eq!(event.event.run(), run, "event {event:?} names another run");
            assert_eq!(
                event.event.document(),
                document,
                "event {event:?} names another document"
            );
        }
        assert!(matches!(
            events.first().map(|e| &e.event),
            Some(DesktopEvent::RunStarted { .. })
        ));
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
        assert!(matches!(
            events.last().map(|e| &e.event),
            Some(DesktopEvent::Finished { .. })
        ));
        println!(
            "ARC-004 stream: document={} run={} revision={} events={} sequence={}..{} kinds={:?}",
            document.0,
            run.0,
            revision.0,
            events.len(),
            events.first().map(|event| event.sequence).unwrap_or(0),
            events.last().map(|event| event.sequence).unwrap_or(0),
            events
                .iter()
                .map(|event| event.event.kind())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            events.len(),
            1 + 3 * 3 + 1,
            "one started, per statement start+commit+progress, one finished"
        );
    }

    /// ARC-005: the engine records its calling thread; it is the named worker, never the caller.
    struct ThreadRecordingEngine {
        threads: Arc<Mutex<Vec<(std::thread::ThreadId, Option<String>)>>>,
    }

    impl Evaluator for ThreadRecordingEngine {
        fn eval(&mut self, _src: &str) -> Result<String, String> {
            self.threads.lock().unwrap().push((
                std::thread::current().id(),
                std::thread::current().name().map(str::to_string),
            ));
            Ok("ok".to_string())
        }
    }

    #[test]
    fn evaluation_records_the_worker_thread_not_the_caller_ui_thread() {
        let threads = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&threads);
        let host = ExecutionHost::new(Box::new(move || {
            Box::new(ThreadRecordingEngine {
                threads: Arc::clone(&recorded),
            })
        }));
        let ui_thread = std::thread::current().id();
        let handle = host
            .submit(
                DocumentId(5),
                SourceRevision(1),
                "1 + 1".to_string(),
                DEFAULT_PRECISION,
            )
            .expect("submit");
        assert_ne!(
            handle.worker_thread(),
            ui_thread,
            "the worker was spawned on the UI thread"
        );
        loop {
            let event = host
                .next_event(Duration::from_secs(5))
                .expect("no terminal event");
            if matches!(event.event, DesktopEvent::Finished { .. }) {
                break;
            }
        }
        let observed = threads.lock().unwrap().clone();
        assert_eq!(
            observed.len(),
            1,
            "one statement must have been evaluated once: {observed:?}"
        );
        assert_eq!(
            observed[0].0,
            handle.worker_thread(),
            "the engine evaluated on a thread other than the worker the handle names"
        );
        assert_ne!(
            observed[0].0, ui_thread,
            "the engine evaluated on the UI thread"
        );
        let name = observed[0].1.as_deref().unwrap_or_default();
        println!(
            "ARC-005 caller_ui_thread={:?} engine_thread={:?} engine_thread_name={:?} worker_handle_thread={:?}",
            ui_thread,
            observed[0].0,
            observed[0].1,
            handle.worker_thread()
        );
        assert!(
            name.starts_with("lovelace-run-"),
            "the worker thread was not named: {name:?}"
        );
    }

    /// ARC-008: a cancellable engine that records when its run really started and when it is
    /// dropped. The worker thread is the only place either side effect can happen.
    struct CancelProbeEngine {
        started: Arc<AtomicBool>,
        dropped: Arc<AtomicBool>,
        cancel: Arc<AtomicBool>,
    }

    impl Evaluator for CancelProbeEngine {
        fn eval(&mut self, _src: &str) -> Result<String, String> {
            self.started.store(true, Ordering::SeqCst);
            while !self.cancel.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err("cancelled by the shutdown probe".to_string())
        }

        fn set_cancel_flag(&mut self, flag: Option<Arc<AtomicBool>>) {
            if let Some(flag) = flag {
                self.cancel = flag;
            }
        }
    }

    impl Drop for CancelProbeEngine {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn shutdown_cancels_and_joins_a_live_worker_and_joins_a_finished_one() {
        let started = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicBool::new(false));
        let cancel = Arc::new(AtomicBool::new(false));
        let host = ExecutionHost::new(Box::new({
            let started = Arc::clone(&started);
            let dropped = Arc::clone(&dropped);
            let cancel = Arc::clone(&cancel);
            move || {
                Box::new(CancelProbeEngine {
                    started: Arc::clone(&started),
                    dropped: Arc::clone(&dropped),
                    cancel: Arc::clone(&cancel),
                })
            }
        }));
        let _handle = host
            .submit(
                DocumentId(8),
                SourceRevision(1),
                "long_kernel()".to_string(),
                DEFAULT_PRECISION,
            )
            .expect("submit");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !started.load(Ordering::SeqCst) {
            assert!(Instant::now() < deadline, "the worker never entered eval");
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(host.live_worker_count(), 1);
        let joined = host.shutdown();
        println!(
            "ARC-008 shutdown: joined={joined} dropped={} live_after={}",
            dropped.load(Ordering::SeqCst),
            host.live_worker_count()
        );
        assert_eq!(joined, 1, "shutdown must join the live worker");
        assert!(
            dropped.load(Ordering::SeqCst),
            "the worker was not joined to completion"
        );
        assert_eq!(host.live_worker_count(), 0);
        assert_eq!(host.shutdown(), 0, "shutdown must be idempotent");

        // A run that finished before shutdown is joined too, not leaked.
        let host = ExecutionHost::new(Box::new(|| Box::new(crate::engine::FakeEngine::new())));
        let _handle = host
            .submit(
                DocumentId(9),
                SourceRevision(1),
                "1 + 1".to_string(),
                DEFAULT_PRECISION,
            )
            .expect("submit");
        loop {
            let event = host
                .next_event(Duration::from_secs(5))
                .expect("no terminal event");
            if matches!(
                event.event,
                DesktopEvent::Finished { .. }
                    | DesktopEvent::Cancelled { .. }
                    | DesktopEvent::Failed { .. }
            ) {
                break;
            }
        }
        assert!(
            host.shutdown() <= 1,
            "a completed worker must either have been reaped or be joined now"
        );
        assert_eq!(host.live_worker_count(), 0);
    }
}
