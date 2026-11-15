//! The PERF-009, PERF-011 and PERF-012 acceptance probes.
//!
//! Each probe drives a shipped product type; none of them re-implements the behaviour it
//! measures:
//!
//! - [`edit_scale_cli`] (PERF-009) opens the editor's own buffer from a real script and types
//!   10 000 characters through [`crate::ui::editor::TextEditor::insert_str`], the exact method the
//!   window's `egui::Event::Text` handler calls. It records every insertion's wall time and the
//!   process's working set, then undoes the run through the editor's own history. The row is
//!   about that buffer path, so this is a measurement of the product function the row names
//!   rather than a microbenchmark of an unrelated helper.
//! - [`precision_probe_cli`] (PERF-011) sets the maximum precision the product accepts through
//!   [`crate::workbench::Workbench::set_precision`], runs a trivial script and reports every log
//!   line the request produced. It measures what the product does with an extreme precision
//!   request; it does not evaluate an expensive kernel.
//! - [`paint_failure_cli`] (PERF-012) runs a real script that produces a plot and a result,
//!   feeds a malformed SVG into the shipped [`crate::ui::plot::Plot`] pane (the same `set_svg` the
//!   window calls when a capture arrives), and then runs a second script. The check is that the
//!   pane surfaces the parse error inline, the engine/document state is unchanged, and the next
//!   run still produces a result and a valid plot.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::run_state::RunState;
use crate::settings::MAX_PRECISION;
use crate::workbench::Workbench;

use crate::ui::editor::TextEditor;
use crate::ui::plot::Plot;

/// The default PERF-009 sample count: 10 000 typed characters.
pub const DEFAULT_CHARS: usize = 10_000;
/// How often the working set is sampled while typing, in characters.
const RSS_SAMPLE_EVERY: usize = 500;
/// How long the probe waits for a script to reach a terminal state.
const RUN_TIMEOUT: Duration = Duration::from_secs(30);
/// A typing pass slower than this is an input freeze under PERF-002's 100 ms ceiling; PERF-009
/// uses the same ceiling because "acceptable scaling" may not hide a visible stall.
const FREEZE_BUDGET_MS: f64 = 100.0;
/// The observed working-set growth a full-buffer clone per keystroke would force is astronomical
/// (10 000 x the document size). 32 MiB is far above any allocator slack this probe can produce
/// and far below the clone-per-keystroke cost of a 500 KB document.
const RSS_BUDGET_MB: f64 = 32.0;

/// One measured distribution, printed as n/p50/p95/max/mean.
#[derive(Clone, Copy, Debug)]
pub struct Distribution {
    /// Number of samples.
    pub count: usize,
    /// Median.
    pub p50: f64,
    /// 95th percentile (nearest-rank on the sorted samples).
    pub p95: f64,
    /// Largest sample.
    pub max: f64,
    /// Arithmetic mean.
    pub mean: f64,
}

impl Distribution {
    /// Computes the distribution of `samples` (milliseconds).
    pub fn from_samples(samples: &[f64]) -> Distribution {
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

    /// One report line in the format `label: n=... p50=...ms p95=...ms max=...ms mean=...ms`.
    pub fn line(&self, label: &str) -> String {
        format!(
            "{label}: n={} p50={:.3}ms p95={:.3}ms max={:.3}ms mean={:.3}ms",
            self.count, self.p50, self.p95, self.max, self.mean
        )
    }
}

/// The current process's (working set, peak working set) in bytes, when the OS reports them.
///
/// Windows asks the kernel (K32GetProcessMemoryInfo); Linux reads the same two numbers from
/// `/proc/self/status` (`VmRSS` and `VmHWM`, in kB), which is the kernel's own accounting. A
/// platform that reports neither returns `None` and the report omits the memory row.
fn process_memory() -> Option<(usize, usize)> {
    imp::process_memory()
}

/// Windows: the kernel's process memory counters through the Win32 FFI lane.
#[cfg(windows)]
mod imp {
    pub(super) fn process_memory() -> Option<(usize, usize)> {
        crate::win32::ffi::process_memory_bytes()
    }
}

/// Linux: the same two counters, read from the kernel's `/proc/self/status`.
#[cfg(unix)]
mod imp {
    /// The value of one `status` line in bytes; `kB` is the unit `/proc` prints.
    fn bytes_of(line: &str, key: &str) -> Option<usize> {
        let value = line.strip_prefix(key)?.trim();
        let value = value.strip_suffix("kB").unwrap_or(value).trim();
        value.parse::<usize>().ok().map(|kb| kb * 1024)
    }

