//! Round-68: the platform IME probe behind EDT-017 and A11Y-008.
//!
//! `lovelace-desktop --ime-probe <mode> <document> <report>` opens the **shipped workbench
//! window** and drives the Windows input-method manager from inside that window's own process,
//! on the thread that owns its HWND:
//!
//! 1. `ImmGetContext(hwnd)` obtains the window's HIMC (the `ImmAssociateContextEx` fallback is
//!    applied if it is null).
//! 2. `ImmSetCompositionStringW(himc, SCS_SETSTR, ...)` puts the composed text into that input
//!    context, and `ImmGetCompositionStringW` reads it back.
//! 3. `WM_IME_STARTCOMPOSITION`, `WM_IME_COMPOSITION` (once with `GCS_COMPSTR` only, once with
//!    `GCS_RESULTSTR`) and `WM_IME_ENDCOMPOSITION` are sent to the window, the same messages an
//!    IME posts. winit's window procedure reads the composition out of the HIMC for those.
//! 4. `ImmReleaseContext` closes the context.
//!
//! Why in-process: an HIMC is per-process state. A second process calling `ImmGetContext` on this
//! window gets null (recorded in round-68's report), so a genuine platform test has to run where
//! the input context lives. The probe is only reachable through `--ime-probe`; the default launch
//! path never constructs it.
//!
//! Modes:
//! - `editor-cafe` / `editor-cjk`: the shipped path. The workbench editor requests IME itself and
//!   this mode requires the full result: pre-edit displayed and not inserted, exactly one commit,
//!   one undo step restoring the pre-composition bytes, and Ctrl+Enter deferred while composing.
//! - `control-cafe`: control. The quick-eval field is an `egui::TextEdit`; while it has focus it
//!   requests IME itself, so this mode proves the probe's messages reach a real widget in this
//!   window without the editor's request.
//!
//! The earlier `wired-*` diagnostic (the harness supplying the editor's missing request) is gone:
//! the request is product code now (`TextEditor::request_ime`), so a harness copy would mask the
//! shipped path this probe exists to measure.
//!
//! The report is one line per observation, appended and flushed as the sequence advances, so the
//! external capture driver can act on `AWAIT_UIA` and read the result. `FINAL` marks the end; the
//! window then stays open so the driver can capture it (`<report>.close` ends it).

use std::io::Write as _;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::ui::{native_options, theme, WorkbenchApp};
use crate::win32::ffi;

/// How long the window stays open after the sequence when the driver writes no close marker.
const HOLD: Duration = Duration::from_secs(180);
/// How long the probe waits for the driver's `AWAIT_UIA` marker before continuing without it.
const UIA_WAIT: Duration = Duration::from_secs(40);
/// Passes per phase that must elapse before the next observation, so the event queue drains.
const DRAIN_PASSES: usize = 3;

/// U+0063 U+0061 U+0066 U+00E9: `café` with the precomposed acute accent.
const CAFE: &str = "caf\u{e9}";
/// U+65E5 U+672C U+8A9E: `日本語`.
const CJK: &str = "\u{65e5}\u{672c}\u{8a9e}";

