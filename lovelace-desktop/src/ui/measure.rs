//! EDT-016: the large-file budget measurement mode.
//!
//! `lovelace-desktop --measure-large <script> --measure-out <report>` opens the **real** native
//! window on the generated script through the shipped [`WorkbenchApp`] and measures three
//! things over that window:
//!
//! - **open time**: from the process entry point (`main`) to the first `App::ui` pass, which is
//!   the first frame the document is painted in;
//! - **typing latency**: 200 single-character edits. Each sample injects one `egui::Event::Text`
//!   into the frame's input at the start of the pass and measures the wall time to the end of
//!   that same pass, when the frame's widget list is complete. This is the UI-thread latency a
//!   keypress pays; GPU presentation and the compositor's vsync wait are deliberately outside it
//!   and are reported separately as the frame-interval distribution.
//! - **scroll frame delivery**: 240 consecutive frames of continuous scrolling of the same
//!   document. Each frame advances the editor's scroll offset by 240 px and the report records
//!   the start-to-start frame interval, so fps is measured frames per wall second exactly as
//!   delivered, not assumed from the 60 Hz request.
//!
//! The mode is a wrapper around the real app: it injects the same event values winit delivers
//! and otherwise calls the shipped `WorkbenchApp::ui`. It is not a second rendering path. The
//! report is written before the window closes; if the window cannot be created the process exits
//! non-zero and no numbers are claimed.

use std::path::PathBuf;
use std::time::Instant;

use eframe::egui;

use super::app::WorkbenchApp;

/// Frames with no injected input before measurement starts, so fonts/layout are warm.
const WARMUP_FRAMES: usize = 30;
/// Number of single-character typing samples.
const TYPING_SAMPLES: usize = 200;
/// Number of caret-movement samples (PERF-002's caret half).
const CARET_SAMPLES: usize = 200;
/// Frames measured while the document scrolls.
const SCROLL_FRAMES: usize = 240;
/// Pixels the view advances on every scroll frame: three rows at the editor's row height.
const SCROLL_STEP: f32 = 48.0;
/// PERF-002's "no input freeze" ceiling, in milliseconds.
const FREEZE_BUDGET_MS: f64 = 100.0;
/// PERF-003's scrolling floor, in frames per second.
const SCROLL_FPS_BUDGET: f64 = 55.0;

/// One row of the measurement report.
#[derive(Clone, Copy, Debug)]
struct Distribution {
    count: usize,
    p50: f64,
    p95: f64,
    max: f64,
    mean: f64,
}

impl Distribution {
    fn from_samples(samples: &[f64]) -> Distribution {
        let mut sorted: Vec<f64> = samples.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        if sorted.is_empty() {
            return Distribution {
                count: 0,
                p50: 0.0,
                p95: 0.0,
                max: 0.0,
                mean: 0.0,
            };
        }
        let pick = |fraction: f64| {
            let index = ((sorted.len() as f64 - 1.0) * fraction).round() as usize;
            sorted[index.min(sorted.len() - 1)]
        };
        Distribution {
            count: sorted.len(),
            p50: pick(0.50),
            p95: pick(0.95),
            max: sorted[sorted.len() - 1],
            mean: sorted.iter().sum::<f64>() / sorted.len() as f64,
        }
    }

    fn line(&self, label: &str) -> String {
        format!(
            "{label}: n={} p50={:.3}ms p95={:.3}ms max={:.3}ms mean={:.3}ms",
            self.count, self.p50, self.p95, self.max, self.mean
        )
    }
}

#[derive(Clone, Copy)]
enum Phase {
    Warmup { frames: usize },
    Typing { samples: usize, cursor: usize, attempts: usize },
    Caret { samples: usize, direction: f32, attempts: usize },
    Scrolling { frames: usize, direction: f32, last_start: Option<Instant> },
    Done,
}

struct MeasureApp {
    inner: WorkbenchApp,
    out: PathBuf,
    started: Instant,
    open_ms: Option<f64>,
    phase: Phase,
    typing: Vec<f64>,
    typing_frames: Vec<f64>,
    typing_editor: Vec<f64>,
    typing_rows: Vec<f64>,
    last_typing_start: Option<Instant>,
    caret: Vec<f64>,
    caret_frames: Vec<f64>,
    caret_editor: Vec<f64>,
    last_caret_start: Option<Instant>,
    scroll: Vec<f64>,
    scroll_editor: Vec<f64>,
    scroll_rows: Vec<f64>,
    scroll_syntax: Vec<f64>,
    scroll_offset_start: f32,
    bytes: usize,
    lines: usize,
    written: bool,
}