    pub(super) fn process_memory() -> Option<(usize, usize)> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let mut working_set = None;
        let mut peak = None;
        for line in status.lines() {
            if let Some(value) = bytes_of(line, "VmRSS:") {
                working_set = Some(value);
            } else if let Some(value) = bytes_of(line, "VmHWM:") {
                peak = Some(value);
            }
        }
        working_set.zip(peak)
    }
}

/// Any other platform: no process-memory API is wired, so the row is omitted.
#[cfg(not(any(windows, unix)))]
mod imp {
    pub(super) fn process_memory() -> Option<(usize, usize)> {
        None
    }
}

fn write_report(path: &std::path::Path, text: &str) -> bool {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(path, text) {
        Ok(()) => {
            eprintln!(
                "lovelace-desktop: PERF probe report written to {}",
                path.display()
            );
            true
        }
        Err(error) => {
            eprintln!(
                "lovelace-desktop: could not write the PERF probe report {}: {error}",
                path.display()
            );
            false
        }
    }
}

/// PERF-009: types `chars` characters through the shipped editor and reports scaling.
///
/// The report is written before the process exits; the return value is 0 only when every
/// premise and every bound held. Usage:
/// `lovelace-desktop --measure-edit-scale <script> --measure-out <report> [--chars N]`.
pub fn edit_scale_cli(script: Option<PathBuf>, out: Option<PathBuf>, chars: Option<usize>) -> i32 {
    let chars = chars.unwrap_or(DEFAULT_CHARS).max(1);
    let (Some(script), Some(out)) = (script, out) else {
        eprintln!(
            "usage: lovelace-desktop --measure-edit-scale <script> --measure-out <report> [--chars N]"
        );
        return 2;
    };
    let source = match std::fs::read_to_string(&script) {
        Ok(source) => source,
        Err(error) => {
            eprintln!(
                "lovelace-desktop: cannot read {} for PERF-009: {error}",
                script.display()
            );
            return 2;
        }
    };
    let document_bytes = source.len();
    let document_lines = source.split('\n').count();
    let mut editor = TextEditor::from_source(&source);
    // The editor normalizes a CRLF file to its own line ending on load, so the undo comparison is
    // against the buffer the editor actually started with, not the raw file bytes.
    let initial_source = editor.source();
    let initial_bytes = editor.bytes();
    editor.set_caret(0, 0);

    let rss_start = process_memory();
    let started = Instant::now();
    let mut samples: Vec<f64> = Vec::with_capacity(chars);
    let mut rss_samples: Vec<(usize, usize)> = Vec::new();
    for index in 0..chars {
        let before = Instant::now();
        editor.insert_str("0");
        let elapsed = before.elapsed().as_secs_f64() * 1000.0;
        samples.push(elapsed);
        if index % RSS_SAMPLE_EVERY == 0 || index + 1 == chars {
            if let Some((working, _peak)) = process_memory() {
                rss_samples.push((index + 1, working));
            }
        }
    }
    let typing_seconds = started.elapsed().as_secs_f64();
    let undo_steps = editor.history.undo_steps();
    let buffer_bytes_after = editor.bytes();
    let buffer_lines_after = editor.line_count();

    // The one undo step a continuous typing burst recorded must restore the whole buffer.
    let undo_started = Instant::now();
    let undone = editor.undo();
    let undo_ms = undo_started.elapsed().as_secs_f64() * 1000.0;
    let restored_bytes = editor.bytes();
    let restored_equals_source = editor.source() == initial_source;

    let all = Distribution::from_samples(&samples);
    let first = Distribution::from_samples(&samples[..samples.len().min(1000)]);
    let last = Distribution::from_samples(
        &samples[samples.len().saturating_sub(samples.len().min(1000))..],
    );
    let rss_start_bytes = rss_start.map(|(working, _)| working).unwrap_or(0);
    let rss_end_bytes = rss_samples
        .last()
        .map(|(_, working)| *working)
        .unwrap_or(rss_start_bytes);
    let rss_peak_bytes = rss_samples
        .iter()
        .map(|(_, working)| *working)
        .chain(std::iter::once(rss_start_bytes))
        .max()
        .unwrap_or(0);
    let rss_delta_mb = (rss_peak_bytes.saturating_sub(rss_start_bytes)) as f64 / (1024.0 * 1024.0);
    let clone_bytes_if_per_keystroke = chars as u128 * document_bytes as u128;
    let ratio = if first.p95 > 0.0 {
        last.p95 / first.p95
    } else {
        0.0
    };

    let mut report = String::new();
    report.push_str("LOVELACE PERF-009 edit-scale probe\n");
    report.push_str(
        "method=release binary headless drive of the shipped TextEditor buffer: 10000 x TextEditor::insert_str(\"0\"), the exact method the window's Event::Text handler calls; per-insertion wall time and process working set (K32GetProcessMemoryInfo); one editor.undo() through the shipped history\n",
    );
    report.push_str(&format!("script={}\n", script.display()));
    report.push_str(&format!("document_bytes={document_bytes}\n"));
    report.push_str(&format!("document_lines={document_lines}\n"));
    report.push_str(&format!(
        "buffer_bytes_before={initial_bytes} buffer_bytes_after_typing={buffer_bytes_after}\n"
    ));
    report.push_str(&format!("chars={chars}\n"));
    report.push_str(&format!("typing_seconds={typing_seconds:.3}\n"));
    report.push_str(&all.line("edit_latency"));
    report.push_str(&format!("\n{}", first.line("first_block")));
    report.push_str(&format!("\n{}", last.line("last_block")));
    report.push_str(&format!("\nblock_p95_ratio={ratio:.3}\n"));
    report.push_str(&format!(
        "buffer_bytes_after={buffer_bytes_after} buffer_lines_after={buffer_lines_after}\n"
    ));
    report.push_str(&format!(
        "rss_start_mb={:.3} rss_end_mb={:.3} rss_peak_mb={:.3} rss_delta_mb={rss_delta_mb:.3}\n",
        rss_start_bytes as f64 / (1024.0 * 1024.0),
        rss_end_bytes as f64 / (1024.0 * 1024.0),
        rss_peak_bytes as f64 / (1024.0 * 1024.0),
    ));
    report.push_str(&format!(
        "undo_steps_after_typing={undo_steps} undo_returned={undone} undo_ms={undo_ms:.3} restored_bytes={restored_bytes} restored_equals_source={restored_equals_source}\n"
    ));
    report.push_str(&format!(
        "full_buffer_clone_bytes_if_one_per_keystroke={clone_bytes_if_per_keystroke}\n"
    ));
    // The row does not state a millisecond number: its contract is "no unbounded full-buffer
    // clones per keystroke; memory/CPU profiling confirms acceptable scaling". These are the
    // falsifiable bounds this probe applies to that sentence.
    let scaling_ok = last.p95 <= (first.p95 * 3.0).max(16.0);
    let freeze_ok = all.max <= FREEZE_BUDGET_MS;
    let memory_ok = rss_delta_mb <= RSS_BUDGET_MB;
    let history_ok = undo_steps == 1 && undone && restored_equals_source;
    let pass = all.count == chars && scaling_ok && freeze_ok && memory_ok && history_ok;
    let criteria = format!(
        "criteria: n==chars {}; last_block_p95<=max(3x first_block_p95,16ms) {}; max<={:.0}ms {}; rss_delta<={:.0}MB {}; undo_steps==1 and undo restores the source {}\n",
        all.count == chars,
        scaling_ok,
        FREEZE_BUDGET_MS,
        freeze_ok,
        RSS_BUDGET_MB,
        memory_ok,
        history_ok
    );
    report.push_str(&criteria);
    report.push_str(if pass {
        "PERF-009: PASS\n"
    } else {
        "PERF-009: FAIL\n"
    });
    let written = write_report(&out, &report);
    if pass && written { 0 } else { 1 }
}

