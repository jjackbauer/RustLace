//! The CLI surface of a build without a UI adapter.
//!
//! The help text is a claim: every headless mode it lists must exist in this binary, and the
//! modes that only the Windows build compiles must not be advertised. `--version` is the
//! compiled package version, and an unknown flag is a usage error rather than a silent launch.

#![cfg(not(windows))]

use std::process::{Command, Output};

fn run(arg: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lovelace-desktop"))
        .arg(arg)
        .output()
        .expect("run the built lovelace-desktop binary")
}

#[test]
fn help_exits_zero_and_names_only_headless_modes_this_build_has() {
    let output = run("--help");
    assert!(output.status.success(), "--help exited {}", output.status);
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(!help.trim().is_empty(), "--help printed nothing on stdout");
    for mode in [
        "--selftest",
        "--gui-selftest",
        "--ui-audit",
        "--thread-audit",
        "--shortcuts",
        "--platform-claims",
    ] {
        assert!(help.contains(mode), "--help does not name {mode}:\n{help}");
    }
    for unavailable in [
        "--journey",
        "--raster-svg",
        "--legacy-win32",
        "--contrast-check",
    ] {
        assert!(
            !help.contains(unavailable),
            "--help claims {unavailable}, which this build does not compile:\n{help}"
        );
    }
}

#[test]
fn version_exits_zero_and_prints_the_package_version() {
    let output = run("--version");
    assert!(
        output.status.success(),
        "--version exited {}",
        output.status
    );
    let version = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        version.trim(),
        format!("lovelace-desktop {}", env!("CARGO_PKG_VERSION")),
        "--version must print the compiled package version"
    );
}

#[test]
fn an_unknown_flag_exits_two_with_usage_on_stderr() {
    let output = run("--definitely-not-a-mode");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown option --definitely-not-a-mode"),
        "stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("USAGE:"),
        "the usage follows the error:\n{stderr}"
    );
    assert!(
        output.stdout.is_empty(),
        "an error must not print to stdout: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
}
