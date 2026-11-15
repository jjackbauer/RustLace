//! Lovelace Desktop: a native Windows IDE for the Lovelace scripting language.
//!
//! The binary has four modes:
//!
//! - Default: the new native workbench. An eframe/egui 0.37 window with a virtualised editor,
//!   a workspace pane fed by the real engine's snapshots, a logs pane fed by the typed
//!   `ExecutionHost` event stream, an SVG plot pane fed by the engine's `PlotCapture`, tabs and
//!   a status strip. Scripts are evaluated on a worker thread; the window never blocks.
//! - --gui-selftest: the headless acceptance mode for the same workbench. It drives the real
//!   engine through `ExecutionHost` exactly as the window does, prints one line per check and
//!   exits non-zero on failure.
//! - --journey <ID>: the seven implemented journeys (J-02, J-03, J-07, J-08, J-09, J-10,
//!   J-15), executed headlessly through the same workbench/`ExecutionHost` path and the same plot
//!   pane. It prints the observed values and exits non-zero when the expected observable in
//!   `docs/goal-desktop-v4/journeys.md` does not match.
//! - --selftest: the pre-cutover headless acceptance checks for the application core. Kept
//!   working on purpose; it is not the shipped UI.
//! - --raster-svg: the round-29 fidelity harness entry point. It rasterises one SVG file through
//!   the pane's exact resvg path (system fonts, PLT-002 resolver) to a PNG, so a reference
//!   rasteriser can be compared against the product rather than against a look-alike.
//! - --contrast-check: VIS-009. Prints every dark/light token pair's WCAG contrast ratio, the
//!   threshold it must reach and its pass/exempt status, and exits non-zero when normal text is
//!   below 4.5:1 or large text/non-text UI indicators are below 3:1. The ratios are computed
//!   from the rendered `Color32` tokens, not from a table in the docs.
//! - --legacy-win32: the old Win32 EDIT-control shell, reachable only through this explicit
//!   argument. It is no longer the default UI.
//!
//! Safety: unsafe code appears only in src/win32, and only as (a) "unsafe extern system" FFI
//! declarations whose signatures mirror the Windows headers and (b) the callbacks Win32 invokes.
//! Every unsafe block in that module carries a SAFETY comment. The workbench, execution core,
//! settings, SVG reader, PE-import scanner and both selftests contain no unsafe code, which the
//! crate-level cfg_attr below enforces on non-Windows builds.

#![cfg_attr(not(windows), forbid(unsafe_code))]

mod app;
#[cfg(test)]
mod cancel_parity;
mod completion;
#[cfg(windows)]
mod diagnostic_map;
mod document_io;
mod engine;
mod execution;
mod find;
#[cfg(windows)]
mod frame_probe;
#[cfg(windows)]
mod glyph_audit;
mod glyphs;
mod gui_selftest;
mod history;
#[cfg(windows)]
mod ime_probe;
mod incremental;
mod inspector;
#[cfg(windows)]
mod journey;
mod keyboard;
mod language;
mod log_kind;
mod netcheck;
#[cfg(windows)]
mod perf_probe;
mod platform_claims;
#[cfg(windows)]
mod platform_dialogs;
#[cfg(windows)]
mod platform_parity;
mod precision_budget;
#[cfg(test)]
mod precision_default;
mod quick_eval;
mod recovery;
mod regex_lite;
mod results_tree;
mod run_state;
mod run_trace;
mod selftest;
mod settings;
mod statements;
mod svg;
mod symbolic_status;
mod thread_audit;
#[cfg(windows)]
mod ui;
mod ui_audit;
#[cfg(windows)]
mod win32;
mod workbench;
mod workspace_table;

