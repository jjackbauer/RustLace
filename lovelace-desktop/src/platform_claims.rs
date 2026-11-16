//! PLF-009: the supported-platform rule, in one place the binary and a test both read.
//!
//! PLF-009 asks for the opposite of a silent fallback: a platform without a UI adapter must be
//! *explicitly* marked unsupported by release metadata, and a build for it must say so instead of
//! shipping a headless placeholder. The release metadata is
//! `docs/goal-desktop-v4/platform-matrix.md`; this module reads it, refuses to accept a matrix row
//! that neither claims a built-and-run GUI nor says it is unsupported/deferred, and produces the
//! exact refusal text a build without a UI adapter prints.
//!
//! The supported set is not a Windows-only assumption: Windows x86_64 and Linux x86_64 (X11) both
//! ship a native workbench, so both are accepted here and neither is refused. macOS (and any other
//! OS) has no UI adapter and gets the refusal.
//!
//! `--platform-claims` prints the matrix verdicts and the refusal, and exits non-zero when a row
//! is silent. [`unsupported_notice`] is the single refusal text for a platform with no UI adapter,
//! so the probe and the product cannot drift.

/// The release matrix, resolved from the crate at build time.
///
/// `CARGO_MANIFEST_DIR` is `lovelace-desktop`, so this is the repository's document wherever
/// the binary is later run from. The file is read, never written.
pub const MATRIX_PATH: &str = "../docs/goal-desktop-v4/platform-matrix.md";

/// The OS words of the platforms this release is built for and certifies.
///
/// Windows x86_64 and Linux x86_64 (X11) are the two supported platforms (ADR-008). The matrix
/// must carry a built-and-run GUI row for each; [`unsupported_notice`] refuses every other OS.
pub const SUPPORTED_PLATFORMS: [&str; 2] = ["windows", "linux"];

/// One parsed matrix row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claim {
    /// The platform cell, verbatim.
    pub platform: String,
    /// The status cell, verbatim (for example `INITIAL RELEASE` or `UNSUPPORTED (untested)`).
    pub status: String,
    /// The run-evidence cell, verbatim (`none` when nothing was run).
    pub run: String,
}

impl Claim {
    /// True when the row's status is a positive support claim rather than a deferral or refusal.
    ///
    /// `UNSUPPORTED` contains the word `SUPPORTED`, so the explicit-refusal check comes first.
    pub fn claims_support(&self) -> bool {
        if self.is_explicitly_unsupported() {
            return false;
        }
        let status = self.status.to_ascii_uppercase();
        status.contains("SUPPORTED") || status.contains("INITIAL RELEASE")
    }

    /// True when the row claims a built-and-run GUI on that platform.
    pub fn claims_gui(&self) -> bool {
        self.claims_support() && !self.run.trim().eq_ignore_ascii_case("none")
    }

    /// True when the row explicitly refuses a support claim.
    pub fn is_explicitly_unsupported(&self) -> bool {
        let status = self.status.to_ascii_uppercase();
        status.contains("UNSUPPORTED") || status.contains("LATER PORT")
    }

    /// Whether this platform is one of the advertised release platforms, by the OS word in its name.
    pub fn is_advertised(&self) -> bool {
        let platform = self.platform.to_ascii_lowercase();
        SUPPORTED_PLATFORMS
            .iter()
            .any(|supported| platform.contains(supported))
    }
}

/// Whether `target_os` is one of the platforms this release supports.
pub fn is_advertised_os(target_os: &str) -> bool {
    SUPPORTED_PLATFORMS
        .iter()
        .any(|supported| target_os.eq_ignore_ascii_case(supported))
}

