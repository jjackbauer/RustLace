//! A11Y-003 (visible focus) and A11Y-014 (glyph localization) for the window.
//!
//! Two window-wide concerns live here so `ui::app` stays the view it already was:
//!
//! - [`paint_focus_ring`] draws one 2 px ring in the active theme's `focus_ring` token around
//!   whatever widget holds keyboard focus. A single ring is what makes the indicator *consistent*:
//!   a button, a text field, a tree header and the editor surface all get the same outline, and
//!   none of them can forget one because the ring is painted once per frame from egui's focus
//!   state rather than by each widget.
//! - [`localize_view`] applies the ASCII/Unicode mathematical-notation setting to the one
//!   assembled `DocumentView` the frame paints, so the structured result, the workspace tables,
//!   the logs result line and the inspector tree cannot disagree. The same pass replaces a glyph
//!   the installed font cannot draw with its documented fallback (`crate::glyphs`), which is what
//!   keeps a missing glyph from becoming a tofu box.
//!
//! Both are testable without a window: the ring painter is exercised by the window's headless
//! tests (they read the painted shapes) and the localization is a pure walk over the view's
//! display fields with the font probe supplied by the caller.

use egui::{Context, FontId, Id, LayerId, Order, Stroke, StrokeKind};
use lovelace_json::Json;

use crate::glyphs::{self, GlyphMode};
use crate::log_kind::LogKind;
use crate::workbench::DocumentView;

use super::theme;

/// The ring's width in logical pixels. Two pixels is a visible perimeter at 100-200% scaling.
pub const FOCUS_RING_WIDTH: f32 = 2.0;

/// The gap between the focused widget's own edge and the ring, in logical pixels.
///
/// The ring is outside the widget, so it never covers the widget's text, its control border or
/// its background fill; the colour behind it is the pane surface the widget sits on, which is the
/// surface the contrast test measures.
pub const FOCUS_RING_GAP: f32 = 2.0;

/// The radius of the ring's corners, matching the window's widget corner radius.
pub const FOCUS_RING_RADIUS: u8 = 3;

/// The layer the ring paints in: above panes, windows and dialogs, so a focused control inside a
/// prompt keeps its indicator.
pub fn focus_ring_layer() -> LayerId {
    LayerId::new(Order::Tooltip, Id::unique("a11y_focus_ring"))
}

/// Paints the keyboard-focus ring around whatever widget currently owns focus.
///
/// Nothing is painted when no widget is focused or when the focused widget was not painted this
/// pass (for example a control inside a window that just closed), so a stale ring can never
/// linger over unrelated content.
pub fn paint_focus_ring(ctx: &Context) {
    let Some(id) = ctx.memory(|memory| memory.focused()) else {
        return;
    };
    let Some(rect) = ctx.read_response(id).map(|response| response.rect) else {
        return;
    };
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let painter = ctx.layer_painter(focus_ring_layer());
    painter.rect_stroke(
        rect.expand(FOCUS_RING_GAP),
        egui::CornerRadius::same(FOCUS_RING_RADIUS),
        Stroke::new(FOCUS_RING_WIDTH, theme::focus_ring()),
        StrokeKind::Outside,
    );
}

/// The JSON keys whose string values are display text the panes paint.
///
/// Only these fields are localized: a structural field such as `kind`, `name` or `type` keeps its
/// exact spelling, because it is an identifier (a variable named `pi` must not become `π`) rather
/// than a rendering. The list is the union of the keys `results_tree` and `inspector` read as
/// text.
const DISPLAY_KEYS: &[&str] = &[
    "pretty",
    "display",
    "value",
    "label",
    "canonical",
    "detail",
    "text",
    "left",
    "right",
    "source",
    "opLabel",
    "classification",
    "conditions",
    "resultType",
    "summary",
    "reason",
    "expression",
];

/// True when a string under this key is display text.
fn is_display_key(key: &str) -> bool {
    DISPLAY_KEYS.contains(&key)
}

/// Renders `text` and returns `Some` only when the rendering differs, so an unchanged value keeps
/// its allocation and the per-frame pass stays cheap on an ASCII document.
fn localized(
    text: &str,
    mode: GlyphMode,
    has_glyph: &mut impl FnMut(char) -> bool,
) -> Option<String> {
    let rendered = glyphs::render(text, mode, has_glyph);
    (rendered != text).then_some(rendered)
}

