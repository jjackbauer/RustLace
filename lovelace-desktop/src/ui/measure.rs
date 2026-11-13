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
/// Frames measured while the document scrolls.
const SCROLL_FRAMES: usize = 240;
/// Pixels the view advances on every scroll frame: three rows at the editor's row height.
const SCROLL_STEP: f32 = 48.0;

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

    fn write_report(&mut self) {
        if self.written {
            return;
        }
        self.written = true;
        let typing = Distribution::from_samples(&self.typing);
        let typing_frames = Distribution::from_samples(&self.typing_frames);
        let typing_editor = Distribution::from_samples(&self.typing_editor);
        let typing_rows = Distribution::from_samples(&self.typing_rows);
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
        let open = self.open_ms.unwrap_or_default();
        let mut report = String::new();
        report.push_str("LOVELACE EDT-016 large-file measurement\n");
        report.push_str("method=real-window eframe/egui; event injected at frame start; typing latency = start of the edit pass to the end of that pass (UI list complete); scroll = start-to-start frame intervals while the view scrolls\n");
        report.push_str(&format!("document_bytes={}\n", self.bytes));
        report.push_str(&format!("document_lines={}\n", self.lines));
        report.push_str(&format!("open_ms={open:.3}\n"));
        report.push_str(&typing.line("typing_latency"));
        report.push_str(&format!("\n{}", typing_editor.line("typing_editor_pass")));
        report.push_str(&format!("\n{}", typing_rows.line("typing_rows_pass")));
        report.push_str(&format!("\n{}", typing_frames.line("typing_frame_interval")));
        report.push_str(&format!("\nscroll_frames={} scroll_seconds={:.3} fps={fps:.2}\n", self.scroll.len(), self.scroll.iter().sum::<f64>() / 1000.0));
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
            "budget_typing_p95_ms=16 typing_p95_pass={}\n",
            typing.p95 <= 16.0
        ));
        report.push_str(&format!(
            "budget_scroll_fps_min=55 scroll_fps_pass={}\n",
            fps >= 55.0
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