/// What one run is testing.
#[derive(Clone, Copy)]
enum Mode {
    /// The shipped editor requests IME and handles the composition itself.
    Editor(&'static str),
    /// Control: an `egui::TextEdit` in this window requests IME itself.
    Control(&'static str),
}

impl Mode {
    fn parse(name: &str) -> Option<Mode> {
        match name {
            "editor-cafe" => Some(Mode::Editor(CAFE)),
            "editor-cjk" => Some(Mode::Editor(CJK)),
            "control-cafe" => Some(Mode::Control(CAFE)),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Mode::Editor(text) => {
                if text == CAFE {
                    "editor-cafe"
                } else {
                    "editor-cjk"
                }
            }
            Mode::Control(_) => "control-cafe",
        }
    }

    fn composed(self) -> &'static str {
        match self {
            Mode::Editor(text) | Mode::Control(text) => text,
        }
    }

    /// The editor buffer a commit must produce: the probe puts the caret at the start, so the
    /// composed text is inserted there.
    fn expected_after_commit(self, before: &str) -> String {
        let mut after = String::with_capacity(before.len() + self.composed().len());
        after.push_str(self.composed());
        after.push_str(before);
        after
    }
}

/// One probe run's state machine.
struct ProbeApp {
    inner: WorkbenchApp,
    mode: Mode,
    report: PathBuf,
    marker: PathBuf,
    undo_marker: PathBuf,
    close: PathBuf,
    started: Instant,
    hwnd: ffi::HWND,
    himc: ffi::HIMC,
    /// `(name, pass)` for every assertion the probe makes.
    checks: Vec<(String, bool)>,
    /// The `PlatformOutput::ime` the shipped window requested on this pass, read after
    /// `WorkbenchApp::ui` produced it. This is the product's own request, not a harness copy.
    last_ime: Option<egui::output::IMEOutput>,
    /// The pre-edit that was on screen when the chord was posted, so the log can say what the
    /// keyboard was withheld for.
    preedit_at_shortcut: Option<String>,
    phase: usize,
    passes_in_phase: usize,
    baseline: String,
    baseline_undo: usize,
    uia_deadline: Option<Instant>,
    undo_deadline: Option<Instant>,
    final_at: Option<Instant>,
    written: bool,
}

impl ProbeApp {
    fn new(inner: WorkbenchApp, mode: Mode, report: PathBuf, hwnd: ffi::HWND) -> ProbeApp {
        let marker = report.with_extension("go");
        let undo_marker = report.with_extension("undo");
        let close = report.with_extension("close");
        let _ = std::fs::remove_file(&marker);
        let _ = std::fs::remove_file(&undo_marker);
        let _ = std::fs::remove_file(&close);
        let _ = std::fs::remove_file(&report);
        let mut app = ProbeApp {
            inner,
            mode,
            report,
            marker,
            undo_marker,
            close,
            started: Instant::now(),
            hwnd,
            himc: 0,
            checks: Vec::new(),
            last_ime: None,
            preedit_at_shortcut: None,
            phase: 0,
            passes_in_phase: 0,
            baseline: String::new(),
            baseline_undo: 0,
            uia_deadline: None,
            undo_deadline: None,
            final_at: None,
            written: false,
        };
        app.log(&format!(
            "window hwnd=0x{:X} default-ime-window=0x{:X} mode={} composed={:?}",
            app.hwnd,
            ffi::ime_default_window(app.hwnd),
            app.mode.name(),
            app.mode.composed()
        ));
        app.check("window-found", app.hwnd != 0, &format!("0x{:X}", app.hwnd));
        app
    }

    /// Appends one line to the report (and to stderr, so a redirected launch shows it).
    fn log(&mut self, line: &str) {
        let text = format!("{:>7}ms {line}", self.started.elapsed().as_millis());
        eprintln!("ime-probe {text}");
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.report)
        {
            let _ = writeln!(file, "{text}");
        }
    }

    fn check(&mut self, name: &str, ok: bool, observed: &str) {
        self.checks.push((name.to_string(), ok));
        self.log(&format!(
            "CHECK {} {name} observed={observed}",
            if ok { "PASS" } else { "FAIL" }
        ));
    }

    /// The one-line state of the window, read from the app itself (not from a document).
    fn snapshot(&self, ctx: &egui::Context) -> String {
        let (source, preedit, undo, focus, caret) = self.inner.ime_probe_snapshot(ctx);
        format!(
            "editor={source:?} preedit={preedit:?} undo_steps={undo} caret={caret:?} focus={focus} quick_eval={:?} run={} {}",
            self.inner.ime_probe_quick_eval(),
            self.inner.ime_probe_run_state(),
            self.ime_line(),
        )
    }

    /// The IME request the product produced this pass, with its two geometry rectangles.
    fn ime_line(&self) -> String {
        match self.last_ime {
            Some(ime) => format!(
                "ime=Some(purpose={:?} rect={:?} cursor_rect={:?})",
                ime.purpose, ime.rect, ime.cursor_rect
            ),
            None => "ime=None".to_string(),
        }
    }

    fn editor_text(&self, ctx: &egui::Context) -> String {
        self.inner.ime_probe_snapshot(ctx).0
    }

