//! A11Y-014: the `--glyph-check` report.
//!
//! This diagnostic mode lives outside `src/ui/` on purpose. ARC-003's source audit requires
//! every file under `src/ui/` to reach the engine only through the execution host's typed
//! message/state surface; constructing `lovelace_lang::Engine` here keeps the window module free
//! of any direct engine call. The report itself still measures the engine's own display glyphs and
//! the shipped font stack, exactly as before.

use crate::glyphs::{self, GlyphMode};
use crate::ui::a11y::{font_has_glyph, glyph_report};

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

/// Runs the A11Y-014 no-tofu report and returns the process exit code.
pub fn run() -> i32 {
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
