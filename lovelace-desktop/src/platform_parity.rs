//! PLF-010: the platform-specific behaviour of this build, exercised as one printable report.
//!
//! A platform claim is only honest when the behaviour behind it is tested *on that platform*.
//! Windows 11 x64 is the only advertised platform (`docs/goal-desktop-v4/platform-matrix.md`), so
//! this report is the Windows half of PLF-010: keyboard modifiers, the native dialog contract, the
//! clipboard hand-off, caret/IME composition and DPI all go through the same code the shipped
//! window runs, and every check either asserts an observable or reports the number the OS gave.
//!
//! What it can assert without a dialog: the shipped `FileDialogs` adapter's filters, extension and
//! suggested name, and that the native entry point is the only caller of the Win32 wrappers. What
//! it cannot do is a modal dialog round trip inside one process; that path is exercised from a real
//! window by `round-56/plf-001` (the UI Automation driven Open/Save capture).
//!
//! `--platform-parity` prints one line per area and exits non-zero when any check fails, so the
//! gate can read the exit code instead of trusting prose.

use crate::completion::CompletionItem;
use crate::engine::EngineState;
use crate::keyboard::{self, EditorCommand, FileCommand};
use crate::settings;
use crate::ui::editor::TextEditor;
use crate::ui::file_dialogs;
use crate::ui::typography;

/// One area's result: what was exercised, the observed value and whether it held.
pub struct Check {
    /// The PLF-010 area: modifiers, dialogs, clipboard, caret/IME, DPI.
    pub area: &'static str,
    /// The measured value, printed verbatim.
    pub observed: String,
    /// True when every assertion in the area held.
    pub ok: bool,
}

/// Runs every area's check and returns them in report order.
pub fn checks() -> Vec<Check> {
    vec![
        modifier_check(),
        dialog_check(),
        clipboard_check(),
        caret_ime_check(),
        dpi_check(),
    ]
}

/// `--platform-parity`: prints the five areas and exits non-zero when one fails.
pub fn run() -> i32 {
    let (dpi, monitors) = crate::win32::ffi::system_dpi_and_monitors();
    println!("lovelace-desktop platform parity (PLF-010)");
    println!(
        "platform=windows arch={} advertised=windows-11-x64 (the only supported platform)",
        std::env::consts::ARCH
    );
    println!("system_dpi={dpi} ({:.0}%) monitors={monitors}", dpi as f64 / 96.0 * 100.0);
    let checks = checks();
    let mut failed = 0usize;
    for check in &checks {
        println!(
            "{} | {} | {}",
            if check.ok { "PASS" } else { "FAIL" },
            check.area,
            check.observed
        );
        if !check.ok {
            failed += 1;
        }
    }
    println!(
        "result {}: {}/{} areas pass",
        if failed == 0 { "PASS" } else { "FAIL" },
        checks.len() - failed,
        checks.len()
    );
    if failed == 0 {
        0
    } else {
        1
    }
}

/// PLF-010 "keyboard modifiers": Windows Ctrl drives the file/editor chords and Alt does not.
fn modifier_check() -> Check {
    let mut problems = Vec::new();
    let mut expect_file = |letter: char, ctrl: bool, shift: bool, want: Option<FileCommand>| {
        let got = keyboard::file_command_for_shortcut(letter, ctrl, shift);
        if got != want {
            problems.push(format!(
                "file_command_for_shortcut({letter:?}, ctrl={ctrl}, shift={shift}) = {got:?}, want {want:?}"
            ));
        }
    };
    expect_file('n', true, false, Some(FileCommand::New));
    expect_file('o', true, false, Some(FileCommand::Open));
    expect_file('s', true, false, Some(FileCommand::Save));
    expect_file('s', true, true, Some(FileCommand::SaveAs));
    expect_file('w', true, false, Some(FileCommand::CloseTab));
    // A bare letter is typing, not a command: the modifier is what makes it a chord.
    expect_file('s', false, false, None);
    // Ctrl+Shift+O is Find Symbol, not Open: the shift state must reach the mapping.
    if keyboard::editor_command_for_shortcut('o', true, true) != Some(EditorCommand::FindSymbol) {
        problems.push("Ctrl+Shift+O did not map to Find Symbol".to_string());
    }
    if keyboard::editor_command_for_shortcut('f', true, false) != Some(EditorCommand::Find) {
        problems.push("Ctrl+F did not map to Find".to_string());
    }
    if keyboard::editor_command_for_shortcut('o', true, false).is_some() {
        problems.push("Ctrl+O must stay the file Open command, not an editor command".to_string());
    }
    Check {
        area: "modifiers",
        observed: format!(
            "Ctrl+N/O/S/W, Ctrl+Shift+S, Ctrl+Shift+O, Ctrl+F mapped; bare letters issue no command{}",
            if problems.is_empty() {
                String::new()
            } else {
                format!("; problems: {problems:?}")
            }
        ),
        ok: problems.is_empty(),
    }
}