/// Parses the matrix table's data rows (the header and separator are skipped).
///
/// The parse is strict about the shape it consumes: a row is used only when it has five cells and
/// the table was found by its header, so a prose paragraph cannot be mistaken for a claim.
pub fn parse_matrix(text: &str) -> Vec<Claim> {
    let mut claims = Vec::new();
    let mut in_table = false;
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            if in_table && !claims.is_empty() {
                break;
            }
            continue;
        }
        let cells: Vec<&str> = line
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim())
            .collect();
        if !in_table {
            if cells.len() == 5 && cells[0] == "Platform" && cells[1] == "Status for this release" {
                in_table = true;
            }
            continue;
        }
        if cells
            .iter()
            .all(|cell| cell.chars().all(|c| c == '-' || c == ':'))
        {
            continue;
        }
        if cells.len() != 5 {
            continue;
        }
        claims.push(Claim {
            platform: cells[0].to_string(),
            status: cells[1].to_string(),
            run: cells[3].to_string(),
        });
    }
    claims
}

/// The refusal a build for `target_os`/`arch` prints instead of opening a UI it does not have.
///
/// Returns `None` for every supported platform: Windows and Linux each have a UI adapter, so there
/// is nothing to refuse. Every other platform gets a message that names the platform, states that
/// the workbench is unsupported there, names both supported platforms, and says no headless
/// placeholder is being started.
pub fn unsupported_notice(target_os: &str, arch: &str) -> Option<String> {
    if is_advertised_os(target_os) {
        return None;
    }
    Some(format!(
        "lovelace-desktop: the native workbench is unsupported on {target_os}/{arch}: this build \
         has no UI adapter for that platform (the supported platforms for this release are Windows \
         11 x64 and Linux x86_64 (X11)). No headless placeholder is started; exiting."
    ))
}

/// Every problem the matrix has under PLF-009's rule, as printable lines.
pub fn matrix_problems(claims: &[Claim]) -> Vec<String> {
    let mut problems = Vec::new();
    if claims.is_empty() {
        problems.push("the platform matrix could not be parsed".to_string());
        return problems;
    }
    // More than one row may claim support now: both supported platforms must, each with run
    // evidence. A supported row with `none` evidence is the silent claim PLF-009 forbids.
    for claim in claims.iter().filter(|claim| claim.claims_support()) {
        if claim.run.trim().eq_ignore_ascii_case("none") {
            problems.push(format!(
                "{} claims support but has no run evidence: status={:?}",
                claim.platform, claim.status
            ));
        } else if !claim.is_advertised() {
            problems.push(format!(
                "{} claims support but is not one of the supported platforms {:?}",
                claim.platform, SUPPORTED_PLATFORMS
            ));
        }
    }
    for supported in SUPPORTED_PLATFORMS {
        let present = claims.iter().any(|claim| {
            claim.claims_gui() && claim.platform.to_ascii_lowercase().contains(supported)
        });
        if !present {
            problems.push(format!(
                "the release matrix has no built-and-run GUI row for {supported}/x86_64"
            ));
        }
    }
    for claim in claims {
        if claim.claims_support() {
            continue;
        }
        if !claim.is_explicitly_unsupported() {
            problems.push(format!(
                "{} is neither supported with run evidence nor explicitly unsupported: status={:?}",
                claim.platform, claim.status
            ));
        }
    }
    problems
}

/// Everything the probe reports as a problem, including the code invariant that a supported
/// platform must never be refused by the same module that advertises it.
fn release_problems(claims: &[Claim], arch: &str) -> Vec<String> {
    let mut problems = matrix_problems(claims);
    for supported in SUPPORTED_PLATFORMS {
        if unsupported_notice(supported, arch).is_some() {
            problems.push(format!(
                "{supported} must not be refused by its own build ({arch})"
            ));
        }
    }
    problems
}