/// Rewrites every display string in a structured projection.
fn localize_json(value: &mut Json, mode: GlyphMode, has_glyph: &mut impl FnMut(char) -> bool) {
    match value {
        Json::Str(text) => {
            if let Some(rendered) = localized(text, mode, has_glyph) {
                *text = rendered;
            }
        }
        Json::Arr(items) => {
            for item in items {
                localize_json(item, mode, has_glyph);
            }
        }
        Json::Obj(fields) => {
            for (key, field) in fields.iter_mut() {
                match field {
                    Json::Str(text) if is_display_key(key) => {
                        if let Some(rendered) = localized(text, mode, has_glyph) {
                            *text = rendered;
                        }
                    }
                    // A display field can hold a nested object or array (a record of symbolic
                    // values, a list of solutions); recurse so their display strings are reached
                    // while identifiers stay exact.
                    Json::Arr(_) | Json::Obj(_) => localize_json(field, mode, has_glyph),
                    _ => {}
                }
            }
        }
        Json::Null | Json::Bool(_) | Json::Num(_) => {}
    }
}

/// Applies the ASCII/Unicode setting to every display field of the frame's view.
///
/// A glyph whose character is not drawable by the installed font is replaced by its documented
/// fallback as part of the same call, so every surface shows the same string for the same value.
/// `has_glyph` is the window's font probe (cached by the caller).
///
/// The four A11Y-014 surfaces are covered by construction:
///
/// | surface | fields |
/// | --- | --- |
/// | structured result | `result`, `result_structured.display` |
/// | workspace | `variables[].display`, `state.variables[].display`/`structured` |
/// | logs | the value-bearing `[result]` line |
/// | inspector | `result_inspection` display fields |
///
/// Source echoes, `print(...)` output, diagnostics and the status strip are the user's own words.
/// They are left byte-exact, so the setting can never change what a script printed.
pub fn localize_view(
    view: &mut DocumentView,
    mode: GlyphMode,
    has_glyph: &mut impl FnMut(char) -> bool,
) {
    if let Some(rendered) = localized(&view.result, mode, has_glyph) {
        view.result = rendered;
    }
    if let Some(structured) = view.result_structured.as_mut() {
        localize_json(structured, mode, has_glyph);
    }
    if let Some(inspection) = view.result_inspection.as_mut() {
        localize_json(inspection, mode, has_glyph);
    }
    // Workspace: the variable display column and the same value inside the completion snapshot's
    // structured projection. Names and kinds are identifiers and are left exact.
    for (_, display) in view.variables.iter_mut() {
        if let Some(rendered) = localized(display, mode, has_glyph) {
            *display = rendered;
        }
    }
    for variable in view.state.variables.iter_mut() {
        if let Some(rendered) = localized(&variable.display, mode, has_glyph) {
            variable.display = rendered;
        }
        if let Some(structured) = variable.structured.as_mut() {
            localize_json(structured, mode, has_glyph);
        }
    }
    // Logs: only the value-bearing result line is a rendering of a value.
    for line in view.logs.iter_mut() {
        if line.kind != LogKind::Result {
            continue;
        }
        if let Some(rendered) = localized(&line.text, mode, has_glyph) {
            line.text = rendered;
        }
    }
}

/// True when the installed font can draw `c` in the UI's monospace or proportional family.
///
/// Asking egui's own font stack means the answer describes the shipped window, including any
/// system font the platform resolver added, rather than a guess from a glyph table.
pub fn font_has_glyph(ctx: &Context, c: char) -> bool {
    ctx.fonts_mut(|fonts| {
        fonts.has_glyph(&FontId::monospace(13.0), c)
            || fonts.has_glyph(&FontId::proportional(13.0), c)
    })
}

/// Every non-ASCII character in `sample` with its fallback and whether the font can draw it.
///
/// This is the A11Y-014 no-tofu report: a character with `has_glyph == false` and no fallback
/// would be drawn as `?` by [`glyphs::render`], so the report is empty of such characters for a
/// shipped sample.
pub fn glyph_report(
    sample: &str,
    has_glyph: &mut impl FnMut(char) -> bool,
) -> Vec<(char, bool, Option<&'static str>)> {
    glyphs::non_ascii(sample)
        .into_iter()
        .map(|c| (c, has_glyph(c), glyphs::fallback(c)))
        .collect()
}