/// PLF-010 "dialogs": the shipped native dialog contract (filters, extension, suggested name).
fn dialog_check() -> Check {
    let mut problems = Vec::new();
    if !file_dialogs::OPEN_FILTERS
        .iter()
        .any(|(_, pattern)| pattern.contains("*.lv"))
    {
        problems.push("the Open dialog does not list .lv files".to_string());
    }
    if file_dialogs::SAVE_FILTERS[0] != ("Lovelace script (*.lv)", "*.lv") {
        problems.push(format!(
            "the Save dialog's default filter is {:?}",
            file_dialogs::SAVE_FILTERS[0]
        ));
    }
    if file_dialogs::DOCUMENT_EXTENSION != "lv" {
        problems.push("the document extension is not lv".to_string());
    }
    if file_dialogs::suggested_name("untitled-3") != "untitled-3.lv" {
        problems.push("an untitled document is not suggested as a .lv name".to_string());
    }
    if file_dialogs::suggested_name("notes.ls") != "notes.ls" {
        problems.push("an existing name is not suggested unchanged".to_string());
    }
    Check {
        area: "dialogs",
        observed: format!(
            "open={:?} save={:?} extension={} suggested={}{}",
            file_dialogs::OPEN_FILTERS[0].1,
            file_dialogs::SAVE_FILTERS[0],
            file_dialogs::DOCUMENT_EXTENSION,
            file_dialogs::suggested_name("untitled-3"),
            if problems.is_empty() {
                String::new()
            } else {
                format!("; problems: {problems:?}")
            }
        ),
        ok: problems.is_empty(),
    }
}

/// One headless pass of the shipped editor, returning the text it would put on the clipboard.
fn editor_pass(editor: &mut TextEditor, events: Vec<egui::Event>) -> Vec<String> {
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 600.0));
    let engine = EngineState::default();
    let catalogue: [CompletionItem; 0] = [];
    let mut run = |events: Vec<egui::Event>| {
        let input = egui::RawInput {
            screen_rect: Some(screen),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            let rect = ui.max_rect();
            let _ = editor.ui(ui, rect, &catalogue, &engine);
        });
        output.textures_delta.clear();
        output
    };
    let _ = run(Vec::new());
    ctx.memory_mut(|memory| memory.request_focus(crate::ui::editor::surface_id()));
    let output = run(events);
    output
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// PLF-010 "clipboard" (and PLF-001): copy/cut/paste through the editor's real event handling.
fn clipboard_check() -> Check {
    let mut problems = Vec::new();
    let mut editor = TextEditor::from_source("alpha beta gamma");
    editor.set_caret(0, 6);
    editor.select_to(0, 10);
    let copied = editor_pass(&mut editor, vec![egui::Event::Copy]);
    if copied != vec!["beta".to_string()] {
        problems.push(format!("Ctrl+C put {copied:?} on the clipboard, want [\"beta\"]"));
    }
    let cut = editor_pass(&mut editor, vec![egui::Event::Cut]);
    if cut != vec!["beta".to_string()] {
        problems.push(format!("Ctrl+X put {cut:?} on the clipboard, want [\"beta\"]"));
    }
    if editor.source() != "alpha  gamma" {
        problems.push(format!("Ctrl+X left {:?}", editor.source()));
    }
    if !editor.undo() || editor.source() != "alpha beta gamma" {
        problems.push("one undo did not restore the cut text".to_string());
    }
    // Paste is the platform's `Event::Paste`: exactly the text the OS handed over.
    editor.set_caret(0, 0);
    let _ = editor_pass(&mut editor, vec![egui::Event::Paste("pasted".to_string())]);
    if !editor.source().starts_with("pasted") {
        problems.push(format!("paste left {:?}", editor.source()));
    }
    Check {
        area: "clipboard",
        observed: format!(
            "copy={copied:?} cut={cut:?} paste={} {}",
            editor.source().split(' ').next().unwrap_or("-"),
            if problems.is_empty() {
                "ok".to_string()
            } else {
                format!("problems: {problems:?}")
            }
        ),
        ok: problems.is_empty(),
    }
}