#[cfg(windows)]
fn main() {
    // EDT-016: the measurement window reports the open time from this point, so the number
    // includes process entry and the native window's creation, not only the app constructor.
    let started = std::time::Instant::now();
    let args: Vec<String> = std::env::args().skip(1).collect();

    // VIS-009: the WCAG contrast checker. It computes every ratio from the rendered token values
    // (the same `Color32`s the paint routines use), prints each pair and exits non-zero when a
    // normal-text pair is below 4.5:1 or a large-text/non-text pair is below 3:1.
    if args.iter().any(|arg| arg == "--contrast-check") {
        std::process::exit(ui::theme::contrast_cli());
    }
    // VIS-004/VIS-005: the type scale and legibility report. It lays out one sample per role at
    // 96 DPI, measures the rendered row height, and prints the size, line ratio and WCAG contrast
    // of every role on the surfaces it paints on; it exits non-zero when a role is below 11 pt,
    // below a 1.2x line ratio, or below 4.5:1.
    if args.iter().any(|arg| arg == "--legibility-check") {
        std::process::exit(ui::type_scale::legibility_cli());
    }
    // VIS-004/006/007/008: the design-system tables (type scale, control states, semantic roles
    // and carriers, empty states), printed from the same constants and constructors the window
    // paints with.
    if args.iter().any(|arg| arg == "--visual-audit") {
        std::process::exit(ui::visual_audit_cli());
    }
    // A11Y-014: the no-tofu report for the mathematical glyph set the engine emits. It measures
    // every character against the shipped font stack and exits non-zero when one is neither
    // drawable nor documented as a fallback.
    if args.iter().any(|arg| arg == "--glyph-check") {
        std::process::exit(glyph_audit::run());
    }
    // ARC-003: the source audit over src/ui. It prints every file, its imports and the exact
    // separation rule, and exits non-zero when a UI file evaluates, constructs an engine or
    // names the compiler directly.
    if args.iter().any(|arg| arg == "--ui-audit") {
        std::process::exit(ui_audit::run());
    }
    // ARC-008: the source audit over every thread spawn, sleep and synchronisation primitive.
    if args.iter().any(|arg| arg == "--thread-audit") {
        std::process::exit(thread_audit::run());
    }
    // A11Y-007: the keyboard map, printed from the one binding table the window and the
    // Help > Keyboard Shortcuts dialog read. This is the documented map; it cannot drift from
    // the registration because it is a projection of it.
    if args.iter().any(|arg| arg == "--shortcuts") {
        std::process::exit(shortcuts_cli());
    }
    if args.iter().any(|arg| arg == "--gui-selftest") {
        std::process::exit(gui_selftest::run());
    }
    // PLF-010: the platform-specific behaviour report (modifiers, dialogs, clipboard, caret/IME,
    // DPI) for this advertised platform. Exits non-zero when an area fails.
    if args.iter().any(|arg| arg == "--platform-parity") {
        std::process::exit(platform_parity::run());
    }
    // PLF-009: the release matrix's platform claims and the refusal a non-Windows build prints.
    // It reads a document, so it runs on every build.
    if args.iter().any(|arg| arg == "--platform-claims") {
        std::process::exit(platform_claims::run());
    }
    // Round-68/69 (EDT-017 / A11Y-008): the platform IME probe. It opens the shipped workbench
    // window and drives imm32 and the WM_IME_* messages on the window's own thread, because an
    // HIMC is per-process. The default launch path never constructs it.
    if let Some(index) = args.iter().position(|arg| arg == "--ime-probe") {
        let mode = args.get(index + 1).cloned().unwrap_or_default();
        let document = args.get(index + 2).map(std::path::PathBuf::from);
        let report = args
            .get(index + 3)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("ime-probe.log"));
        std::process::exit(ime_probe::run(&mode, document, report));
    }
    if let Some(index) = args.iter().position(|arg| arg == "--journey") {
        std::process::exit(journey::run(&args[index + 1..]));
    }
    if let Some(index) = args.iter().position(|arg| arg == "--raster-svg") {
        std::process::exit(ui::plot::raster_cli(&args[index + 1..]));
    }
    // Diagnostic: the cadence of a bare eframe window on this machine, so a frame-pacing result
    // can be attributed between the app's paint and the windowing stack. No PERF row uses it.
    if args.iter().any(|arg| arg == "--perf-frame-probe") {
        let out = args
            .iter()
            .position(|arg| arg == "--measure-out")
            .and_then(|index| args.get(index + 1))
            .map(std::path::PathBuf::from);
        let mode = if args.iter().any(|arg| arg == "--probe-request=after16") {
            frame_probe::RequestMode::After16
        } else {
            frame_probe::RequestMode::Immediate
        };
        std::process::exit(frame_probe::run(out, mode));
    }
    if let Some(index) = args.iter().position(|arg| arg == "--measure-large") {
        let script = args
            .get(index + 1)
            .filter(|arg| !arg.starts_with("--"))
            .map(std::path::PathBuf::from);
        let out = args
            .iter()
            .position(|arg| arg == "--measure-out")
            .and_then(|index| args.get(index + 1))
            .map(std::path::PathBuf::from);
        std::process::exit(ui::measure::run(script, out, started));
    }
    if let Some(index) = args.iter().position(|arg| arg == "--measure-run") {
        let script = args
            .get(index + 1)
            .filter(|arg| !arg.starts_with("--"))
            .map(std::path::PathBuf::from);
        let out = args
            .iter()
            .position(|arg| arg == "--measure-out")
            .and_then(|index| args.get(index + 1))
            .map(std::path::PathBuf::from);
        std::process::exit(ui::measure_run::run(script, out));
    }
    // PERF-001: one real-window launch measured from process entry to the first completed pass.
    // The perf-check harness runs this once per cold and per warm sample.
    if let Some(index) = args.iter().position(|arg| arg == "--measure-startup") {
        let script = args
            .get(index + 1)
            .filter(|arg| !arg.starts_with("--"))
            .map(std::path::PathBuf::from);
        let out = args
            .iter()
            .position(|arg| arg == "--measure-out")
            .and_then(|index| args.get(index + 1))
            .map(std::path::PathBuf::from);
        std::process::exit(ui::measure::run_startup(script, out, started));
    }
    // PERF-009: 10 000 typed characters through the shipped editor buffer, in release.
    if let Some(index) = args.iter().position(|arg| arg == "--measure-edit-scale") {
        let script = args
            .get(index + 1)
            .filter(|arg| !arg.starts_with("--"))
            .map(std::path::PathBuf::from);
        let out = args
            .iter()
            .position(|arg| arg == "--measure-out")
            .and_then(|index| args.get(index + 1))
            .map(std::path::PathBuf::from);
        let chars = args
            .iter()
            .position(|arg| arg == "--chars")
            .and_then(|index| args.get(index + 1))
            .and_then(|value| value.parse::<usize>().ok());
        std::process::exit(perf_probe::edit_scale_cli(script, out, chars));
    }
    // PERF-012: an invalid plot must not corrupt the document/engine; the next run must work.
    if args.iter().any(|arg| arg == "--perf-paint-failure") {
        std::process::exit(perf_probe::paint_failure_cli());
    }
    // PERF-011: what the product does with an extreme precision request. Exits non-zero when no
    // estimate/warning/refusal exists, which is the capability the row requires.
    if args.iter().any(|arg| arg == "--perf-precision-probe") {
        std::process::exit(perf_probe::precision_probe_cli());
    }
    if args.iter().any(|arg| arg == "--selftest") {
        std::process::exit(selftest::run());
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print!("{}", usage());
        return;
    }
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("lovelace-desktop {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|arg| arg == "--legacy-win32") {
        let initial = args.iter().find(|arg| !arg.starts_with("--")).cloned();
        std::process::exit(win32::shell::run(initial.as_deref()));
    }

    let unknown: Vec<&String> = args.iter().filter(|arg| arg.starts_with("--")).collect();
    if !unknown.is_empty() {
        eprintln!("unknown option {}\n\n{}", unknown[0], usage());
        std::process::exit(2);
    }
    let initial = args.iter().find(|arg| !arg.starts_with("--")).cloned();
    // Printed at startup so a launch with redirected stderr shows where the engine will write.
    // It lives here rather than in the UI module so `src/ui/**` names no engine function at all.
    eprintln!(
        "lovelace-desktop: plot directory {}",
        engine::default_plot_dir().display()
    );
    std::process::exit(ui::run(initial.map(std::path::PathBuf::from)));
}