/// The body of `--platform-claims` as printable lines, plus the problems that decide its exit code.
///
/// Kept separate from [`run`] so a test can read the exact lines a Linux build prints.
pub fn probe_report(os: &str, arch: &str, claims: &[Claim]) -> (Vec<String>, Vec<String>) {
    let mut lines = Vec::new();
    lines.push("platform | status | run evidence | verdict".to_string());
    for claim in claims {
        let verdict = if claim.claims_gui() {
            "SUPPORTED (built + run GUI declared)"
        } else if claim.claims_support() {
            "SUPPORTED (no run evidence)"
        } else if claim.is_explicitly_unsupported() {
            "EXPLICITLY NOT SHIPPED"
        } else {
            "SILENT"
        };
        lines.push(format!(
            "{} | {} | {} | {}",
            claim.platform, claim.status, claim.run, verdict
        ));
    }
    lines.push(format!(
        "this build: target_os={os} arch={arch} advertised={}",
        is_advertised_os(os)
    ));
    for (probe_os, probe_arch) in [("linux", "x86_64"), ("macos", "aarch64")] {
        match unsupported_notice(probe_os, probe_arch) {
            Some(notice) => lines.push(format!("refusal[{probe_os}/{probe_arch}]: {notice}")),
            None => lines.push(format!(
                "supported[{probe_os}/{probe_arch}]: this release has a UI adapter for it"
            )),
        }
    }
    let problems = release_problems(claims, arch);
    for problem in &problems {
        lines.push(format!("problem: {problem}"));
    }
    lines.push(format!(
        "result {}: {} platform row(s), {} problem(s)",
        if problems.is_empty() { "PASS" } else { "FAIL" },
        claims.len(),
        problems.len()
    ));
    (lines, problems)
}