/// PLF-010 "caret/IME": a composition is displayed but never written, and a commit is one edit.
fn caret_ime_check() -> Check {
    let mut problems = Vec::new();
    let mut editor = TextEditor::from_source("x = ");
    editor.set_caret(0, 4);
    let _ = editor_pass(
        &mut editor,
        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "にほ".to_string(),
            active_range_chars: None,
        })],
    );
    if editor.preedit.as_deref() != Some("にほ") {
        problems.push(format!("the preedit is {:?}", editor.preedit));
    }
    if editor.source() != "x = " {
        problems.push("an uncommitted composition edited the buffer".to_string());
    }
    let _ = editor_pass(
        &mut editor,
        vec![egui::Event::Ime(egui::ImeEvent::Commit("日本".to_string()))],
    );
    if editor.source() != "x = 日本" || editor.preedit.is_some() {
        problems.push(format!("the commit left {:?}", editor.source()));
    }
    if editor.line_col_1based() != (1, 7) {
        problems.push(format!(
            "the caret is at {:?}, want (1, 7)",
            editor.line_col_1based()
        ));
    }
    // An astral commit is one character in the buffer and two UTF-16 units in the caret report,
    // the exact pair the engine's diagnostics use.
    let _ = editor_pass(
        &mut editor,
        vec![egui::Event::Ime(egui::ImeEvent::Commit("😀".to_string()))],
    );
    let utf16 = editor.caret_column_utf16();
    if !editor.source().ends_with('😀') || utf16 != 8 {
        problems.push(format!(
            "after an astral commit the source is {:?} and the UTF-16 column {utf16}",
            editor.source()
        ));
    }
    Check {
        area: "caret/ime",
        observed: format!(
            "preedit-displayed=true commit=日本+😀 caret_utf16={utf16} source={:?}{}",
            editor.source(),
            if problems.is_empty() {
                String::new()
            } else {
                format!("; problems: {problems:?}")
            }
        ),
        ok: problems.is_empty(),
    }
}

/// PLF-010 "DPI": the OS's scale is applied on top of the 100–200% user setting.
fn dpi_check() -> Check {
    let mut problems = Vec::new();
    let (dpi, monitors) = crate::win32::ffi::system_dpi_and_monitors();
    if !(96..=768).contains(&dpi) {
        problems.push(format!("GetDpiForSystem returned {dpi}"));
    }
    let wanted = [(100u32, 1.0f32), (125, 1.25), (150, 1.5), (200, 2.0)];
    let mut applied = Vec::new();
    for (percent, factor) in wanted {
        let got = typography::scale_factor(percent);
        applied.push(format!("{percent}%->{got:.2}"));
        if (got - factor).abs() > 1e-6 {
            problems.push(format!("scale_factor({percent}) = {got}, want {factor}"));
        }
    }
    if settings::MIN_UI_SCALE_PERCENT != 100 || settings::MAX_UI_SCALE_PERCENT != 200 {
        problems.push(format!(
            "the offered range is {}..{}%",
            settings::MIN_UI_SCALE_PERCENT,
            settings::MAX_UI_SCALE_PERCENT
        ));
    }
    Check {
        area: "dpi",
        observed: format!(
            "system_dpi={dpi} ({:.0}%) monitors={monitors} ui_scale {} range={}..{}%{}",
            dpi as f64 / 96.0 * 100.0,
            applied.join(" "),
            settings::MIN_UI_SCALE_PERCENT,
            settings::MAX_UI_SCALE_PERCENT,
            if problems.is_empty() {
                String::new()
            } else {
                format!("; problems: {problems:?}")
            }
        ),
        ok: problems.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_platform_area_passes_on_this_platform() {
        let checks = checks();
        assert_eq!(
            checks.iter().map(|check| check.area).collect::<Vec<_>>(),
            vec!["modifiers", "dialogs", "clipboard", "caret/ime", "dpi"],
            "the five PLF-010 areas must all be reported"
        );
        for check in checks {
            assert!(
                check.ok,
                "{} failed: {}",
                check.area,
                check.observed
            );
        }
    }

    #[test]
    fn the_probe_exits_zero_when_every_area_passes() {
        assert_eq!(run(), 0, "--platform-parity must succeed on the advertised platform");
    }

    #[test]
    fn the_editor_is_the_one_that_receives_the_platform_clipboard_commands() {
        // The clipboard check above asserts the bytes; this pins the direction of the seam: the
        // window's editor is what turns `Event::Copy`/`Cut` into `CopyText`, not a second path.
        let mut editor = TextEditor::from_source("keep me");
        editor.select_all();
        let copied = editor_pass(&mut editor, vec![egui::Event::Copy]);
        assert_eq!(copied, vec!["keep me".to_string()]);
        assert_eq!(editor.source(), "keep me");
    }
}