/// PERF-011: reports what the product does with the largest precision it accepts.
///
/// The probe sets the precision through the workbench method the toolbar calls, runs a trivial
/// statement and prints every log line. A warning, estimate or refusal would appear there; a
/// silent acceptance is the observable this probe is looking for.
pub fn precision_probe_cli() -> i32 {
    let mut workbench = Workbench::new();
    workbench.set_precision(0, MAX_PRECISION);
    workbench.set_source(0, "1 + 1".to_string());
    let submitted = workbench.run_active();
    let idle = workbench.wait_until_idle(RUN_TIMEOUT);
    let document = &workbench.documents()[0];
    let result = document
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.result.clone())
        .unwrap_or_default();
    let log_lines: Vec<String> = document.logs.iter().map(|line| line.render()).collect();
    let warnings: Vec<&String> = log_lines
        .iter()
        .filter(|line| {
            let lower = line.to_lowercase();
            (lower.contains("estimat")
                || lower.contains("budget")
                || lower.contains("refus")
                || lower.contains("memory")
                || lower.contains("may take")
                || lower.contains("warning"))
                && !lower.contains("precision for")
        })
        .collect();
    let refused = !submitted || document.state() != RunState::Finished;
    println!(
        "PERF-011 probe: precision={MAX_PRECISION} digits; submitted={submitted}; idle={idle}; state={:?}; result={result:?}",
        document.state()
    );
    for line in &log_lines {
        println!("PERF-011 log: {line}");
    }
    println!(
        "PERF-011 observed: warnings={} refusal={refused} estimate=none",
        warnings.len()
    );
    // Exit 0 only when the row's capability exists (a warning/estimate or a refusal).
    if warnings.is_empty() && !refused {
        1
    } else {
        0
    }
}

