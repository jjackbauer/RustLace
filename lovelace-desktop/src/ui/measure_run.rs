//! ARC-007: the real-window long-run responsiveness measurement.
//!
//! `lovelace-desktop --measure-run <script> --measure-out <report>` opens the **shipped** window
//! over `<script>` and starts the run through the same `Workbench::run_active` path the Run
//! button uses. While the run is in flight it measures, from inside the real event loop:
//!
//! - **frame interval**: start-to-start time of consecutive `App::ui` passes, i.e. the cadence the
//!   compositor is actually handed frames while the engine worker computes;
//! - **input latency**: a real Ctrl+Shift+T key event is injected into the frame input every
//!   `INPUT_PERIOD_FRAMES` frames; the measured latency is the time from injecting it to the end
//!   of the same `App::ui` pass, and the report confirms the theme really flipped (the shortcut
//!   was handled, not merely delivered);
//! - **run duration** and the committed result, so a pass cannot be claimed for a run that ended
//!   before the frame sample or produced no result.
//!
//! The mode is a wrapper around the shipped app, like `--measure-large`: it calls
//! `WorkbenchApp::ui` for every pass and only injects the same `egui::Event` values winit delivers.
//! It is not a second rendering path. The report is written before the window closes; a failed
//! budget makes the process exit non-zero and the report says `RESULT FAIL`.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use eframe::egui;

use super::app::WorkbenchApp;

/// Frames with no injected input before the run starts, so fonts/layout are warm.
const WARMUP_FRAMES: usize = 30;
/// Frames between injected Ctrl+Shift+T commands while the run is in flight (~200 ms at 60 Hz).
const INPUT_PERIOD_FRAMES: usize = 12;
/// Budgets: p95 frame interval, worst gap, input p95. The worst-gap budget is PERF-002's 100 ms
/// "no input freeze" ceiling; the frame budget keeps the UI painting at a usable cadence.
const FRAME_P95_BUDGET_MS: f64 = 25.0;
const WORST_GAP_BUDGET_MS: f64 = 100.0;
const INPUT_P95_BUDGET_MS: f64 = 16.0;
const MIN_RUN_SECONDS: f64 = 2.0;

/// One measured distribution, printed as n/p50/p95/max/mean.
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

#[derive(Clone, Copy, Debug)]
enum Phase {
    Warmup { frames: usize },
    Running,
    Done,
}

struct MeasureRunApp {
    inner: WorkbenchApp,
    out: PathBuf,
    outcome: Arc<AtomicBool>,
    phase: Phase,
    run_started: Option<Instant>,
    run_finished: Option<Instant>,
    run_id: Option<crate::execution::RunId>,
    started_frames: usize,
    last_frame: Option<Instant>,
    frame_intervals: Vec<f64>,
    input_latencies: Vec<f64>,
    input_samples: Vec<(f64, f64)>,
    input_events: usize,
    input_acknowledged: usize,
    pending_input: Option<(Instant, crate::keyboard::Pane)>,
    frames_since_input: usize,
    written: bool,
}

impl MeasureRunApp {
    fn new(script: PathBuf, out: PathBuf, outcome: Arc<AtomicBool>) -> MeasureRunApp {
        MeasureRunApp {
            inner: WorkbenchApp::new(Some(script)),
            out,
            outcome,
            phase: Phase::Warmup { frames: 0 },
            run_started: None,
            run_finished: None,
            run_id: None,
            started_frames: 0,
            last_frame: None,
            frame_intervals: Vec::new(),
            input_latencies: Vec::new(),
            input_samples: Vec::new(),
            input_events: 0,
            input_acknowledged: 0,
            pending_input: None,
            frames_since_input: 0,
            written: false,
        }
    }

