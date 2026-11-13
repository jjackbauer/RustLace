//! PLF-009: the unsupported-platform rule, in one place the binary and a test both read.
//!
//! PLF-009 asks for the opposite of a silent fallback: a platform without a UI adapter must be
//! *explicitly* marked unsupported by release metadata, and a build for it must say so instead of
//! shipping a headless placeholder. The release metadata is
//! `docs/goal-desktop-v4/platform-matrix.md`; this module reads it, refuses to accept a matrix row
//! that neither claims a built-and-run GUI nor says it is unsupported/deferred, and produces the
//! exact refusal text a non-Windows build prints.
//!
//! `--platform-claims` prints the matrix verdicts and the refusal, and exits non-zero when a row
//! is silent. The non-Windows `main` calls [`unsupported_notice`] for its own target, so the
//! message the product prints is the same value the check asserts.

/// The release matrix, resolved from the crate at build time.
///
/// `CARGO_MANIFEST_DIR` is `rust/lovelace-desktop`, so this is the repository's document wherever
/// the binary is later run from. The file is read, never written.
pub const MATRIX_PATH: &str = "../../docs/goal-desktop-v4/platform-matrix.md";

/// The platform this release is built for and certifies.
pub const ADVERTISED_PLATFORM: &str = "windows";

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
    /// True when the row claims a built-and-run GUI on that platform.
    pub fn claims_gui(&self) -> bool {
        self.status.to_ascii_uppercase().contains("INITIAL RELEASE")
            && !self.run.trim().eq_ignore_ascii_case("none")
    }

    /// True when the row explicitly refuses a support claim.
    pub fn is_explicitly_unsupported(&self) -> bool {
        let status = self.status.to_ascii_uppercase();
        status.contains("UNSUPPORTED") || status.contains("LATER PORT")
    }

    /// Whether this platform is the advertised one, by the OS word in its name.
    pub fn is_advertised(&self) -> bool {
        self.platform
            .to_ascii_lowercase()
            .contains(ADVERTISED_PLATFORM)
    }
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
        if cells.iter().all(|cell| cell.chars().all(|c| c == '-' || c == ':')) {
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
/// Returns `None` for the advertised platform: Windows has a UI adapter, so there is nothing to
/// refuse. Every other platform gets a message that names the platform, states that the workbench
/// is unsupported there, and says no headless placeholder is being started.
pub fn unsupported_notice(target_os: &str, arch: &str) -> Option<String> {
    if target_os.eq_ignore_ascii_case(ADVERTISED_PLATFORM) {
        return None;
    }
    Some(format!(
        "lovelace-desktop: the native workbench is unsupported on {target_os}/{arch}: this build \
         has no UI adapter for that platform (the supported platform for this release is Windows \
         11 x64). No headless placeholder is started; exiting."
    ))
}

/// Every problem the matrix has under PLF-009's rule, as printable lines.
pub fn matrix_problems(claims: &[Claim]) -> Vec<String> {
    let mut problems = Vec::new();
    if claims.is_empty() {
        problems.push("the platform matrix could not be parsed".to_string());
        return problems;
    }
    // Exactly one row may claim a built-and-run GUI, and it must be the advertised Windows row.
    let supported: Vec<&Claim> = claims.iter().filter(|claim| claim.claims_gui()).collect();
    if supported.len() != 1 {
        problems.push(format!(
            "expected exactly one platform with a built-and-run GUI claim, found {}",
            supported.len()
        ));
    } else if !supported[0].is_advertised() {
        problems.push(format!(
            "the supported row is not the Windows release: {:?}",
            supported[0].platform
        ));
    }
    for claim in claims {
        if claim.claims_gui() {
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

/// `--platform-claims`: the matrix verdicts, the refusal text for a deferred platform and a
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
    println!("platform | status | run evidence | verdict");
    for claim in &claims {
        let verdict = if claim.claims_gui() {
            "SUPPORTED (built + run GUI declared)"
        } else if claim.is_explicitly_unsupported() {
            "EXPLICITLY NOT SHIPPED"
        } else {
            "SILENT"
        };
        println!("{} | {} | {} | {}", claim.platform, claim.status, claim.run, verdict);
    }
    println!(
        "this build: target_os={} arch={} advertised={}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::env::consts::OS == ADVERTISED_PLATFORM
    );
    for (os, arch) in [("linux", "x86_64"), ("macos", "aarch64")] {
        match unsupported_notice(os, arch) {
            Some(notice) => println!("refusal[{os}/{arch}]: {notice}"),
            None => println!("refusal[{os}/{arch}]: NONE (would be a silent claim)"),
        }
    }
    if unsupported_notice(ADVERTISED_PLATFORM, std::env::consts::ARCH).is_some() {
        println!("FAIL Windows must not be refused by its own build");
        return 1;
    }
    let problems = matrix_problems(&claims);
    for problem in &problems {
        println!("problem: {problem}");
    }
    println!(
        "result {}: {} platform row(s), {} problem(s)",
        if problems.is_empty() { "PASS" } else { "FAIL" },
        claims.len(),
        problems.len()
    );
    if problems.is_empty() {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matrix() -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(MATRIX_PATH),
        )
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
    fn exactly_one_platform_is_the_certified_windows_release() {
        let claims = parse_matrix(&matrix());
        let supported: Vec<&Claim> = claims.iter().filter(|claim| claim.claims_gui()).collect();
        assert_eq!(supported.len(), 1);
        assert!(
            supported[0].is_advertised(),
            "the supported row must be the Windows release, got {:?}",
            supported[0].platform
        );
    }

    #[test]
    fn every_other_platform_gets_a_refusal_that_names_it() {
        for os in ["linux", "macos", "freebsd"] {
            let notice = unsupported_notice(os, "x86_64").expect("a non-Windows build must refuse");
            assert!(
                notice.contains(os),
                "the refusal must name the platform: {notice}"
            );
            assert!(
                notice.contains("unsupported") && notice.contains("No headless placeholder"),
                "the refusal must state the unsupported platform and refuse a placeholder: {notice}"
            );
        }
        assert_eq!(
            unsupported_notice(ADVERTISED_PLATFORM, "x86_64"),
            None,
            "Windows is the advertised platform, so it has nothing to refuse"
        );
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
    fn the_probe_reads_the_shipped_matrix_and_exits_zero() {
        assert_eq!(run(), 0, "--platform-claims must accept the shipped matrix");
    }
}
