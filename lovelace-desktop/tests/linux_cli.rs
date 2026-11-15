//! The CLI surface of the Linux build.
//!
//! The help text is a claim: every mode it lists must exist in this binary. Since round 1 of the
//! Linux GUI port the Linux build compiles the whole workbench, so the GUI, journey, plot-raster
//! and contrast modes are advertised. The three Win32-only modes (`--legacy-win32`,
//! `--ime-probe`, `--platform-parity`) are not compiled there and the build must name them as
//! requiring Windows and refuse them, never start a placeholder. `--version` is the compiled
//! package version, and an unknown flag is a usage error rather than a silent launch.

#![cfg(not(windows))]

use std::process::{Command, Output};

fn run(arg: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lovelace-desktop"))
        .arg(arg)
        .output()
        .expect("run the built lovelace-desktop binary")
}

#[test]
fn help_exits_zero_and_names_the_modes_this_build_compiles() {
    let output = run("--help");
    assert!(output.status.success(), "--help exited {}", output.status);
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(!help.trim().is_empty(), "--help printed nothing on stdout");
    // The modes the Linux build compiles, including the ones that used to be Windows-only.
    for mode in [
        "--selftest",
        "--gui-selftest",
        "--journey",
        "--raster-svg",
        "--contrast-check",
        "--ui-audit",
        "--thread-audit",
        "--shortcuts",
        "--platform-claims",
    ] {
        assert!(help.contains(mode), "--help does not name {mode}:\n{help}");
    }
    // The Win32-only modes are refused by name, so the help has to say why. A Linux build that
    // stayed silent about them would leave a user with an unexplained exit 2.
    assert!(
        help.contains("need the Windows build"),
        "--help must explain the Win32-only modes:\n{help}"
    );
    for unavailable in ["--legacy-win32", "--ime-probe", "--platform-parity"] {
        assert!(
            help.contains(unavailable),
            "--help must name {unavailable} as Windows-only:\n{help}"
        );
    }
}

#[test]
fn the_win32_only_modes_are_refused_by_name() {
    for mode in ["--legacy-win32", "--ime-probe", "--platform-parity"] {
        let output = run(mode);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{mode} must exit 2 on Linux:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(mode) && stderr.contains("requires the Windows build"),
            "{mode} must name itself and the reason:\n{stderr}"
        );
        assert!(
            output.stdout.is_empty(),
            "{mode} must not print to stdout: {:?}",
            String::from_utf8_lossy(&output.stdout)
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