/// A11Y-014: the `--glyph-check` report.
///
/// It collects the non-ASCII character set the engine actually emits for a symbolic and a plot
/// sample, measures every character against the shipped font stack (the same probe the window
/// uses), and prints the rendering in both modes plus each character's fallback. The exit code is
/// non-zero when a character is neither drawable nor has a documented fallback — that is exactly
/// the condition that would draw a tofu box.
pub fn glyph_cli() -> i32 {
    let sample = engine_glyph_sample();
    let unicode = glyphs::to_unicode(&sample);
    // One empty pass initialises the font stack, the same way the window's first frame does.
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    output.textures_delta.clear();

    let mut coverage = std::collections::HashMap::new();
    let mut probe = |c: char| -> bool {
        *coverage
            .entry(c)
            .or_insert_with(|| font_has_glyph(&ctx, c))
    };
    let report = glyph_report(&unicode, &mut probe);
    let mut missing = Vec::new();
    println!("lovelace-desktop --glyph-check (A11Y-014)");
    println!(
        "engine sample: {} bytes, {} non-ASCII glyph(s) in the engine's Unicode rendering",
        unicode.len(),
        report.len()
    );
    for (glyph, drawable, fallback) in &report {
        let state = if *drawable {
            "font".to_string()
        } else if let Some(replacement) = fallback {
            format!("fallback {replacement}")
        } else {
            missing.push(*glyph);
            "MISSING".to_string()
        };
        println!(
            "  U+{:04X} {:<6} -> {}",
            *glyph as u32,
            format!("{glyph}"),
            state
        );
    }
    let short = |text: &str| -> String {
        // Prefer a line that carries the glyphs the check is about (the engine sample's first
        // lines are plain ASCII expressions); fall back to the first line.
        let line = text
            .lines()
            .find(|line| !line.is_ascii())
            .or_else(|| text.lines().find(|line| line.contains("glyph check")))
            .unwrap_or_default();
        let mut shortened: String = line.chars().take(160).collect();
        if line.chars().count() > 160 {
            shortened.push('…');
        }
        shortened.trim().to_string()
    };
    println!(
        "ascii   : {}",
        short(&glyphs::render(&sample, GlyphMode::Ascii, &mut probe))
    );
    println!(
        "unicode : {}",
        short(&glyphs::render(&unicode, GlyphMode::Unicode, &mut probe))
    );
    // No tofu: every non-ASCII character that reached the rendered string must be drawable (the
    // fallback already replaced the ones that were not). A character the font cannot draw here
    // would be painted as a tofu box by the window.
    let rendered = glyphs::render(&unicode, GlyphMode::Unicode, &mut probe);
    let leftover: Vec<char> = glyphs::non_ascii(&rendered)
        .into_iter()
        .filter(|c| !font_has_glyph(&ctx, *c))
        .collect();
    if missing.is_empty() && leftover.is_empty() && !rendered.contains('\u{FFFD}') {
        println!(
            "result PASS: every glyph is drawable or mapped; nothing is handed to the font as tofu"
        );
        0
    } else {
        println!("result FAIL: missing={missing:?} unrendered after fallback={leftover:?}");
        1
    }
}