    fn acquire_himc(&mut self) {
        let hwnd = self.hwnd;
        let himc = ffi::ime_context(hwnd);
        self.log(&format!("ImmGetContext hwnd=0x{hwnd:X} himc=0x{himc:X}"));
        if himc == 0 {
            let associated = ffi::ime_associate_default(hwnd);
            let retry = ffi::ime_context(hwnd);
            self.log(&format!(
                "ImmGetContext returned null; ImmAssociateContextEx(IACE_DEFAULT)={associated} himc_after=0x{retry:X}"
            ));
            self.himc = retry;
        } else {
            self.himc = himc;
        }
        self.log(&format!(
            "ImmGetDefaultIMEWnd=0x{:X}",
            ffi::ime_default_window(hwnd)
        ));
        self.check("himc-obtained", self.himc != 0, &format!("0x{:X}", self.himc));
    }

    /// Writes the composition string into the HIMC and reads both strings back, so the report
    /// proves the platform input context really holds the composed text.
    fn set_composition(&mut self, ctx: &egui::Context) {
        let text = self.mode.composed();
        let himc = self.himc;
        let set = ffi::ime_set_composition_string(himc, text);
        let composition = ffi::ime_composition_string(himc, false);
        let result = ffi::ime_composition_string(himc, true);
        self.log(&format!(
            "ImmSetCompositionStringW(SCS_SETSTR,{text:?})={set} GCS_COMPSTR={composition:?} GCS_RESULTSTR={result:?} utf16_bytes={}",
            text.encode_utf16().count() * 2
        ));
        self.check("composition-string-set", set, &format!("set={set}"));
        let (_, _, _, _, caret) = self.inner.ime_probe_snapshot(ctx);
        self.check(
            "caret-at-buffer-start",
            caret == (1, 1),
            &format!("{caret:?}"),
        );
    }

    /// Re-reads the window's HIMC after the UIA focus round trip.
    ///
    /// UIA `SetFocus` on the Run button moves egui's focus off the editor, so the editor stops
    /// requesting IME for a few passes and `egui-winit` toggles `set_ime_allowed(false)`; winit
    /// implements that with `ImmAssociateContextEx(..., IACE_CHILDREN)`, which detaches the
    /// context this probe was holding. The live context is re-read (and the documented fallback
    /// applied) before the commit case, so the second composition addresses the session's current
    /// input context instead of a released one.
    fn reacquire_himc(&mut self) {
        if self.himc != 0 {
            let released = ffi::ime_release(self.hwnd, self.himc);
            self.log(&format!(
                "ImmReleaseContext himc=0x{:X} released={released} before re-reading the context",
                self.himc
            ));
            self.himc = 0;
        }
        let hwnd = self.hwnd;
        let mut himc = ffi::ime_context(hwnd);
        if himc == 0 {
            let associated = ffi::ime_associate_default(hwnd);
            himc = ffi::ime_context(hwnd);
            self.log(&format!(
                "ImmGetContext after the UIA focus round trip returned null; ImmAssociateContextEx(IACE_DEFAULT)={associated} himc_after=0x{himc:X}"
            ));
        } else {
            self.log(&format!(
                "ImmGetContext after the UIA focus round trip himc=0x{himc:X}"
            ));
        }
        self.himc = himc;
        self.check(
            "himc-after-uia-focus",
            self.himc != 0,
            &format!("0x{:X}", self.himc),
        );
    }

    fn send_ime(&mut self, message: u32, lparam: usize, label: &str) {
        let hwnd = self.hwnd;
        let result = ffi::send(hwnd, message, 0, lparam as isize);
        self.log(&format!(
            "SendMessage hwnd=0x{hwnd:X} msg=0x{message:04X} lparam=0x{lparam:X} result={result} ({label})"
        ));
    }

    /// Sends Ctrl+`virtual_key` with the Control modifier visible to winit's `GetKeyState` read.
    fn send_control_key(&mut self, virtual_key: u16, scan: u16, extended: bool, label: &str) {
        let hwnd = self.hwnd;
        let state = ffi::set_control_key_state(true);
        self.log(&format!("SetKeyboardState(VK_CONTROL down)={state} ({label})"));
        ffi::send_key(hwnd, ffi::VK_CONTROL, 0x1D, false, true);
        ffi::send_key(hwnd, virtual_key, scan, extended, true);
        ffi::send_key(hwnd, virtual_key, scan, extended, false);
        ffi::send_key(hwnd, ffi::VK_CONTROL, 0x1D, false, false);
        let restored = ffi::set_control_key_state(false);
        self.log(&format!(
            "SendMessage Ctrl+{virtual_key:#04X} down/up ({label}) control-state-restored={restored}"
        ));
    }

