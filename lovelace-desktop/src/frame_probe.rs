//! Diagnostic: what cadence does eframe/egui actually deliver on this machine?
//!
//! `lovelace-desktop --perf-frame-probe --measure-out <report> [--probe-request=after16]` opens a
//! real eframe window with a trivial UI (one label) and records the start-to-start interval of
//! every pass for 600 passes after 60 warmup passes. It is the control experiment for
//! `--measure-large`: it isolates the frame-pacing behaviour of the windowing stack from the
//! shipped workbench, so a long frame can be attributed either to what the app paints or to the
//! pace at which the event loop is woken.
//!
//! `LOVELACE_PROBE_DWMFLUSH=1` additionally blocks on `DwmFlush` once per pass. That is the
//! documented way to wait for the desktop compositor to finish a frame; when the compositor has
//! nothing to compose (a locked session, for example) the call returns immediately, which the
//! recorded pass durations show.
//!
//! Nothing here is part of a PERF row: no fps budget is measured or reported by this mode, and
//! `--measure-large` does not use it.

use std::path::PathBuf;
use std::time::Instant;

use eframe::egui;

use crate::ui::native_options;
use crate::ui::theme;

/// Passes painted before recording starts, so window creation/font upload are excluded.
const WARMUP_PASSES: usize = 60;
/// Passes recorded.
const MEASURED_PASSES: usize = 600;

/// How the probe asks for its next pass.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestMode {
    /// `Context::request_repaint` every pass: what the EDT-016 driver does.
    Immediate,
    /// `Context::request_repaint_after(16 ms)` every pass: a timer-paced request.
    After16,
}

struct ProbeApp {
    out: PathBuf,
    mode: RequestMode,
    /// Block on the desktop compositor once per pass (`LOVELACE_PROBE_DWMFLUSH=1`).
    compositor_flush: bool,
    passes: usize,
    last: Option<Instant>,
    intervals: Vec<f64>,
    pass_ms: Vec<f64>,
    cpu_ms: Vec<f64>,
    written: bool,
}

impl ProbeApp {
    fn new(out: PathBuf, mode: RequestMode) -> ProbeApp {
        ProbeApp {
            out,
            mode,
            compositor_flush: std::env::var("LOVELACE_PROBE_DWMFLUSH")
                .is_ok_and(|value| value == "1"),
            passes: 0,
            last: None,
            intervals: Vec::new(),
            pass_ms: Vec::new(),
            cpu_ms: Vec::new(),
            written: false,
        }
    }

    fn write_report(&mut self) {
        if self.written {
            return;
        }
        self.written = true;
        let mut csv = String::from("index,interval_ms,pass_ms,cpu_ms\n");
        for index in 0..self.intervals.len() {
            csv.push_str(&format!(
                "{},{:.3},{:.3},{:.3}\n",
                index,
                self.intervals[index],
                self.pass_ms.get(index).copied().unwrap_or_default(),
                self.cpu_ms.get(index).copied().unwrap_or_default()
            ));
        }
        match std::fs::write(&self.out, csv) {
            Ok(()) => eprintln!(
                "lovelace-desktop: frame probe written to {}",
                self.out.display()
            ),
            Err(error) => eprintln!(
                "lovelace-desktop: could not write the frame probe {}: {error}",
                self.out.display()
            ),
        }
    }
}

impl eframe::App for ProbeApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let started = Instant::now();
        let previous = self.last;
        self.last = Some(started);
        self.passes += 1;
        match self.mode {
            RequestMode::Immediate => ui.ctx().request_repaint(),
            RequestMode::After16 => {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(16));
            }
        }
        ui.label("frame probe");
        // A pass that follows the warmup records the interval since the previous pass.
        if self.passes > WARMUP_PASSES
            && let Some(previous) = previous
        {
            self.intervals
                .push(started.saturating_duration_since(previous).as_secs_f64() * 1000.0);
            self.pass_ms.push(started.elapsed().as_secs_f64() * 1000.0);
            self.cpu_ms.push(
                frame
                    .info()
                    .cpu_usage
                    .map(|seconds| f64::from(seconds) * 1000.0)
                    .unwrap_or_default(),
            );
        }
        if self.passes >= WARMUP_PASSES + MEASURED_PASSES + 1 {
            self.write_report();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.compositor_flush {
            crate::win32::ffi::flush_compositor();
        }
    }
}

/// Runs the frame-pacing probe and returns the process exit code.
pub fn run(out: Option<PathBuf>, mode: RequestMode) -> i32 {
    let Some(out) = out else {
        eprintln!("usage: lovelace-desktop --perf-frame-probe --measure-out <report>");
        return 2;
    };
    let options = native_options();
    match eframe::run_native(
        "Lovelace Desktop - frame probe",
        options,
        Box::new(move |creation| {
            theme::apply(&creation.egui_ctx, theme::ThemeId::Dark);
            Ok(Box::new(ProbeApp::new(out, mode)))
        }),
    ) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("lovelace-desktop: could not open the frame probe window: {error}");
            1
        }
    }
}