/// PERF-012: an invalid plot must surface inline without corrupting the engine/document, and the
/// next run must work.
pub fn paint_failure_cli() -> i32 {
    let first_source =
        "a = 2\nb = a + 3\nplot([1, 4, 2], [4, 5, 6], \"perf-012\")\nb\n".to_string();
    let second_source = "c = 7\nplot([1, 2], [2, 3], \"perf-012-next\")\nc\n".to_string();
    let mut workbench = Workbench::new();
    workbench.set_source(0, first_source.clone());
    let first_submitted = workbench.run_active();
    let first_idle = workbench.wait_until_idle(RUN_TIMEOUT);
    let first_state = workbench.documents()[0].state();
    let first_snapshot = workbench.documents()[0].snapshot.clone();
    let first_plot = workbench.documents()[0].last_plot.clone();
    let first_logs: Vec<String> = workbench.documents()[0]
        .logs
        .iter()
        .map(|line| line.render())
        .collect();
    let Some(first_snapshot) = first_snapshot else {
        println!(
            "PERF-012: FAIL the first run published no workspace snapshot (submitted={first_submitted} idle={first_idle} state={first_state:?} logs={first_logs:?})"
        );
        return 1;
    };
    let Some(first_plot) = first_plot else {
        println!(
            "PERF-012: FAIL the first run produced no plot capture (submitted={first_submitted} idle={first_idle} state={first_state:?} logs={first_logs:?})"
        );
        return 1;
    };
    let mut pane = Plot::new();
    let valid_loaded = pane.set_svg(&first_plot.svg);
    let valid_error = pane.error.clone();

    let invalid = "<svg xmlns=\"http://www.w3.org/2000/svg\"><path d=\"M 0 0 L";
    let invalid_loaded = pane.set_svg(invalid);
    let invalid_error = pane.error.clone();
    let invalid_surface = !invalid_loaded && invalid_error.is_some();

    let after_source = workbench.documents()[0].source.clone();
    let after_state = workbench.documents()[0].state();
    let after_snapshot = workbench.documents()[0].snapshot.clone();
    let after_plot = workbench.documents()[0].last_plot.clone();
    let unchanged = after_source == first_source
        && after_state == first_state
        && after_snapshot.as_ref() == Some(&first_snapshot)
        && after_plot.as_ref().map(|plot| plot.svg.clone()) == Some(first_plot.svg.clone());

    workbench.set_source(0, second_source.clone());
    let second_submitted = workbench.run_active();
    let second_idle = workbench.wait_until_idle(RUN_TIMEOUT);
    let second_state = workbench.documents()[0].state();
    let second_snapshot = workbench.documents()[0].snapshot.clone();
    let second_result = second_snapshot
        .as_ref()
        .map(|snapshot| snapshot.result.clone())
        .unwrap_or_default();
    let second_plot = workbench.documents()[0].last_plot.clone();
    let next_works = second_state == RunState::Finished && second_result == "7";
    let recovered = match second_plot {
        Some(plot) => {
            let loaded = pane.set_svg(&plot.svg);
            loaded && pane.error.is_none() && second_result == "7"
        }
        None => false,
    };
    let pass = first_submitted
        && first_idle
        && first_state == RunState::Finished
        && first_snapshot.result == "5"
        && valid_loaded
        && valid_error.is_none()
        && invalid_surface
        && unchanged
        && second_submitted
        && second_idle
        && next_works
        && recovered;
    println!(
        "PERF-012 first_run: submitted={first_submitted} idle={first_idle} state={first_state:?} result={:?} plot_bytes={} variables={:?}",
        first_snapshot.result,
        first_plot.svg.len(),
        first_snapshot.variables
    );
    println!("PERF-012 invalid_plot: accepted={invalid_loaded} inline_error={invalid_error:?}");
    println!(
        "PERF-012 after_invalid: source_unchanged={} state={after_state:?} snapshot_unchanged={} plot_unchanged={}",
        after_source == first_source,
        after_snapshot.as_ref() == Some(&first_snapshot),
        after_plot.as_ref().map(|plot| plot.svg.clone()) == Some(first_plot.svg.clone())
    );
    println!(
        "PERF-012 next_run: submitted={second_submitted} idle={second_idle} state={second_state:?} result={second_result:?} valid_plot_reloaded={recovered}"
    );
    println!("PERF-012: {}", if pass { "PASS" } else { "FAIL" });
    if pass { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbers(count: usize, value: f64) -> Vec<f64> {
        vec![value; count]
    }

    #[test]
    fn percentiles_are_order_statistics() {
        let values: Vec<f64> = (1..=100).map(|value| value as f64).collect();
        let distribution = Distribution::from_samples(&values);
        assert_eq!(distribution.count, 100);
        assert_eq!(distribution.p50, 51.0);
        assert_eq!(distribution.p95, 95.0);
        assert_eq!(distribution.max, 100.0);
        assert_eq!(distribution.mean, 50.5);
    }

    #[test]
    fn an_empty_distribution_is_zero_and_never_panics() {
        let distribution = Distribution::from_samples(&[]);
        assert_eq!(distribution.count, 0);
        assert_eq!(distribution.p95, 0.0);
        assert_eq!(distribution.max, 0.0);
    }

    #[test]
    fn a_growing_typing_latency_fails_the_scaling_bound() {
        let mut samples = numbers(1000, 1.0);
        samples.extend(numbers(1000, 100.0));
        let first = Distribution::from_samples(&samples[..1000]);
        let last = Distribution::from_samples(&samples[1000..]);
        assert!(
            last.p95 > (first.p95 * 3.0).max(16.0),
            "a 100x regression must not pass the 3x bound"
        );
    }
}