impl MeasureApp {
    fn new(script: PathBuf, out: PathBuf, started: Instant) -> MeasureApp {
        let inner = WorkbenchApp::new(Some(script));
        MeasureApp {
            inner,
            out,
            started,
            open_ms: None,
            phase: Phase::Warmup { frames: 0 },
            typing: Vec::new(),
            typing_frames: Vec::new(),
            typing_editor: Vec::new(),
            typing_rows: Vec::new(),
            last_typing_start: None,
            caret: Vec::new(),
            caret_frames: Vec::new(),
            caret_editor: Vec::new(),
            last_caret_start: None,
            scroll: Vec::new(),
            scroll_editor: Vec::new(),
            scroll_rows: Vec::new(),
            scroll_syntax: Vec::new(),
            scroll_offset_start: 0.0,
            bytes: 0,
            lines: 0,
            written: false,
        }
    }

    /// Records the first painted frame as the end of the open.
    fn mark_open(&mut self) {
        if self.open_ms.is_none() {
            self.open_ms = Some(self.started.elapsed().as_secs_f64() * 1000.0);
        }
    }

    fn inject_text(ui: &mut egui::Ui, text: &str) {
        let event = egui::Event::Text(text.to_string());
        ui.ctx().input_mut(|input| input.events.push(event));
    }

    /// Injects one arrow-key press, the same `Event::Key` winit delivers for a real key.
    fn inject_arrow(ui: &mut egui::Ui, left: bool) {
        let key = if left {
            egui::Key::ArrowLeft
        } else {
            egui::Key::ArrowRight
        };
        let event = egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        ui.ctx().input_mut(|input| input.events.push(event));
    }

    fn write_report(&mut self) {
        if self.written {
            return;
        }
        self.written = true;
        let typing = Distribution::from_samples(&self.typing);
        let typing_frames = Distribution::from_samples(&self.typing_frames);
        let typing_editor = Distribution::from_samples(&self.typing_editor);
        let typing_rows = Distribution::from_samples(&self.typing_rows);
        let caret = Distribution::from_samples(&self.caret);
        let caret_frames = Distribution::from_samples(&self.caret_frames);
        let caret_editor = Distribution::from_samples(&self.caret_editor);
        let scroll = Distribution::from_samples(&self.scroll);
        let scroll_editor = Distribution::from_samples(&self.scroll_editor);
        let scroll_rows = Distribution::from_samples(&self.scroll_rows);
        let scroll_syntax = Distribution::from_samples(&self.scroll_syntax);
        let fps = if self.scroll.len() > 1 {
            let elapsed: f64 = self.scroll.iter().sum::<f64>() / 1000.0;
            (self.scroll.len() as f64) / elapsed
        } else {
            0.0
        };
        // PERF-003's "55 fps p95": 95% of delivered frames must complete within the 1/55 s
        // frame budget, so the p95 frame interval is the statistic that decides it.
        let fps_p95 = if scroll.p95 > 0.0 {
            1000.0 / scroll.p95
        } else {
            0.0
        };
        // PERF-002's "no input freeze >100 ms": the worst input-bearing pass and the worst
        // start-to-start interval between those passes.
        let input_freeze_max_ms = typing
            .max
            .max(typing_frames.max)
            .max(caret.max)
            .max(caret_frames.max)
            .max(typing_editor.max)
            .max(caret_editor.max);
        let open = self.open_ms.unwrap_or_default();
        let mut report = String::new();
        report.push_str("LOVELACE EDT-016 large-file measurement\n");
        report.push_str("method=real-window eframe/egui; event injected at frame start; typing latency = start of the edit pass to the end of that pass (UI list complete); caret latency = injected ArrowLeft/ArrowRight to the end of that same pass, confirmed by the caret column changing; scroll = start-to-start frame intervals while the view scrolls\n");
        report.push_str(&format!("document_bytes={}\n", self.bytes));
        report.push_str(&format!("document_lines={}\n", self.lines));
        report.push_str(&format!("open_ms={open:.3}\n"));
        report.push_str(&typing.line("typing_latency"));
        report.push_str(&format!("\n{}", typing_editor.line("typing_editor_pass")));
        report.push_str(&format!("\n{}", typing_rows.line("typing_rows_pass")));
        report.push_str(&format!("\n{}", typing_frames.line("typing_frame_interval")));
        report.push_str(&format!("\n{}", caret.line("caret_latency")));
        report.push_str(&format!("\n{}", caret_editor.line("caret_editor_pass")));
        report.push_str(&format!("\n{}", caret_frames.line("caret_frame_interval")));
        let scroll_seconds = self.scroll.iter().sum::<f64>() / 1000.0;
        let scroll_delta = self.inner.measure_editor().scroll_y - self.scroll_offset_start;
        let scroll_count = self.scroll.len();
        report.push_str(&format!(
            "\nscroll_frames={scroll_count} scroll_seconds={scroll_seconds:.3} fps={fps:.2} fps_p95={fps_p95:.2} scroll_offset_delta={scroll_delta:.1}\n"
        ));
        report.push_str(&scroll.line("scroll_frame_interval"));
        report.push_str(&format!("\n{}", scroll_editor.line("scroll_editor_pass")));
        report.push_str(&format!("\n{}", scroll_rows.line("scroll_rows_pass")));
        report.push_str(&format!("\n{}", scroll_syntax.line("scroll_syntax_pass")));
        report.push_str(&format!(
            "\nscroll_offset_start={:.1} scroll_offset_end={:.1}\n",
            self.scroll_offset_start,
            self.inner.measure_editor().scroll_y
        ));
        report.push_str(&format!(
            "input_freeze_max_ms={input_freeze_max_ms:.3} budget_freeze_max_ms={FREEZE_BUDGET_MS:.0} freeze_pass={}\n",
            input_freeze_max_ms <= FREEZE_BUDGET_MS
        ));
        report.push_str(&format!(
            "budget_typing_p95_ms=16 typing_p95_pass={}\n",
            typing.p95 <= 16.0
        ));
        report.push_str(&format!(
            "budget_caret_p95_ms=16 caret_p95_pass={}\n",
            caret.count > 0 && caret.p95 <= 16.0
        ));
        report.push_str(&format!(
            "budget_scroll_fps_min={SCROLL_FPS_BUDGET:.0} scroll_fps_pass={} scroll_fps_p95_pass={}\n",
            fps >= SCROLL_FPS_BUDGET,
            fps_p95 >= SCROLL_FPS_BUDGET
        ));
        match std::fs::write(&self.out, report) {
            Ok(()) => eprintln!(
                "lovelace-desktop: EDT-016 measurement written to {}",
                self.out.display()
            ),
            Err(error) => eprintln!(
                "lovelace-desktop: could not write the EDT-016 report {}: {error}",
                self.out.display()
            ),
        }
    }
}