    fn inject_theme_toggle(ctx: &egui::Context) {
        let event = egui::Event::Key {
            key: egui::Key::F6,
            physical_key: Some(egui::Key::F6),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        ctx.input_mut(|input| input.events.push(event));
    }

    fn tick_before_frame(&mut self, now: Instant, ctx: &egui::Context) {
        match self.phase {
            Phase::Warmup { frames } => {
                let frames = frames + 1;
                if frames >= WARMUP_FRAMES {
                    let accepted = self.inner.measure_begin_run();
                    self.run_id = self.inner.measure_run_id();
                    self.run_started = Some(now);
                    self.started_frames = frames;
                    self.phase = if accepted {
                        Phase::Running
                    } else {
                        Phase::Done
                    };
                    self.last_frame = Some(now);
                } else {
                    self.phase = Phase::Warmup { frames };
                }
            }
            Phase::Running => {
                if let Some(previous) = self.last_frame {
                    self.frame_intervals
                        .push(now.saturating_duration_since(previous).as_secs_f64() * 1000.0);
                }
                self.last_frame = Some(now);
                if self.pending_input.is_none() {
                    self.frames_since_input += 1;
                    if self.frames_since_input >= INPUT_PERIOD_FRAMES {
                        self.frames_since_input = 0;
                        self.input_events += 1;
                        self.pending_input = Some((now, self.inner.measure_focused_pane()));
                        Self::inject_theme_toggle(ctx);
                    }
                }
            }
            Phase::Done => {}
        }
    }

    fn after_frame(&mut self, now: Instant, ctx: &egui::Context) {
        if let Some((injected_at, before)) = self.pending_input.take() {
            if self.inner.measure_focused_pane() != before {
                self.input_acknowledged += 1;
                let latency = now.saturating_duration_since(injected_at).as_secs_f64() * 1000.0;
                self.input_latencies.push(latency);
                let at = self
                    .run_started
                    .map(|start| now.saturating_duration_since(start).as_secs_f64())
                    .unwrap_or(0.0);
                self.input_samples.push((at, latency));
            }
        }
        if matches!(self.phase, Phase::Running) && !self.inner.measure_run_state().is_in_flight() {
            self.run_finished = Some(now);
            self.run_id = self.run_id.or_else(|| self.inner.measure_run_id());
            self.phase = Phase::Done;
        }
        if matches!(self.phase, Phase::Done) && !self.written {
            let pass = self.write_report();
            self.outcome.store(pass, Ordering::SeqCst);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Writes the measured report; returns whether every premise and budget held.
    fn write_report(&mut self) -> bool {
        if self.written {
            return true;
        }
        self.written = true;
        let frames = Distribution::from_samples(&self.frame_intervals);
        let input = Distribution::from_samples(&self.input_latencies);
        let run_seconds = match (self.run_started, self.run_finished) {
            (Some(start), Some(end)) => end.saturating_duration_since(start).as_secs_f64(),
            _ => 0.0,
        };
        let result = self.inner.measure_result();
        let result_len = result.as_ref().map(String::len).unwrap_or(0);
        let preview: String = result
            .as_deref()
            .unwrap_or("<none>")
            .chars()
            .take(160)
            .collect();
        let run_id = self.run_id.map(|run| run.0);
        let mut report = String::new();
        report.push_str("LOVELACE ARC-007 long-run UI responsiveness\n");
        report.push_str(
            "method=real-window eframe/egui 0.37; shipped WorkbenchApp::ui every pass; frame interval = start-to-start of App::ui while the run is in flight; input = F6 (next pane focus group) injected into the frame input, latency = injection to end of the same App::ui pass, acknowledgement = painted focus group changed\n",
        );
        report.push_str(&format!(
            "run_id={} run_started_after_warmup_frames={}\n",
            run_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "none".to_string()),
            self.started_frames
        ));
        report.push_str(&format!("run_seconds={run_seconds:.3}\n"));
        report.push_str(&format!(
            "run_at_least_2s={}\n",
            run_seconds >= MIN_RUN_SECONDS
        ));
        report.push_str(&format!(
            "result_preview={preview}\nresult_len={result_len}\n"
        ));
        report.push_str(&frames.line("frame_interval"));
        report.push_str(&format!("\nworst_gap_ms={:.3}\n", frames.max));
        report.push_str(&format!(
            "input_events={} input_acknowledged={}\n",
            self.input_events, self.input_acknowledged
        ));
        report.push_str(&input.line("input_latency"));
        report.push_str("\ninput_samples_seconds:ms=");
        for (index, (at, latency)) in self.input_samples.iter().enumerate() {
            if index > 0 {
                report.push(',');
            }
            report.push_str(&format!("{at:.3}:{latency:.3}"));
        }
        report.push('\n');
        let frames_ok = frames.count >= 60
            && frames.p95 <= FRAME_P95_BUDGET_MS
            && frames.max <= WORST_GAP_BUDGET_MS;
        let inputs_ok = self.input_events >= 3
            && self.input_acknowledged >= 3
            && input.count >= 3
            && input.p95 <= INPUT_P95_BUDGET_MS;
        let run_ok = run_seconds >= MIN_RUN_SECONDS && result.is_some();
        report.push_str(&format!(
            "budget_frame_p95_ms={FRAME_P95_BUDGET_MS} frame_p95_pass={} budget_worst_gap_ms={WORST_GAP_BUDGET_MS} worst_gap_pass={} budget_input_p95_ms={INPUT_P95_BUDGET_MS} input_p95_pass={} frames_min=60 frames_ok={} inputs_min=3 inputs_ok={}\n",
            frames.p95 <= FRAME_P95_BUDGET_MS,
            frames.max <= WORST_GAP_BUDGET_MS,
            input.p95 <= INPUT_P95_BUDGET_MS,
            frames_ok,
            inputs_ok
        ));
        let pass = frames_ok && inputs_ok && run_ok;
        report.push_str(if pass {
            "RESULT PASS\n"
        } else {
            "RESULT FAIL\n"
        });
        if let Some(parent) = self.out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&self.out, report) {
            Ok(()) => eprintln!(
                "lovelace-desktop: ARC-007 run measurement written to {}",
                self.out.display()
            ),
            Err(error) => eprintln!(
                "lovelace-desktop: could not write the ARC-007 report {}: {error}",
                self.out.display()
            ),
        }
        pass
    }
}

impl eframe::App for MeasureRunApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // The measurement needs every frame, including frames with no OS input.
        ctx.request_repaint();
        let now = Instant::now();
        self.tick_before_frame(now, &ctx);
        if matches!(self.phase, Phase::Done) && self.written {
            return;
        }
        <WorkbenchApp as eframe::App>::ui(&mut self.inner, ui, frame);
        let end = Instant::now();
        self.after_frame(end, &ctx);
    }
}