/// `--platform-claims`: the matrix verdicts, the refusal text for an unsupported platform and a
/// non-zero exit when a row is silent.
pub fn run() -> i32 {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(MATRIX_PATH);
    println!("lovelace-desktop platform claims (PLF-009)");
    println!("release metadata: {}", path.display());
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            println!("FAIL cannot read the release matrix: {error}");
            return 1;
        }
    };
    let claims = parse_matrix(&text);
    let (lines, problems) = probe_report(std::env::consts::OS, std::env::consts::ARCH, &claims);
    for line in &lines {
        println!("{line}");
    }
    if problems.is_empty() { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matrix() -> String {
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(MATRIX_PATH))
            .expect("the release matrix must be readable")
    }

    #[test]
    fn the_release_matrix_marks_every_unshipped_platform_explicitly() {
        let claims = parse_matrix(&matrix());
        assert!(
            claims.len() >= 3,
            "expected at least Windows, Linux and macOS rows, parsed {:?}",
            claims
        );
        assert_eq!(
            matrix_problems(&claims),
            Vec::<String>::new(),
            "PLF-009: every platform row must be supported-with-run-evidence or explicitly refused"
        );
    }

    #[test]
    fn windows_and_linux_are_both_the_certified_release() {
        let claims = parse_matrix(&matrix());
        let supported: Vec<&Claim> = claims.iter().filter(|claim| claim.claims_gui()).collect();
        assert_eq!(
            supported.len(),
            2,
            "Windows x86_64 and Linux x86_64 (X11) both claim a built-and-run GUI, got {:?}",
            supported
        );
        assert!(
            supported
                .iter()
                .any(|claim| claim.platform.to_ascii_lowercase().contains("windows")),
            "the Windows row must be supported: {:?}",
            supported
        );
        assert!(
            supported
                .iter()
                .any(|claim| claim.platform.to_ascii_lowercase().contains("linux")),
            "the Linux row must be supported: {:?}",
            supported
        );
    }

    #[test]
    fn only_platforms_without_a_gui_adapter_are_refused() {
        for os in ["macos", "freebsd"] {
            let notice = unsupported_notice(os, "x86_64")
                .expect("a platform without a UI adapter must refuse");
            assert!(
                notice.contains(os),
                "the refusal must name the platform: {notice}"
            );
            assert!(
                notice.contains("unsupported") && notice.contains("No headless placeholder"),
                "the refusal must state the unsupported platform and refuse a placeholder: {notice}"
            );
            assert!(
                notice.contains("Windows 11 x64"),
                "the refusal must name Windows as one of the supported platforms: {notice}"
            );
            assert!(
                notice.contains("Linux x86_64"),
                "the refusal must name Linux as one of the supported platforms: {notice}"
            );
        }
        for os in ["windows", "linux"] {
            assert_eq!(
                unsupported_notice(os, "x86_64"),
                None,
                "{os} has a UI adapter, so it has nothing to refuse"
            );
        }
    }

    #[test]
    fn a_silent_row_is_reported_rather_than_accepted() {
        let silent = vec![
            Claim {
                platform: "Windows 11 x64".to_string(),
                status: "INITIAL RELEASE".to_string(),
                run: "required: native window launched".to_string(),
            },
            Claim {
                platform: "Linux x86_64".to_string(),
                status: "SHOULD WORK".to_string(),
                run: "none".to_string(),
            },
        ];
        let problems = matrix_problems(&silent);
        assert!(
            problems.iter().any(|problem| problem.contains("Linux")),
            "a row that neither claims a GUI nor says unsupported must fail: {problems:?}"
        );
    }

    #[test]
    fn more_than_one_supported_row_is_accepted() {
        let supported = vec![
            Claim {
                platform: "Windows 11 x64 (10.0.22631 or later)".to_string(),
                status: "**INITIAL RELEASE**".to_string(),
                run: "required: native window launched, screenshot captured, GUI suite executed"
                    .to_string(),
            },
            Claim {
                platform: "Linux x86_64".to_string(),
                status: "**SUPPORTED (X11)**".to_string(),
                run: "CI: `--gui-selftest` 20 checks under xvfb-run".to_string(),
            },
        ];
        assert_eq!(
            matrix_problems(&supported),
            Vec::<String>::new(),
            "two supported rows are both accepted when each carries run evidence"
        );
    }

    #[test]
    fn a_supported_row_without_run_evidence_is_still_rejected() {
        let claims = vec![
            Claim {
                platform: "Windows 11 x64".to_string(),
                status: "**INITIAL RELEASE**".to_string(),
                run: "required: native window launched".to_string(),
            },
            Claim {
                platform: "Linux x86_64".to_string(),
                status: "**SUPPORTED (X11)**".to_string(),
                run: "none".to_string(),
            },
        ];
        let problems = matrix_problems(&claims);
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("Linux") && problem.contains("no run evidence")),
            "a supported row with no run evidence must be reported: {problems:?}"
        );
    }

    #[test]
    fn an_empty_matrix_is_reported_rather_than_accepted() {
        assert_eq!(
            matrix_problems(&[]),
            vec!["the platform matrix could not be parsed".to_string()]
        );
    }

    #[test]
    fn the_linux_row_is_advertised_by_the_release_matrix() {
        let claims = parse_matrix(&matrix());
        let linux = claims
            .iter()
            .find(|claim| claim.platform.to_ascii_lowercase().contains("linux"))
            .expect("the matrix must carry a Linux row");
        assert!(
            linux.is_advertised(),
            "the Linux row must be advertised by this release: {linux:?}"
        );
        assert!(
            linux.claims_gui(),
            "the Linux row must claim a built-and-run GUI: {linux:?}"
        );
    }

    #[test]
    fn the_linux_probe_advertises_linux_and_never_refuses_it() {
        let claims = parse_matrix(&matrix());
        let (lines, problems) = probe_report("linux", "x86_64", &claims);
        let report = lines.join("\n");
        assert!(
            problems.is_empty(),
            "the shipped matrix must have no problems: {problems:?}"
        );
        assert!(
            report.contains("this build: target_os=linux arch=x86_64 advertised=true"),
            "a Linux build must advertise itself: {report}"
        );
        assert!(
            !report.contains("refusal[linux/x86_64]"),
            "Linux is supported, so it is never refused: {report}"
        );
        assert!(
            report.contains("supported[linux/x86_64]"),
            "the Linux probe must be reported as supported: {report}"
        );
        assert!(
            report.contains("result PASS"),
            "the shipped matrix must pass: {report}"
        );
    }

    #[test]
    fn the_probe_reads_the_shipped_matrix_and_exits_zero() {
        assert_eq!(run(), 0, "--platform-claims must accept the shipped matrix");
    }
}