impl eframe::App for MeasureApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.mark_open();
        let ctx = ui.ctx().clone();
        // The measurement drives the editor surface directly, so it keeps keyboard focus there
        // for every measured pass. Without this the injected text can be dropped by egui's focus
        // bookkeeping if the desktop window loses OS focus between frames (a locked session),
        // and the report would describe a different input path.
        ctx.memory_mut(|memory| memory.request_focus(super::editor::surface_id()));
        // Measurements need every frame, including frames with no OS input.
        ctx.request_repaint();
        let mut phase = self.phase;
        match &mut phase {
            Phase::Warmup { frames } => {
                ctx.memory_mut(|memory| memory.request_focus(super::editor::surface_id()));
                *frames += 1;
                if *frames >= WARMUP_FRAMES {
                    let editor = self.inner.measure_editor();
                    self.bytes = editor.bytes();
                    self.lines = editor.line_count();
                    editor.set_caret(0, 0);
                    editor.scroll_y = 0.0;
                    phase = Phase::Typing {
                        samples: 0,
                        cursor: 0,
                        attempts: 0,
                    };
                }
            }
            Phase::Typing {
                samples,
                cursor,
                attempts,
            } => {
                if *samples < TYPING_SAMPLES {
                    let digits = b"0123456789";
                    let text = char::from(digits[*cursor % digits.len()]).to_string();
                    let before = self.inner.measure_editor().edits;
                    let started = Instant::now();
                    if let Some(previous) = self.last_typing_start {
                        self.typing_frames.push(
                            started
                                .saturating_duration_since(previous)
                                .as_secs_f64()
                                * 1000.0,
                        );
                    }
                    self.last_typing_start = Some(started);
                    Self::inject_text(ui, &text);
                    <WorkbenchApp as eframe::App>::ui(&mut self.inner, ui, frame);
                    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
                    let after = self.inner.measure_editor().edits;
                    let (ui_ms, rows_ms) = {
                        let editor = self.inner.measure_editor();
                        (editor.last_ui_ms, editor.metrics.rows_ms)
                    };
                    self.typing_editor.push(ui_ms);
                    self.typing_rows.push(rows_ms);
                    if after > before {
                        self.typing.push(elapsed);
                        *samples += 1;
                        *cursor += 1;
                    } else {
                        *attempts += 1;
                        if *attempts > 120 {
                            // The editor never received the injected text (no focus), so the
                            // report states n=0 instead of inventing samples.
                            let editor = self.inner.measure_editor();
                            self.bytes = editor.bytes();
                            self.lines = editor.line_count();
                            phase = Phase::Done;
                        }
                    }
                    self.phase = phase;
                    return;
                }
                self.scroll_offset_start = self.inner.measure_editor().scroll_y;
                // Start the caret run in the middle of a long line so both directions have room.
                self.inner.measure_editor().set_caret(0, 10);
                phase = Phase::Caret {
                    samples: 0,
                    direction: 1.0,
                    attempts: 0,
                };
            }
            Phase::Caret {
                samples,
                direction,
                attempts,
            } => {
                if *samples < CARET_SAMPLES {
                    let before = {
                        let editor = self.inner.measure_editor();
                        (editor.caret_row, editor.caret_col)
                    };
                    let started = Instant::now();
                    if let Some(previous) = self.last_caret_start {
                        self.caret_frames.push(
                            started
                                .saturating_duration_since(previous)
                                .as_secs_f64()
                                * 1000.0,
                        );
                    }
                    self.last_caret_start = Some(started);
                    Self::inject_arrow(ui, *direction < 0.0);
                    <WorkbenchApp as eframe::App>::ui(&mut self.inner, ui, frame);
                    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
                    let after = {
                        let editor = self.inner.measure_editor();
                        (editor.caret_row, editor.caret_col)
                    };
                    let ui_ms = self.inner.measure_editor().last_ui_ms;
                    self.caret_editor.push(ui_ms);
                    if after != before {
                        self.caret.push(elapsed);
                        *samples += 1;
                        // Walk right for the first half, then back left, so neither run hits
                        // the line edge and reports a no-op as a latency sample.
                        if *samples == CARET_SAMPLES / 2 {
                            *direction = -1.0;
                        }
                    } else {
                        *attempts += 1;
                        if *attempts > 120 {
                            phase = Phase::Done;
                        }
                    }
                    self.phase = phase;
                    return;
                }
                self.scroll_offset_start = self.inner.measure_editor().scroll_y;
                phase = Phase::Scrolling {
                    frames: 0,
                    direction: 1.0,
                    last_start: None,
                };
            }
            Phase::Scrolling {
                frames,
                direction,
                last_start,
            } => {
                let start = Instant::now();
                if let Some(previous) = *last_start {
                    self.scroll.push(
                        start.saturating_duration_since(previous).as_secs_f64() * 1000.0,
                    );
                }
                *last_start = Some(start);
                let editor = self.inner.measure_editor();
                let row_h = editor.metrics.row_h.max(1.0);
                let max_scroll =
                    (editor.line_count() as f32 * row_h - editor.metrics.visible_rows as f32 * row_h)
                        .max(0.0);
                if editor.scroll_y >= max_scroll - 1.0 {
                    *direction = -1.0;
                } else if editor.scroll_y <= 0.0 {
                    *direction = 1.0;
                }
                editor.scroll_y =
                    (editor.scroll_y + SCROLL_STEP * *direction).clamp(0.0, max_scroll);
                *frames += 1;
                if *frames >= SCROLL_FRAMES {
                    phase = Phase::Done;
                }
            }
            Phase::Done => {}
        }
        self.phase = phase;
        if let Phase::Done = self.phase {
            self.write_report();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        <WorkbenchApp as eframe::App>::ui(&mut self.inner, ui, frame);
        if let Phase::Scrolling { frames, .. } = self.phase {
            if frames > 0 {
                let (ui_ms, rows_ms, syntax_ms) = {
                    let editor = self.inner.measure_editor();
                    (
                        editor.last_ui_ms,
                        editor.metrics.rows_ms,
                        editor.metrics.syntax_ms,
                    )
                };
                self.scroll_editor.push(ui_ms);
                self.scroll_rows.push(rows_ms);
                self.scroll_syntax.push(syntax_ms);
            }
        }
    }
}