#[cfg(not(windows))]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print!("{}", usage());
        return;
    }
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("lovelace-desktop {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|arg| arg == "--gui-selftest") {
        std::process::exit(gui_selftest::run());
    }
    if args.iter().any(|arg| arg == "--journey") {
        eprintln!("--journey requires the Windows build");
        std::process::exit(2);
    }
    if args.iter().any(|arg| arg == "--measure-large") {
        eprintln!("--measure-large requires the Windows build");
        std::process::exit(2);
    }
    if args.iter().any(|arg| arg == "--selftest") {
        std::process::exit(selftest::run());
    }
    // ARC-003 / ARC-008: the two source audits are platform-independent and read the shipped
    // source tree, so they print on every build.
    if args.iter().any(|arg| arg == "--ui-audit") {
        std::process::exit(ui_audit::run());
    }
    if args.iter().any(|arg| arg == "--thread-audit") {
        std::process::exit(thread_audit::run());
    }
    // A11Y-007: the binding table and the reference are window-independent, so the map prints on
    // every platform even where the native workbench is unavailable.
    if args.iter().any(|arg| arg == "--shortcuts") {
        std::process::exit(shortcuts_cli());
    }
    // PLF-009: the release matrix is a document, so its claims are checkable on every platform.
    if args.iter().any(|arg| arg == "--platform-claims") {
        std::process::exit(platform_claims::run());
    }
    // An unrecognised flag is a usage error: name it, print the usage and exit 2. Only a bare
    // launch (or a script path) is the unsupported-platform refusal.
    let unknown: Vec<&String> = args.iter().filter(|arg| arg.starts_with("--")).collect();
    if !unknown.is_empty() {
        eprintln!("unknown option {}\n\n{}", unknown[0], usage());
        std::process::exit(2);
    }
    // PLF-009: a platform without a UI adapter says so by name and refuses to start a placeholder.
    // The text comes from `platform_claims::unsupported_notice`, the same value the test asserts.
    match platform_claims::unsupported_notice(std::env::consts::OS, std::env::consts::ARCH) {
        Some(notice) => eprintln!("{notice}"),
        None => eprintln!("this build targets Windows; the native workbench is unavailable"),
    }
    std::process::exit(2);
}

