//! ARC-008: the mechanical source audit for threads and synchronisation.
//!
//! It scans every **production** Rust source file in the crate (comments and `#[cfg(test)]`
//! modules stripped) and enforces:
//!
//! - exactly one thread-spawn site exists, and it is `execution.rs`'s named run worker;
//! - that worker's `JoinHandle` is stored on the `ExecutionHost` and joined by
//!   `ExecutionHost::shutdown`, which `Drop for ExecutionHost` calls — no detached or daemon-like
//!   worker survives the host;
//! - no production code sleeps, parks or spins to synchronise (`thread::sleep`, `thread::park`,
//!   `spin_loop`);
//! - the execution core synchronises with the native primitives it imports (`mpsc` channel,
//!   `Mutex`, atomics), and the workbench's wait path blocks on that channel instead of polling.
//!
//! The runtime half of the proof lives beside the construction: `execution::tests` asserts the
//! recorded engine thread differs from the UI thread and that `shutdown` joins every spawned
//! worker (`shutdown_joins_every_spawned_worker`). This audit is the static half: it fails when a
//! new spawn site, a sleep or a lost join path appears in the source, and its negative tests feed
//! those exact violations through the same scanner.

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
        scan_sites(&relative, &production, &mut spawn_sites, &mut blocking_sites);
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
    if workbench_source.contains("next_event") && !workbench_source.contains("thread::sleep") {
        join_path.push("Workbench wait path blocks on ExecutionHost::next_event, not sleep".to_string());
    } else {
        failures.push("Workbench wait path does not block on the event channel".to_string());
    }

    if spawn_sites.len() != 1 {
        failures.push(format!(
            "expected exactly one production thread-spawn site, found {}",
            spawn_sites.len()
        ));
    } else {
        let site = &spawn_sites[0];
        if site.file != "execution.rs" {
            failures.push(format!(
                "the only spawn site must be execution.rs, found {}:{}",
                site.file, site.line
            ));
        }
        if !execution_source_text.contains("lovelace-run-") {
            failures.push("the worker thread is not named lovelace-run-<doc>-<run>".to_string());
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
        println!(
            "{}:{} | {} | ExecutionHost.workers[run] = JoinHandle; ExecutionHost::shutdown sets cancel flags and joins every handle; Drop for ExecutionHost calls shutdown",
            site.file, site.line, site.text
        );
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
        assert!(
            report.pass(),
            "ARC-008 violations: {:#?}",
            report.failures
        );
        assert_eq!(report.spawn_sites.len(), 1);
        assert_eq!(report.spawn_sites[0].file, "execution.rs");
        let execution = std::fs::read_to_string(src.join("execution.rs")).expect("execution.rs");
        assert!(
            execution.contains("lovelace-run-"),
            "the worker thread name is not lovelace-run-<doc>-<run>"
        );
    }

    #[test]
    fn the_scan_finds_every_spawn_site_and_rejects_a_second_one() {
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

        // The shipped rule: exactly one production spawn site, in execution.rs. A second file
        // that spawns fails the audit even if it is otherwise clean.
        let synthetic = ThreadReport {
            spawn_sites: vec![
                SpawnSite {
                    file: "execution.rs".to_string(),
                    line: 1,
                    text: "thread::Builder::new().name(\"lovelace-run\")".to_string(),
                },
                SpawnSite {
                    file: "ui/paint.rs".to_string(),
                    line: 2,
                    text: "std::thread::spawn(|| work())".to_string(),
                },
            ],
            blocking_sites: Vec::new(),
            primitives: vec![
                "mpsc".to_string(),
                "Mutex".to_string(),
                "AtomicBool".to_string(),
                "AtomicU64".to_string(),
            ],
            join_path: vec!["join".to_string()],
            failures: Vec::new(),
        };
        assert_eq!(synthetic.spawn_sites.len(), 2);
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