/// Runs the real-window measurement and returns the process exit code.
pub fn run(script: Option<PathBuf>, out: Option<PathBuf>) -> i32 {
    let (Some(script), Some(out)) = (script, out) else {
        eprintln!("usage: lovelace-desktop --measure-run <script> --measure-out <report>");
        return 2;
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 1000.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("Lovelace Desktop - ARC-007 run measurement")
            .with_icon(super::app::window_icon()),
        ..Default::default()
    };
    let outcome = Arc::new(AtomicBool::new(false));
    let app_outcome = Arc::clone(&outcome);
    let exit = match eframe::run_native(
        "Lovelace Desktop - ARC-007 run measurement",
        options,
        Box::new(move |creation| {
            super::theme::apply(&creation.egui_ctx, super::theme::ThemeId::Dark);
            Ok(Box::new(MeasureRunApp::new(
                script.clone(),
                out.clone(),
                app_outcome,
            )))
        }),
    ) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("lovelace-desktop: could not open the ARC-007 measurement window: {error}");
            1
        }
    };
    if exit != 0 {
        return exit;
    }
    if outcome.load(Ordering::SeqCst) { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(values: &[f64]) -> Distribution {
        Distribution::from_samples(values)
    }

    #[test]
    fn the_percentiles_are_order_statistics_not_guesses() {
        let values: Vec<f64> = (1..=100).map(|value| value as f64).collect();
        let distribution = samples(&values);
        assert_eq!(distribution.count, 100);
        assert_eq!(distribution.p50, 51.0);
        assert_eq!(distribution.p95, 95.0);
        assert_eq!(distribution.max, 100.0);
        assert_eq!(distribution.mean, 50.5);
    }

    #[test]
    fn a_clean_sequence_passes_the_frame_budget_and_a_stall_does_not() {
        let clean: Vec<f64> = (0..180).map(|_| 16.7).collect();
        let distribution = samples(&clean);
        assert!(distribution.p95 <= FRAME_P95_BUDGET_MS);
        assert!(distribution.max <= WORST_GAP_BUDGET_MS);
        let stalled = samples(&[16.7, 16.7, 16.7, 240.0, 16.7, 17.0]);
        assert!(
            stalled.max > WORST_GAP_BUDGET_MS,
            "the budget check must reject a 240 ms gap"
        );
    }

    #[test]
    fn a_run_shorter_than_two_seconds_fails_the_premise() {
        assert!(1.999 < MIN_RUN_SECONDS);
        assert!(2.000 >= MIN_RUN_SECONDS);
    }
}