/// Runs the measurement window and returns the process exit code.
pub fn run(script: Option<PathBuf>, out: Option<PathBuf>, started: Instant) -> i32 {
    let (Some(script), Some(out)) = (script, out) else {
        eprintln!("usage: lovelace-desktop --measure-large <script> --measure-out <report>");
        return 2;
    };
    let options = super::app::native_options();
    match eframe::run_native(
        "Lovelace Desktop - EDT-016 measurement",
        options,
        Box::new(move |creation| {
            // VIS-002: the measurement window paints the shipped dark token set; the embedded
            // WorkbenchApp applies its own (dark by default) tokens on every pass.
            super::theme::apply(&creation.egui_ctx, super::theme::ThemeId::Dark);
            Ok(Box::new(MeasureApp::new(script, out, started)))
        }),
    ) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("lovelace-desktop: could not open the measurement window: {error}");
            1
        }
    }
}

/// Frames the startup window paints before it writes the report and closes. Four passes prove
/// the first frame was followed by a normal repaint, not only a window that was created.
const STARTUP_FRAMES: usize = 4;

/// PERF-001: one launch of the real window, measured from process entry to the first completed
/// `App::ui` pass (the first frame in which the document is laid out and submitted).
struct StartupApp {
    inner: WorkbenchApp,
    out: PathBuf,
    started: Instant,
    open_ms: Option<f64>,
    frames: usize,
    /// The settings/recovery warnings this launch started with, captured before the app's own
    /// load could rewrite a corrupt settings file. Written into the report so the launch's
    /// corrupt-config behaviour is part of the measured artifact.
    startup_warnings: String,
    written: bool,
}

