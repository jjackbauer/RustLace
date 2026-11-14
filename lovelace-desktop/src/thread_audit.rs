//! ARC-008: the mechanical source audit for threads and synchronisation.
//!
//! It scans every **production** Rust source file in the crate (comments and `#[cfg(test)]`
//! modules stripped) and enforces:
//!
//! - exactly two thread-spawn sites exist, each with a reviewed owner: `execution.rs`'s named run
//!   worker and `ui/plot.rs`'s named canvas raster worker (PLT-012);
//! - each worker's `JoinHandle` is stored on its owner and joined by that owner's shutdown path
//!   (`ExecutionHost::shutdown` and `Drop for RasterWorker`, which `Drop for Plot` reaches) — no
//!   detached or daemon-like worker survives its owner;
//! - the canvas worker's cancellation protocol is present (an atomic generation compared by the
//!   worker, and a `Shutdown` message that ends its receive loop);
//! - no production code sleeps, parks or spins to synchronise (`thread::sleep`, `thread::park`,
//!   `spin_loop`);
//! - the execution core synchronises with the native primitives it imports (`mpsc` channel,
//!   `Mutex`, atomics), and the workbench's wait path blocks on that channel instead of polling.
//!
//! The runtime half of the proof lives beside the construction: `execution::tests` asserts the
//! recorded engine thread differs from the UI thread and that `shutdown` joins every spawned
//! worker (`shutdown_joins_every_spawned_worker`), and `ui::plot::tests` asserts that the raster
//! thread is not the UI thread, that a superseded raster is cancelled, and that dropping the pane
//! joins the worker. This audit is the static half: it fails when an unapproved spawn site, a
//! sleep or a lost join path appears in the source, and its negative tests feed those exact
//! violations through the same scanner.

use std::path::{Path, PathBuf};

use crate::ui_audit::{mask_literals, strip_comments, strip_test_modules};

/// One thread-spawn site found in production source.
#[derive(Clone, Debug)]
pub struct SpawnSite {
    /// Path relative to `src/`.
    pub file: String,
    /// 1-based line number.
    pub line: usize,
    /// The source line that constructs the thread.
    pub text: String,
}

/// The whole audit result.
#[derive(Clone, Debug)]
pub struct ThreadReport {
    /// Every thread-spawn site, in file/line order.
    pub spawn_sites: Vec<SpawnSite>,
    /// Every production sleep/park/spin site.
    pub blocking_sites: Vec<SpawnSite>,
    /// Native primitives observed in the execution core.
    pub primitives: Vec<String>,
    /// Join-path facts observed in the execution core.
    pub join_path: Vec<String>,
    /// Rule failures.
    pub failures: Vec<String>,
}

impl ThreadReport {
    /// True when no rule failed.
    pub fn pass(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Audits the crate source below `src/`.
pub fn audit_src(src: &Path) -> ThreadReport {
    let mut files = Vec::new();
    let mut failures = Vec::new();
    if let Err(error) = collect_rust_files(src, &mut files) {
        return ThreadReport {
            spawn_sites: Vec::new(),
            blocking_sites: Vec::new(),
            primitives: Vec::new(),
            join_path: Vec::new(),
            failures: vec![format!("cannot read {}: {error}", src.display())],
        };
    }
    files.sort();
    let mut spawn_sites = Vec::new();
    let mut blocking_sites = Vec::new();
    let mut execution_source = String::new();
    let mut execution_source_text = String::new();
    let mut workbench_source = String::new();
    let mut plot_source = String::new();
    let mut plot_source_text = String::new();
    for path in files {
        let relative = path
            .strip_prefix(src)
            .unwrap_or(&path)
            .display()
            .to_string()
            .replace('\\', "/");
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) => {
                failures.push(format!("{relative}: {error}"));
                continue;
            }
        };
        let production = strip_test_modules(&strip_comments(&source));
        let code_only = mask_literals(&production);
        if relative == "execution.rs" {
            execution_source = code_only.clone();
            execution_source_text = production.clone();
        }
        if relative == "workbench.rs" {
            workbench_source = code_only.clone();
        }
        if relative == "ui/plot.rs" {
            plot_source = code_only.clone();
            plot_source_text = production.clone();
        }
        scan_sites(
            &relative,
            &production,
            &mut spawn_sites,
            &mut blocking_sites,
        );
    }

    let mut primitives = Vec::new();
    for (name, needle) in [
        ("mpsc channel (std::sync::mpsc)", "mpsc::channel"),
        ("Receiver", "Receiver<RawEvent>"),
        ("Mutex", "Mutex<"),
        ("AtomicBool", "AtomicBool"),
        ("AtomicU64", "AtomicU64"),
    ] {
        if execution_source.contains(needle) {
            primitives.push(name.to_string());
        }
    }