/// A11Y-007: prints the keyboard map, generated from [`keyboard::BINDINGS`].
///
/// The Help > Keyboard Shortcuts dialog renders the same `reference_rows()` projection, so this
/// output and the dialog are two views of one registration; neither can list a chord the app does
/// not dispatch or omit one it does.
fn shortcuts_cli() -> i32 {
    let rows = keyboard::reference_rows();
    println!("lovelace-desktop --shortcuts (A11Y-007)");
    println!(
        "{} registered bindings, generated from src/keyboard.rs",
        rows.len()
    );
    for scope in keyboard::Scope::ALL {
        println!("\n{}", scope.label().to_uppercase());
        for row in rows.iter().filter(|row| row.scope == scope) {
            println!("  {:<12}  {}", row.chord, row.description);
        }
    }
    println!(
        "\nMathematical glyphs: {} switches between ASCII and Unicode rendering; a glyph the \
         installed font lacks falls back to its ASCII spelling (never a tofu box).",
        keyboard::chord_of(keyboard::Action::ToggleUnicodeGlyphs)
    );
    0
}

#[cfg(windows)]
fn usage() -> String {
    let mut text = String::new();
    text.push_str("lovelace-desktop - a native IDE for the Lovelace scripting language\n\n");
    text.push_str("USAGE:\n");
    text.push_str("  lovelace-desktop [script.ls]        open the native workbench\n");
    text.push_str(
        "  lovelace-desktop --gui-selftest     headless acceptance of the workbench run path\n",
    );
    text.push_str("  lovelace-desktop --journey <ID> [--journey <ID> ...] [--out <dir>]\n");
    text.push_str(
        "                                      run J-02/J-03/J-07/J-08/J-09/J-10/J-15 through the\n",
    );
    text.push_str(
        "                                      workbench path; non-zero when the journeys.md\n",
    );
    text.push_str("                                      expected observable does not match\n");
    text.push_str(
        "  lovelace-desktop --selftest         headless acceptance of the pre-cutover core\n",
    );
    text.push_str("  lovelace-desktop --raster-svg <in.svg> --out <out.png> [--scale N]\n");
    text.push_str(
        "                                      rasterise one SVG exactly as the plot pane does\n",
    );
    text.push_str("  lovelace-desktop --measure-large <script> --measure-out <report>\n");
    text.push_str(
        "                                      open the real window on <script>, measure open,\n",
    );
    text.push_str(
        "                                      typing and scroll-frame budgets, write <report>\n",
    );
    text.push_str("  lovelace-desktop --measure-run <script> --measure-out <report>\n");
    text.push_str(
        "                                      run <script> (>=2 s of engine work) in the real\n",
    );
    text.push_str(
        "                                      window and measure frame/input responsiveness\n",
    );
    text.push_str("  lovelace-desktop --measure-startup [script] --measure-out <report>\n");
    text.push_str(
        "                                      one real-window launch: process entry to the first\n",
    );
    text.push_str(
        "                                      completed frame, plus the launch's setting/recovery\n",
    );
    text.push_str("                                      warnings (PERF-001)\n");
    text.push_str(
        "  lovelace-desktop --measure-edit-scale <script> --measure-out <report> [--chars N]\n",
    );
    text.push_str(
        "                                      type N characters (default 10000) through the\n",
    );
    text.push_str(
        "                                      shipped editor buffer and report latency/memory\n",
    );
    text.push_str("                                      scaling (PERF-009)\n");
    text.push_str("  lovelace-desktop --perf-paint-failure\n");
    text.push_str(
        "                                      feed an invalid plot into the shipped pane, then run\n",
    );
    text.push_str(
        "                                      again (PERF-012; exits non-zero on failure)\n",
    );
    text.push_str("  lovelace-desktop --perf-precision-probe\n");
    text.push_str(
        "                                      report what an extreme precision request does\n",
    );
    text.push_str(
        "                                      (PERF-011; exits non-zero when nothing warns)\n",
    );
    text.push_str(
        "  lovelace-desktop --perf-frame-probe --measure-out <report> [--probe-request=after16]\n",
    );
    text.push_str(
        "                                      diagnostic: interval of every pass of a bare\n",
    );
    text.push_str(
        "                                      eframe window (no PERF row; not the measurement)\n",
    );
    text.push_str(
        "  lovelace-desktop --platform-claims  print the release matrix's platform verdicts and\n",
    );
    text.push_str(
        "                                      the refusal an unsupported build prints (PLF-009)\n",
    );
    text.push_str(
        "  lovelace-desktop --platform-parity  report this platform's modifiers, dialogs,\n",
    );
    text.push_str(
        "                                      clipboard, caret/IME and DPI behaviour (PLF-010;\n",
    );
    text.push_str("                                      exits non-zero when an area fails)\n");
    text.push_str(
        "  lovelace-desktop --ui-audit         audit src/ui for ARC-003 separation (no eval,\n",
    );
    text.push_str(
        "                                      no engine construction, imports allowlisted)\n",
    );
    text.push_str(
        "  lovelace-desktop --thread-audit     audit every thread spawn, join path, sleep and\n",
    );
    text.push_str("                                      synchronisation primitive (ARC-008)\n");
    text.push_str("  lovelace-desktop --contrast-check   print every theme pair's WCAG ratio\n");
    text.push_str(
        "                                      (fails below 4.5:1 text / 3:1 large or non-text)\n",
    );
    text.push_str(
        "  lovelace-desktop --legibility-check print the type scale's rendered size, line ratio\n",
    );
    text.push_str(
        "                                      and contrast per text role (fails below 11 pt,\n",
    );
    text.push_str("                                      1.15x line ratio or 4.5:1)\n");
    text.push_str(
        "  lovelace-desktop --visual-audit     print the type scale, control state matrix,\n",
    );
    text.push_str("                                      semantic colour roles and empty states\n");
    text.push_str(
        "  lovelace-desktop --glyph-check      print the engine glyph set's font coverage and\n",
    );
    text.push_str(
        "                                      fallbacks (fails on a glyph with neither)\n",
    );
    text.push_str(
        "  lovelace-desktop --shortcuts        print the keyboard map from the binding table\n",
    );
    text.push_str("  lovelace-desktop --legacy-win32 [script.ls]\n");
    text.push_str(
        "                                      open the old Win32 shell (explicit fallback only)\n",
    );
    text.push_str("  lovelace-desktop --help             this message\n");
    text.push_str("  lovelace-desktop --version          print the version\n\n");
    text.push_str("SHORTCUTS:\n");
    text.push_str("  Ctrl+N                              new document tab\n");
    text.push_str("  Ctrl+O                              open a Lovelace script from disk\n");
    text.push_str("  Ctrl+S                              save the active document\n");
    text.push_str("  Ctrl+Shift+S                        save the active document as a new file\n");
    text.push_str(
        "  Ctrl+W                              close the active tab (prompts when dirty)\n",
    );
    text.push_str("  Ctrl+Enter                          run the active document\n");
    text.push_str("  Esc                                 cancel the in-flight run\n");
    text.push_str("  Ctrl+Z                              undo the active document's last edit\n");
    text.push_str("  Ctrl+Y, Ctrl+Shift+Z                redo the last undone edit\n");
    text.push_str("  Tab, Shift+Tab                      indent or outdent the selected lines\n");
    text.push_str("  Ctrl+G                              go to a 1-based line or line:column\n");
    text.push_str(
        "  Ctrl+M                              jump to the bracket matching the caret's\n",
    );
    text.push_str("  Ctrl+Shift+O                        find a symbol in the active document\n");
    text.push_str(
        "  Ctrl+F                              find and replace in the active document\n",
    );
    text.push_str("  Ctrl+Tab, Ctrl+Shift+Tab            next / previous document tab\n");
    text.push_str(
        "  Ctrl+,                              editor settings (indent, tabs, auto-close)\n",
    );
    text.push_str(
        "  F8                                  jump to the last failed run's diagnostic\n",
    );
    text.push_str(
        "  Ctrl+Q                              quit (asks about every unsaved document)\n",
    );
    text.push_str("  F6, Shift+F6                        next / previous pane group\n");
    text.push_str(
        "  Ctrl+1..Ctrl+5                      focus toolbar / editor / plot / workspace / logs\n",
    );
    text.push_str("  Ctrl+Alt+1, Ctrl+Alt+2              workspace Result / Inspector view\n");
    text.push_str("  Ctrl+Alt+F                          focus the workspace filter\n");
    text.push_str("  Ctrl+Alt+C                          clear the committed variables\n");
    text.push_str("  Ctrl+0, Ctrl+Shift+0                fit the plot / natural size\n");
    text.push_str("  Ctrl+=, Ctrl+-                      zoom the plot in / out\n");
    text.push_str(
        "  Ctrl+Shift+P                        export the plot as a PNG beside the engine capture\n",
    );
    text.push_str("  Ctrl+Shift+U                        ASCII / Unicode mathematical glyphs\n");
    text.push_str("  Ctrl+Shift+T                        switch the dark / light theme\n");
    text.push_str("  F1                                  Help > Keyboard Shortcuts\n");
    text.push_str(
        "  The Help menu shows the same table (generated from src/keyboard.rs, the one binding\n",
    );
    text.push_str("  registration).\n");
    text.push_str(
        "  The File bar has New, Open..., Save, Save As..., Recent, Close and Quit; closing a\n",
    );
    text.push_str(
        "  document with unsaved changes asks Save / Discard / Cancel. The toolbar has Run,\n",
    );
    text.push_str("  Cancel (while running) and the precision control.\n");
    text.push_str(
        "  At launch the last session's tabs come back by reading their files again, and after a\n",
    );
    text.push_str(
        "  crash a recovery prompt offers the journaled buffers. Nothing is executed by either.\n",
    );
    text
}