impl StartupApp {
    fn new(script: Option<PathBuf>, out: PathBuf, started: Instant) -> StartupApp {
        // Read the same two stores the window reads, before the window opens: a corrupt settings
        // file is rewritten on the first pass, so this is the only point at which the launch's
        // warnings describe the state the process actually started with.
        let settings_path = crate::settings::default_config_path();
        let settings_warnings = match &settings_path {
            Some(path) => crate::settings::Settings::load(path).take_warnings(),
            None => Vec::new(),
        };
        let recovery_warnings = match crate::settings::default_recovery_dir() {
            Some(root) => crate::recovery::scan(&root).warnings,
            None => Vec::new(),
        };
        let mut startup_warnings = String::new();
        startup_warnings.push_str(&format!(
            "settings_path={}\nsettings_warnings={}\n",
            settings_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "none".to_string()),
            settings_warnings.len()
        ));
        for warning in &settings_warnings {
            startup_warnings.push_str(&format!("settings_warning={warning}\n"));
        }
        startup_warnings.push_str(&format!("recovery_warnings={}\n", recovery_warnings.len()));
        for warning in &recovery_warnings {
            startup_warnings.push_str(&format!("recovery_warning={warning}\n"));
        }
        StartupApp {
            inner: WorkbenchApp::new(script),
            out,
            started,
            open_ms: None,
            frames: 0,
            startup_warnings,
            written: false,
        }
    }

    fn write_report(&mut self) {
        if self.written {
            return;
        }
        self.written = true;
        let open = self.open_ms.unwrap_or_default();
        let exe = std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        let mut report = String::new();
        report.push_str("LOVELACE PERF-001 startup measurement\n");
        report.push_str("method=release binary eframe/egui real window; open_ms = process entry (main, before argument parsing) to the end of the first completed App::ui pass; pid = this process\n");
        report.push_str(&format!("exe={exe}\n"));
        report.push_str(&format!("pid={}\n", std::process::id()));
        report.push_str(&format!("open_ms={open:.3}\n"));
        report.push_str(&format!("frames={}\n", self.frames));
        report.push_str(&self.startup_warnings);
        match std::fs::write(&self.out, report) {
            Ok(()) => eprintln!(
                "lovelace-desktop: PERF-001 startup report written to {}",
                self.out.display()
            ),
            Err(error) => eprintln!(
                "lovelace-desktop: could not write the PERF-001 report {}: {error}",
                self.out.display()
            ),
        }
    }
}

impl eframe::App for StartupApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        <WorkbenchApp as eframe::App>::ui(&mut self.inner, ui, frame);
        if self.open_ms.is_none() {
            self.open_ms = Some(self.started.elapsed().as_secs_f64() * 1000.0);
        }
        self.frames += 1;
        if self.frames >= STARTUP_FRAMES {
            self.write_report();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

/// Runs the PERF-001 startup window and returns the process exit code.
pub fn run_startup(script: Option<PathBuf>, out: Option<PathBuf>, started: Instant) -> i32 {
    let Some(out) = out else {
        eprintln!("usage: lovelace-desktop --measure-startup [script] --measure-out <report>");
        return 2;
    };
    let options = super::app::native_options();
    match eframe::run_native(
        "Lovelace Desktop - PERF-001 startup measurement",
        options,
        Box::new(move |creation| {
            super::theme::apply(&creation.egui_ctx, super::theme::ThemeId::Dark);
            Ok(Box::new(StartupApp::new(script, out, started)))
        }),
    ) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("lovelace-desktop: could not open the startup measurement window: {error}");
            1
        }
    }
}