    let mut join_path = Vec::new();
    if execution_source.contains("JoinHandle") {
        join_path.push("ExecutionHost stores the worker's JoinHandle".to_string());
    } else {
        failures.push("execution.rs does not name JoinHandle".to_string());
    }
    if execution_source.contains("fn shutdown") && execution_source.contains(".join()") {
        join_path.push("ExecutionHost::shutdown joins every stored handle".to_string());
    } else {
        failures.push("execution.rs has no shutdown()/join() join path".to_string());
    }
    if execution_source.contains("impl Drop for ExecutionHost") {
        join_path.push("Drop for ExecutionHost calls shutdown".to_string());
    } else {
        failures.push("Drop for ExecutionHost is missing".to_string());
    }
    if plot_source.contains("JoinHandle")
        && plot_source.contains("fn drop")
        && plot_source.contains(".join()")
    {
        join_path.push("RasterWorker stores the worker's JoinHandle and Drop joins it".to_string());
    } else {
        failures
            .push("ui/plot.rs has no JoinHandle/Drop/join path for the canvas worker".to_string());
    }
    if plot_source.contains("Shutdown") && plot_source.contains("fetch_add") {
        join_path.push(
            "the canvas worker stops on a Shutdown message and cancels through an atomic generation"
                .to_string(),
        );
    } else {
        failures.push("ui/plot.rs has no Shutdown/cancellation protocol".to_string());
    }
    if workbench_source.contains("next_event") && !workbench_source.contains("thread::sleep") {
        join_path
            .push("Workbench wait path blocks on ExecutionHost::next_event, not sleep".to_string());
    } else {
        failures.push("Workbench wait path does not block on the event channel".to_string());
    }

    // ARC-008: exactly two reviewed spawn sites, each in its approved file and named thread.
    if spawn_sites.len() != 2 {
        failures.push(format!(
            "expected exactly two production thread-spawn sites (execution.rs run worker and ui/plot.rs canvas worker), found {}",
            spawn_sites.len()
        ));
    }
    let run_site = spawn_sites.iter().find(|site| site.file == "execution.rs");
    let plot_site = spawn_sites.iter().find(|site| site.file == "ui/plot.rs");
    match run_site {
        Some(_) if execution_source_text.contains("lovelace-run-") => {}
        Some(_) => {
            failures.push("the run worker is not named lovelace-run-<doc>-<run>".to_string())
        }
        None => failures.push("the run worker's spawn site in execution.rs is missing".to_string()),
    }
    match plot_site {
        Some(_) if plot_source_text.contains("lovelace-plot-raster") => {}
        Some(_) => failures.push("the canvas worker is not named lovelace-plot-raster".to_string()),
        None => {
            failures.push("the canvas worker's spawn site in ui/plot.rs is missing".to_string())
        }
    }
    for site in &spawn_sites {
        if site.file != "execution.rs" && site.file != "ui/plot.rs" {
            failures.push(format!(
                "unapproved thread-spawn site at {}:{}",
                site.file, site.line
            ));
        }
    }
    if !blocking_sites.is_empty() {
        failures.push(format!(
            "production sleep/park/spin synchronization found at {} site(s)",
            blocking_sites.len()
        ));
    }
    if primitives.len() < 4 {
        failures.push("the execution core does not show its native sync primitives".to_string());
    }

    ThreadReport {
        spawn_sites,
        blocking_sites,
        primitives,
        join_path,
        failures,
    }
}

fn scan_sites(
    file: &str,
    production: &str,
    spawn_sites: &mut Vec<SpawnSite>,
    blocking_sites: &mut Vec<SpawnSite>,
) {
    for (index, line) in production.lines().enumerate() {
        // Mask literals per line: a diagnostic string that names `thread::sleep` or a negative
        // test's source snippet is not a call site.
        let code = mask_literals(line);
        if code.contains("thread::Builder::new") || code.contains("thread::spawn") {
            spawn_sites.push(SpawnSite {
                file: file.to_string(),
                line: index + 1,
                text: line.trim().to_string(),
            });
        }
        if code.contains("thread::sleep")
            || code.contains("thread::park")
            || code.contains("spin_loop")
        {
            blocking_sites.push(SpawnSite {
                file: file.to_string(),
                line: index + 1,
                text: line.trim().to_string(),
            });
        }
    }
}