    fn write_final(&mut self) {
        if self.written {
            return;
        }
        self.written = true;
        let failures = self.checks.iter().filter(|(_, ok)| !ok).count();
        let summary = self
            .checks
            .iter()
            .map(|(name, ok)| format!("{name}={}", if *ok { "PASS" } else { "FAIL" }))
            .collect::<Vec<_>>()
            .join(",");
        self.log(&format!(
            "RESULT mode={} checks={} failures={failures} {summary}",
            self.mode.name(),
            self.checks.len()
        ));
        self.log("FINAL");
    }

    fn step(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.passes_in_phase += 1;
        if self.final_at.is_some() {
            if self.close.exists() || self.started.elapsed() > HOLD {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }
        if self.phase == 0 {
            // Give the widget under test the focus and let a pass render it.
            match self.mode {
                Mode::Editor(_) => self.inner.ime_probe_focus_editor(&ctx),
                Mode::Control(_) => self.inner.ime_probe_focus_quick_eval(&ctx),
            }
            if self.passes_in_phase >= DRAIN_PASSES {
                let (source, _, undo, focus, _) = self.inner.ime_probe_snapshot(&ctx);
                self.baseline = source;
                self.baseline_undo = undo;
                let snapshot = self.snapshot(&ctx);
                self.log(&format!("PHASE baseline {snapshot}"));
                self.check(
                    "baseline-focus",
                    match self.mode {
                        Mode::Editor(_) => focus == "editor",
                        Mode::Control(_) => focus == "quick-eval",
                    },
                    &focus,
                );
                // EDT-017: the shipped widget under test must ask the integration for IME.
                // Without this request egui-winit never calls `set_ime_allowed(true)` and winit
                // drops every WM_IME_* message before the widget can see it.
                self.check(
                    "ime-requested-by-widget",
                    self.last_ime.is_some(),
                    &self.ime_line(),
                );
                // The editor's own request must carry the caret rect inside its text rect. The
                // control is egui's `TextEdit`, whose caret rect deliberately overhangs the field
                // by a pixel at each edge; for it only the two positive rectangles are asserted.
                let geometry_ok = self.last_ime.is_some_and(|ime| match self.mode {
                    Mode::Editor(_) => {
                        ime.rect.is_positive()
                            && ime.rect.contains_rect(ime.cursor_rect)
                            && ime.cursor_rect.height() > 1.0
                    }
                    Mode::Control(_) => ime.rect.is_positive() && ime.cursor_rect.is_positive(),
                });
                self.check(
                    "ime-caret-rect-in-text-rect",
                    geometry_ok,
                    &self.ime_line(),
                );
                self.acquire_himc();
                self.set_composition(&ctx);
                self.phase = 1;
                self.passes_in_phase = 0;
            }
        } else if self.phase == 1 {
            self.send_ime(ffi::WM_IME_STARTCOMPOSITION, 0, "start composition");
            // imm32 exposes the composition string through the IME as the composition starts, so
            // this read is recorded but not asserted; the readback is asserted on the next phase.
            let composition = ffi::ime_composition_string(self.himc, false);
            let result = ffi::ime_composition_string(self.himc, true);
            self.log(&format!(
                "HIMC after start: GCS_COMPSTR={composition:?} GCS_RESULTSTR={result:?}"
            ));
            self.send_ime(
                ffi::WM_IME_COMPOSITION,
                ffi::GCS_COMPSTR as usize,
                "composition with GCS_COMPSTR",
            );
            self.log(&format!("PHASE preedit-sent {}", self.snapshot(&ctx)));
            self.phase = 2;
            self.passes_in_phase = 0;
        } else if self.phase == 2 {
            if self.passes_in_phase >= DRAIN_PASSES {
                let snapshot = self.snapshot(&ctx);
                self.log(&format!("PHASE preedit-active {snapshot}"));
                let composition = ffi::ime_composition_string(self.himc, false);
                let result = ffi::ime_composition_string(self.himc, true);
                self.log(&format!(
                    "HIMC during composition: GCS_COMPSTR={composition:?} GCS_RESULTSTR={result:?}"
                ));
                self.check(
                    "composition-string-in-himc",
                    composition == self.mode.composed(),
                    &format!("compstr={composition:?}"),
                );
                match self.mode {
                    Mode::Editor(text) => {
                        // The shipped editor receives the platform composition now: it displays
                        // the pre-edit at the caret and does not splice it into the buffer.
                        let (_, preedit, _, _, _) = self.inner.ime_probe_snapshot(&ctx);
                        self.check(
                            "preedit-displayed-not-inserted",
                            preedit.as_deref() == Some(text)
                                && self.editor_text(&ctx) == self.baseline,
                            &format!("preedit={preedit:?} editor={:?}", self.editor_text(&ctx)),
                        );
                    }
                    Mode::Control(_) => {
                        // egui's TextEdit paints the composition inside the field while it is
                        // being built, so this is the display half of case 3.
                        self.check(
                            "control-preedit-displayed",
                            self.inner.ime_probe_quick_eval() == self.mode.composed(),
                            &format!("{:?}", self.inner.ime_probe_quick_eval()),
                        );
                    }
                }
                // A11Y-008: the composition is active right now. Send the window's own Ctrl+Enter
                // accelerator and observe that it does not fire. `LOVELACE_IME_NO_SHORTCUT=1` is
                // the attribution control: the same run without the chord.
                let before_run = self.inner.ime_probe_run_state();
                self.preedit_at_shortcut = self.inner.ime_probe_snapshot(&ctx).1;
                if std::env::var("LOVELACE_IME_NO_SHORTCUT").is_ok_and(|value| value == "1") {
                    self.log("shortcut-during-composition skipped (LOVELACE_IME_NO_SHORTCUT=1)");
                } else {
                    self.send_control_key(ffi::VK_RETURN, 0x1C, false, "Ctrl+Enter during composition");
                }
                self.log(&format!(
                    "shortcut-during-composition sent run_state_at_send={before_run:?} preedit_at_send={:?}",
                    self.preedit_at_shortcut
                ));
                self.log("AWAIT_UIA");
                self.uia_deadline = Some(Instant::now() + UIA_WAIT);
                self.phase = 3;
                self.passes_in_phase = 0;
            }
        } else if self.phase == 3 {
            let ready = self.marker.exists();
            let timed_out = self
                .uia_deadline
                .is_some_and(|deadline| Instant::now() >= deadline);
            if ready || timed_out {
                if ready {
                    let _ = std::fs::remove_file(&self.marker);
                }
                let snapshot = self.snapshot(&ctx);
                let (_, preedit, _, _, _) = self.inner.ime_probe_snapshot(&ctx);
                let run = self.inner.ime_probe_run_state();
                let text = self.editor_text(&ctx);
                self.log(&format!(
                    "PHASE uia-done marker={ready} timeout={timed_out} {snapshot}"
                ));
                self.check("uia-call-observed", ready, &format!("{ready}"));
                // A11Y-008: the chord was posted while the composition owned the keyboard. The
                // run must still be Idle and the buffer unchanged. The platform may end the
                // composition when the Ctrl modifier arrives (a real IME consumes that key), so
                // the observed pre-edit is recorded rather than required to survive.
                let (shortcut_ok, shortcut_observed) = if std::env::var("LOVELACE_IME_NO_SHORTCUT")
                    .is_ok_and(|value| value == "1")
                {
                    (true, "skipped".to_string())
                } else {
                    (
                        run == "Idle" && text == self.baseline,
                        format!(
                            "run_state={run:?} preedit={preedit:?} editor={text:?}"
                        ),
                    )
                };
                self.check(
                    "shortcut-deferred-during-composition",
                    shortcut_ok,
                    &shortcut_observed,
                );
                // Case 3: end the composition with no result string. Nothing may be committed.
                self.send_ime(
                    ffi::WM_IME_ENDCOMPOSITION,
                    0,
                    "end composition without GCS_RESULTSTR",
                );
                self.phase = 4;
                self.passes_in_phase = 0;
            }
        } else if self.phase == 4 {
            if self.passes_in_phase >= DRAIN_PASSES {
                let snapshot = self.snapshot(&ctx);
                self.log(&format!("PHASE after-end-without-result {snapshot}"));
                let text = self.editor_text(&ctx);
                self.check(
                    "composition-only-never-committed",
                    text == self.baseline,
                    &format!("editor={text:?}"),
                );
                // A11Y-008: the deferred chord is not replayed when the composition ends.
                let run = self.inner.ime_probe_run_state();
                self.check(
                    "shortcut-not-replayed-after-composition",
                    run == "Idle",
                    &format!("run_state={run:?}"),
                );
                if matches!(self.mode, Mode::Control(_)) {
                    // Ending a composition without a result must leave the TextEdit empty again:
                    // the composition is display-only and was never committed.
                    self.check(
                        "control-composition-without-result-cleared",
                        self.inner.ime_probe_quick_eval().is_empty(),
                        &format!("{:?}", self.inner.ime_probe_quick_eval()),
                    );
                }
                // Re-arm for the commit case: the UIA focus round trip may have detached the
                // input context, so read the live one first; then put the composed string back
                // into the HIMC and start a second composition, exactly as an IME does before
                // the user confirms it.
                self.reacquire_himc();
                let text = self.mode.composed();
                let reset = ffi::ime_set_composition_string(self.himc, text);
                self.log(&format!(
                    "ImmSetCompositionStringW(SCS_SETSTR,{text:?}) again={reset}"
                ));
                self.send_ime(ffi::WM_IME_STARTCOMPOSITION, 0, "start composition again");
                let composition = ffi::ime_composition_string(self.himc, false);
                let result = ffi::ime_composition_string(self.himc, true);
                self.log(&format!(
                    "HIMC after second start: GCS_COMPSTR={composition:?} GCS_RESULTSTR={result:?}"
                ));
                self.send_ime(
                    ffi::WM_IME_COMPOSITION,
                    ffi::GCS_COMPSTR as usize,
                    "composition with GCS_COMPSTR",
                );
                self.phase = 5;
                self.passes_in_phase = 0;
            }
        } else if self.phase == 5 {
            if self.passes_in_phase >= DRAIN_PASSES {
                self.log(&format!("PHASE preedit-again {}", self.snapshot(&ctx)));
                // The IME finalizes the composition when the user accepts it: CPS_COMPLETE moves
                // the composition string into the result string that GCS_RESULTSTR reports.
                let notify = ffi::ime_complete_composition(self.himc);
                let composition = ffi::ime_composition_string(self.himc, false);
                let result = ffi::ime_composition_string(self.himc, true);
                self.log(&format!(
                    "ImmNotifyIME(NI_COMPOSITIONSTR,CPS_COMPLETE)={notify} GCS_COMPSTR={composition:?} GCS_RESULTSTR={result:?}"
                ));
                self.check(
                    "composition-finalized",
                    result == self.mode.composed(),
                    &format!("notify={notify} result={result:?}"),
                );
                self.send_ime(
                    ffi::WM_IME_COMPOSITION,
                    ffi::GCS_RESULTSTR as usize,
                    "composition with GCS_RESULTSTR",
                );
                self.send_ime(ffi::WM_IME_ENDCOMPOSITION, 0, "end composition");
                self.phase = 6;
                self.passes_in_phase = 0;
            }
        } else if self.phase == 6 {
            if self.passes_in_phase >= DRAIN_PASSES {
                let snapshot = self.snapshot(&ctx);
                self.log(&format!("PHASE after-commit {snapshot}"));
                let text = self.editor_text(&ctx);
                match self.mode {
                    Mode::Editor(_) => {
                        let expected = self.mode.expected_after_commit(&self.baseline);
                        let commits = text.matches(self.mode.composed()).count();
                        self.check(
                            "editor-text-after-commit",
                            text == expected,
                            &format!("editor={text:?} expected={expected:?} occurrences={commits}"),
                        );
                        self.check(
                            "commit-not-duplicated",
                            commits == 1,
                            &format!("occurrences={commits}"),
                        );
                    }
                    Mode::Control(_) => {
                        let field = self.inner.ime_probe_quick_eval();
                        self.check(
                            "control-textedit-commit",
                            field == self.mode.composed(),
                            &format!("quick_eval={field:?}"),
                        );
                    }
                }
                // The probe's own post-commit observation is recorded before any undo, and the
                // driver gets one more chance to capture the committed buffer from UIA. The
                // control has no Ctrl+Z of ours (egui's TextEdit owns its own), but the same
                // capture window is offered so both screenshots come from the same pass.
                self.log("AWAIT_UNDO");
                self.undo_deadline = Some(Instant::now() + UIA_WAIT);
                self.phase = 7;
                self.passes_in_phase = 0;
            }
        } else if self.phase == 7 {
            // Case 4's capture window: the commit is in the buffer and has not been undone yet.
            let ready = self.undo_marker.exists();
            let timed_out = self
                .undo_deadline
                .is_some_and(|deadline| Instant::now() >= deadline);
            if ready || timed_out {
                if ready {
                    let _ = std::fs::remove_file(&self.undo_marker);
                }
                self.log(&format!(
                    "PHASE undo-requested marker={ready} timeout={timed_out} {}",
                    self.snapshot(&ctx)
                ));
                if matches!(self.mode, Mode::Editor(_)) {
                    self.send_control_key(ffi::VK_Z, 0x2C, false, "Ctrl+Z after the commit");
                } else {
                    self.log("Ctrl+Z skipped in control mode (egui TextEdit owns undo there)");
                }
                self.phase = 8;
                self.passes_in_phase = 0;
            }
        } else if self.phase == 8 {
            if self.passes_in_phase >= DRAIN_PASSES {
                let snapshot = self.snapshot(&ctx);
                self.log(&format!("PHASE after-undo {snapshot}"));
                let (source, _, undo, _, _) = self.inner.ime_probe_snapshot(&ctx);
                self.check(
                    "undo-restores-precomposition-bytes",
                    source == self.baseline,
                    &format!("editor={source:?} baseline={:?}", self.baseline),
                );
                self.check(
                    "undo-depth-restored",
                    undo == self.baseline_undo,
                    &format!("undo_steps={undo} baseline_undo={}", self.baseline_undo),
                );
                if self.himc != 0 {
                    let released = ffi::ime_release(self.hwnd, self.himc);
                    self.log(&format!(
                        "ImmReleaseContext hwnd=0x{:X} himc=0x{:X} ={released}",
                        self.hwnd, self.himc
                    ));
                    self.himc = 0;
                }
                self.write_final();
                self.final_at = Some(Instant::now());
            }
        }
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

impl eframe::App for ProbeApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // The real workbench paints and handles events exactly as the shipped window does. It is
        // also the only code that asks egui for IME now: the probe records that request instead
        // of supplying one, so a regression in the product cannot be masked by the harness.
        self.inner.ui(ui, frame);
        self.last_ime = ui.ctx().output(|output| output.ime);
        self.step(ui);
    }
}

/// The workbench window's HWND, read from eframe's creation context.
///
/// eframe owns the winit window; `CreationContext` implements `HasWindowHandle`, which is the
/// supported way to reach it. Enumerating top-level windows here instead would re-enter winit's
/// window procedure while a frame is being built and deadlock on its window-state lock.
fn window_handle_of(creation: &eframe::CreationContext<'_>) -> ffi::HWND {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match creation.window_handle() {
        Ok(handle) => match handle.as_raw() {
            RawWindowHandle::Win32(win32) => win32.hwnd.get() as ffi::HWND,
            _ => 0,
        },
        Err(_) => 0,
    }
}

/// Runs one probe mode and returns the process exit code.
pub(crate) fn run(mode: &str, document: Option<PathBuf>, report: PathBuf) -> i32 {
    let Some(mode) = Mode::parse(mode) else {
        eprintln!(
            "usage: lovelace-desktop --ime-probe <editor-cafe|editor-cjk|control-cafe> <document> <report>"
        );
        return 2;
    };
    let options = native_options();
    eprintln!(
        "ime-probe: mode={} document={:?} report={}",
        mode.name(),
        document,
        report.display()
    );
    match eframe::run_native(
        "Lovelace Desktop",
        options,
        Box::new(move |creation| {
            theme::apply(&creation.egui_ctx, theme::ThemeId::Dark);
            let inner = WorkbenchApp::new(document.clone());
            // eframe owns the winit window; its creation context is the supported way to read
            // that window's HWND. Enumerating top-level windows instead would re-enter winit's
            // window procedure while this app is being constructed.
            let hwnd = window_handle_of(&creation);
            Ok(Box::new(ProbeApp::new(inner, mode, report.clone(), hwnd)))
        }),
    ) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("lovelace-desktop: could not open the IME probe window: {error}");
            1
        }
    }
}
