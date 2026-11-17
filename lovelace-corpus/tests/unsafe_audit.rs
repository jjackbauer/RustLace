//! `COR-015`: the structural half of the unsafe/FFI audit.
//!
//! This is a source-level test, not a runtime FFI test: it proves that every crate that can
//! forbid unsafe does, the two Windows lanes are the only files containing unsafe code, and both
//! lanes carry SAFETY comments. The runtime invalid-handle tests live beside the wrappers in
//! `lovelace-desktop/src/win32/ffi.rs`; `--gui-selftest` exercises the real window path.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn collect_rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|error| panic!("read {}: {error}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if name == "target" || name.starts_with('.') {
                continue;
            }
            collect_rust_sources(&path, out);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn code_has_unsafe(text: &str) -> bool {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .any(|line| {
            line.contains("unsafe {")
                || line.contains("unsafe extern")
                || line.contains("unsafe fn")
        })
}

#[test]
fn unsafe_is_confined_to_the_documented_windows_lanes() {
    let root = repo_root();
    let mut sources = Vec::new();
    for entry in std::fs::read_dir(&root).expect("repository root") {
        let path = entry.expect("directory entry").path();
        if path.is_dir()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("lovelace-") && name != "lovelace-desktop-spike"
                })
        {
            let src = path.join("src");
            if src.is_dir() {
                collect_rust_sources(&src, &mut sources);
            }
        }
    }
    assert!(
        sources.len() > 100,
        "the audit found only {} sources",
        sources.len()
    );

    let mut offenders = Vec::new();
    let mut safety_files = Vec::new();
    for path in &sources {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        if !code_has_unsafe(&text) {
            continue;
        }
        let relative = path.strip_prefix(&root).unwrap_or(path);
        let relative = relative.to_string_lossy().replace('\\', "/");
        let allowed = relative.starts_with("lovelace-desktop/src/win32/")
            || relative == "lovelace-run/src/repl.rs";
        if !allowed {
            offenders.push(relative);
        } else if !text.contains("// SAFETY:") {
            safety_files.push(relative);
        }
    }
    assert!(
        offenders.is_empty(),
        "unsafe outside the documented lanes: {offenders:?}"
    );
    assert!(
        safety_files.is_empty(),
        "unsafe lane without a SAFETY comment: {safety_files:?}"
    );
}

#[test]
fn every_engine_crate_that_can_forbid_unsafe_does() {
    let root = repo_root();
    let forbid = [
        "lovelace-array/src/lib.rs",
        "lovelace-int/src/lib.rs",
        "lovelace-know/src/lib.rs",
        "lovelace-systems/src/lib.rs",
        "lovelace-json/src/lib.rs",
        "lovelace-nat/src/lib.rs",
        "lovelace-real/src/lib.rs",
        "lovelace-symbolics/src/lib.rs",
        "lovelace-lang/src/lib.rs",
        "lovelace-corpus/src/lib.rs",
        "lovelace-bench/src/main.rs",
    ];
    for relative in forbid {
        let path = root.join(relative);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        assert!(
            text.contains("#![forbid(unsafe_code)]"),
            "{relative} does not forbid unsafe code"
        );
    }

    let run = std::fs::read_to_string(root.join("lovelace-run/src/main.rs")).expect("run main");
    assert!(run.contains("#![deny(unsafe_code)]"));
    let desktop =
        std::fs::read_to_string(root.join("lovelace-desktop/src/main.rs")).expect("desktop main");
    assert!(desktop.contains("forbid(unsafe_code)"));
}