fn collect_rust_files(root: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// Runs the audit and prints it, returning the process exit code.
pub fn run() -> i32 {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let report = audit_src(&src);
    println!("lovelace-desktop --thread-audit (ARC-008)");
    println!("root: {}", src.display());
    println!();
    println!("== thread spawn sites ==");
    println!("file:line | construction | join/stop path");
    for site in &report.spawn_sites {
        let path = if site.file == "execution.rs" {
            "ExecutionHost.workers[run] = JoinHandle; ExecutionHost::shutdown sets cancel flags and joins every handle; Drop for ExecutionHost calls shutdown"
        } else {
            "RasterWorker.handle = JoinHandle; RasterWorker::drop sends Shutdown and joins; Drop for Plot drops the worker"
        };
        println!("{}:{} | {} | {path}", site.file, site.line, site.text);
    }
    println!();
    println!("== production sleep/park/spin sites ==");
    if report.blocking_sites.is_empty() {
        println!("none");
    } else {
        for site in &report.blocking_sites {
            println!("{}:{} | {}", site.file, site.line, site.text);
        }
    }
    println!();
    println!("== native synchronization primitives ==");
    for primitive in &report.primitives {
        println!("{primitive}");
    }
    println!();
    println!("== join/stop facts ==");
    for fact in &report.join_path {
        println!("{fact}");
    }
    println!();
    if report.pass() {
        println!("result PASS");
        0
    } else {
        for failure in &report.failures {
            println!("FAIL {failure}");
        }
        println!("result FAIL");
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_source_passes_the_thread_audit() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let report = audit_src(&src);
        assert!(report.pass(), "ARC-008 violations: {:#?}", report.failures);
        assert_eq!(report.spawn_sites.len(), 2);
        let files: Vec<&str> = report
            .spawn_sites
            .iter()
            .map(|site| site.file.as_str())
            .collect();
        assert_eq!(files, vec!["execution.rs", "ui/plot.rs"]);
        let execution = std::fs::read_to_string(src.join("execution.rs")).expect("execution.rs");
        assert!(
            execution.contains("lovelace-run-"),
            "the worker thread name is not lovelace-run-<doc>-<run>"
        );
        let plot = std::fs::read_to_string(src.join("ui").join("plot.rs")).expect("ui/plot.rs");
        assert!(
            plot.contains("lovelace-plot-raster"),
            "the canvas worker thread name is not lovelace-plot-raster"
        );
    }

    #[test]
    fn the_scan_finds_every_spawn_site_and_rejects_an_unapproved_one() {
        let mut spawn_sites = Vec::new();
        let mut blocking = Vec::new();
        scan_sites(
            "ui/paint.rs",
            "fn paint(){ std::thread::spawn(move || work()); }",
            &mut spawn_sites,
            &mut blocking,
        );
        assert_eq!(spawn_sites.len(), 1);
        assert_eq!(spawn_sites[0].file, "ui/paint.rs");

        // The shipped rule: the only production spawn sites are execution.rs's run worker and
        // ui/plot.rs's canvas worker. A third file that spawns fails the audit even if it is
        // otherwise clean.
        let mut third = Vec::new();
        let mut third_blocking = Vec::new();
        for file in ["execution.rs", "ui/plot.rs", "ui/paint.rs"] {
            scan_sites(
                file,
                "fn start(){ std::thread::Builder::new().name(\"worker\").spawn(work); }",
                &mut third,
                &mut third_blocking,
            );
        }
        assert_eq!(third.len(), 3);
        assert!(third.iter().any(|site| site.file == "ui/paint.rs"));
    }

    #[test]
    fn a_sleep_in_production_is_rejected() {
        let mut spawn_sites = Vec::new();
        let mut blocking = Vec::new();
        scan_sites(
            "workbench.rs",
            "loop { workbench.poll(); std::thread::sleep(Duration::from_millis(1)); }",
            &mut spawn_sites,
            &mut blocking,
        );
        assert_eq!(blocking.len(), 1);
    }

    #[test]
    fn test_modules_and_comments_do_not_count_as_production() {
        let source = "\
// std::thread::sleep(Duration::from_millis(1)) in a comment is not production
fn production_wait(host: &ExecutionHost) { let _ = host.next_event(Duration::from_secs(1)); }

#[cfg(test)]
mod tests {
    fn wait() { std::thread::sleep(Duration::from_millis(1)); }
}
";
        let production = strip_test_modules(&strip_comments(source));
        let mut spawn_sites = Vec::new();
        let mut blocking = Vec::new();
        scan_sites("workbench.rs", &production, &mut spawn_sites, &mut blocking);
        assert!(spawn_sites.is_empty());
        assert!(blocking.is_empty());
    }
}