/// The engine-emitted glyph sample the A11Y-014 check measures.
///
/// It is the engine's own display formatter (ASCII, the shipped default) for a symbolic
/// expression, the named constants, a relation and a plot title, plus the window's Unicode
/// additions (`×` and `√`) and one astral mathematical character. Non-ASCII characters in the
/// plot capture's SVG are included too: the plot is engine bytes, not a desktop rendering.
fn engine_glyph_sample() -> String {
    let mut lines: Vec<String> = Vec::new();
    #[cfg(feature = "real-engine")]
    {
        let mut engine = lovelace_lang::Engine::new();
        let plot_dir = std::env::temp_dir().join("lovelace-glyph-check");
        let _ = std::fs::create_dir_all(&plot_dir);
        engine.set_plot_dir(&plot_dir.display().to_string());
        for script in [
            "x = symbol(\"x\")",
            "3*x^2",
            "inf",
            "x <= 5",
            "x != 0",
            "sqrt(2)",
            "pi(20)",
            "plot([1, 4, 2], [4, 5, 6], \"\u{221a} \u{03c0} \u{2264} glyph check\")",
        ] {
            if let Ok(value) = engine.eval(script) {
                lines.push(engine.format(&value));
            }
        }
        if let Some(plot) = engine.last_plot() {
            lines.push(plot.svg);
        }
    }
    lines.push(glyphs::SAMPLE_GLYPHS.to_string());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::EngineState;
    use crate::execution::{Diagnostic, SourceRevision};
    use crate::log_kind::LogLine;
    use crate::run_state::RunState;
    use lovelace_json::Json;

    fn diagnostic() -> Diagnostic {
        Diagnostic {
            message: "sqrt(2)*pi is not a number".to_string(),
            code: Some("TypeError".to_string()),
            category: Some("type".to_string()),
            suggestions: vec!["sqrt".to_string()],
            statement_index: 0,
            statement_line: 1,
            byte_offset: 0,
            line: 1,
            column: 1,
        }
    }

    fn view() -> DocumentView {
        DocumentView {
            revision: SourceRevision(1),
            has_snapshot: true,
            committed_steps: 2,
            total: 2,
            result: "sqrt(2)*pi".to_string(),
            result_structured: Some(Json::obj(vec![
                ("kind", Json::Str("symbolic".to_string())),
                ("pretty", Json::Str("sqrt(2)*pi".to_string())),
            ])),
            result_inspection: Some(Json::obj(vec![
                ("kind", Json::Str("tree".to_string())),
                (
                    "root",
                    Json::obj(vec![
                        ("kind", Json::Str("Fun".to_string())),
                        ("label", Json::Str("sqrt(2)*pi".to_string())),
                    ]),
                ),
            ])),
            variables: vec![("y".to_string(), "sqrt(2)*pi".to_string())],
            functions: Vec::new(),
            state: EngineState::default(),
            plot: None,
            output: vec!["sqrt(2)*pi".to_string()],
            logs: vec![
                LogLine::new(LogKind::Stdout, "sqrt(2)*pi").at_statement(0),
                LogLine::new(LogKind::Result, "sqrt(2)*pi"),
            ],
            diagnostic: Some(diagnostic()),
            run_state: RunState::Finished,
            buffer_revision: SourceRevision(1),
            progress: "2/2".to_string(),
            precision: 30,
            elapsed_ms: 1,
        }
    }

    fn localize(mode: GlyphMode) -> DocumentView {
        let mut view = view();
        // A font with full coverage, so the assertion is about the mode mapping itself; the
        // missing-glyph path is asserted separately with a probe that accepts no non-ASCII.
        localize_view(&mut view, mode, &mut |_c: char| true);
        view
    }

    #[test]
    fn the_same_value_renders_in_both_modes_on_every_surface() {
        let ascii = localize(GlyphMode::Ascii);
        let unicode = localize(GlyphMode::Unicode);
        // Structured result caption and tree.
        assert_eq!(ascii.result, "sqrt(2)*pi");
        assert_eq!(unicode.result, "\u{221a}(2)\u{00d7}\u{03c0}");
        let ascii_tree = ascii.result_structured.as_ref().unwrap();
        let unicode_tree = unicode.result_structured.as_ref().unwrap();
        assert_eq!(
            ascii_tree.get("pretty").and_then(Json::as_str),
            Some("sqrt(2)*pi")
        );
        assert_eq!(
            unicode_tree.get("pretty").and_then(Json::as_str),
            Some("\u{221a}(2)\u{00d7}\u{03c0}")
        );
        assert_eq!(
            ascii_tree.get("kind").and_then(Json::as_str),
            Some("symbolic"),
            "a structural field is never localized"
        );
        // Workspace display column.
        assert_eq!(ascii.variables[0].1, "sqrt(2)*pi");
        assert_eq!(unicode.variables[0].1, "\u{221a}(2)\u{00d7}\u{03c0}");
        // Logs result line.
        assert_eq!(
            ascii
                .logs
                .iter()
                .find(|line| line.kind == LogKind::Result)
                .unwrap()
                .text,
            "sqrt(2)*pi"
        );
        assert_eq!(
            unicode
                .logs
                .iter()
                .find(|line| line.kind == LogKind::Result)
                .unwrap()
                .text,
            "\u{221a}(2)\u{00d7}\u{03c0}"
        );
        // Inspector label.
        let ascii_root = ascii.result_inspection.as_ref().unwrap().get("root").unwrap();
        let unicode_root = unicode.result_inspection.as_ref().unwrap().get("root").unwrap();
        assert_eq!(
            ascii_root.get("label").and_then(Json::as_str),
            Some("sqrt(2)*pi")
        );
        assert_eq!(
            unicode_root.get("label").and_then(Json::as_str),
            Some("\u{221a}(2)\u{00d7}\u{03c0}")
        );
    }

    #[test]
    fn ascii_mode_is_byte_identical_to_the_engine_default() {
        // The default mode must not rewrite anything the engine already produced.
        let ascii = localize(GlyphMode::Ascii);
        assert_eq!(ascii.result, view().result);
        assert_eq!(ascii.variables, view().variables);
        assert_eq!(ascii.logs, view().logs);
        assert_eq!(ascii.result_structured, view().result_structured);
    }

    #[test]
    fn user_text_and_diagnostics_are_not_localized() {
        let unicode = localize(GlyphMode::Unicode);
        // print() output and a diagnostic message are the user's/source's own words.
        assert_eq!(unicode.output, view().output);
        assert_eq!(
            unicode.diagnostic.as_ref().unwrap().message,
            "sqrt(2)*pi is not a number"
        );
        assert_eq!(
            unicode
                .logs
                .iter()
                .find(|line| line.kind == LogKind::Stdout)
                .unwrap()
                .text,
            "sqrt(2)*pi",
            "stdout is not a value rendering"
        );
    }

    #[test]
    fn identifiers_are_never_rewritten_as_named_constants() {
        // A variable literally named `pi` keeps its name in the workspace table; only its value
        // is rendered in the selected notation.
        let mut view = view();
        view.variables = vec![("pi".to_string(), "sqrt(2)*pi".to_string())];
        localize_view(&mut view, GlyphMode::Unicode, &mut |_c: char| true);
        assert_eq!(view.variables[0].0, "pi");
        assert_eq!(view.variables[0].1, "\u{221a}(2)\u{00d7}\u{03c0}");
    }

    #[test]
    fn a_missing_glyph_never_reaches_the_painted_string() {
        let mut view = view();
        view.result = "\u{221a}(2)\u{00d7}\u{03c0} \u{2264} \u{221e}".to_string();
        localize_view(&mut view, GlyphMode::Unicode, &mut |_c: char| false);
        assert_eq!(view.result, "sqrt(2)*pi <= inf");
        assert!(view.result.is_ascii());
    }

    #[test]
    fn the_no_tofu_report_covers_the_engine_sample() {
        // A sample with the engine printer's glyphs, the two this window adds and an astral
        // mathematical-alphanumeric character.
        let sample = format!("{} and {}", glyphs::SAMPLE_GLYPHS, '\u{1D70B}');
        let mut probe = |_c: char| false;
        let report = glyph_report(&sample, &mut probe);
        assert!(!report.is_empty());
        for (glyph, drawable, fallback) in report {
            assert!(
                drawable || fallback.is_some(),
                "{glyph:?} has no coverage and no documented fallback"
            );
        }
        // A font with nothing at all still renders ASCII through the table.
        let rendered = glyphs::render(&sample, GlyphMode::Unicode, |_c: char| false);
        assert!(rendered.is_ascii(), "no tofu: {rendered:?}");
        assert!(!rendered.contains('\u{FFFD}'));
    }

    #[test]
    fn the_display_key_whitelist_covers_the_panes_text_fields() {
        for key in [
            "pretty",
            "value",
            "label",
            "canonical",
            "detail",
            "text",
            "left",
            "right",
            "source",
            "opLabel",
            "classification",
            "conditions",
            "resultType",
            "summary",
            "reason",
            "display",
            "expression",
        ] {
            assert!(is_display_key(key), "{key} must be localized");
        }
        for key in [
            "kind", "name", "type", "path", "code", "category", "ruleId", "shape",
        ] {
            assert!(
                !is_display_key(key),
                "{key} is structural and must stay exact"
            );
        }
    }

    #[test]
    fn the_ring_constants_are_visible_and_do_not_cover_the_control() {
        assert!(FOCUS_RING_WIDTH >= 2.0);
        assert!(FOCUS_RING_GAP >= 2.0);
        assert_eq!(FOCUS_RING_RADIUS, 3);
        assert_ne!(focus_ring_layer(), egui::LayerId::background());
    }
}