/// The usage of a build without a UI adapter: only the headless modes this binary implements.
#[cfg(not(windows))]
fn usage() -> String {
    let mut text = String::new();
    text.push_str("lovelace-desktop - headless tools for the Lovelace scripting language\n\n");
    text.push_str("USAGE:\n");
    text.push_str(
        "  lovelace-desktop --selftest         headless acceptance of the application core\n",
    );
    text.push_str(
        "  lovelace-desktop --gui-selftest     headless acceptance of the workbench run path\n",
    );
    text.push_str("  lovelace-desktop --ui-audit         audit src/ui for ARC-003 separation\n");
    text.push_str("  lovelace-desktop --thread-audit     audit every thread spawn, sleep and\n");
    text.push_str("                                      synchronisation primitive (ARC-008)\n");
    text.push_str(
        "  lovelace-desktop --shortcuts        print the keyboard map from the binding table\n",
    );
    text.push_str(
        "  lovelace-desktop --platform-claims  print the release matrix's platform verdicts\n",
    );
    text.push_str(
        "                                      and the refusal this build prints (PLF-009)\n",
    );
    text.push_str("  lovelace-desktop --help             this message\n");
    text.push_str("  lovelace-desktop --version          print the version\n\n");
    text.push_str(
        "This build has no UI adapter: launching without one of these modes prints the\n",
    );
    text.push_str(
        "unsupported-platform notice and exits 2. The native workbench is a Windows build.\n",
    );
    text
}
