//! A11Y-005 (editor typography) and the font half of A11Y-004 (interface scale).
//!
//! One [`Typography`] value is derived from the persisted [`TypographySettings`] and is the only
//! place the editor's geometry comes from:
//!
//! | geometry | source |
//! | --- | --- |
//! | glyph height | `Typography::font_id().size` |
//! | row height | `Typography::row_height()` = size × line height (raised to the font's own row) |
//! | character advance | the layout of `M` in the resolved family |
//! | gutter numbers | `Typography::gutter_font()` (two points smaller, never below 9) |
//!
//! Because the paint pass derives *every* decoration from those three numbers, changing the
//! family, the size, the line height or the ligature choice cannot move one decoration without
//! moving the text: the gutter number, the diagnostic underline, the bracket cell and the caret
//! are all `column × advance` horizontally and `row × row_height` vertically.
//!
//! ## Font resolution
//!
//! [`EditorFont::Hack`] is the face egui ships, so it needs no file and is always available.
//! Every other family names one font file under the Windows font directory:
//!
//! | family | font file |
//! | --- | --- |
//! | Hack | shipped (`FontFamily::Monospace`) |
//! | Cascadia | `CascadiaCode.ttf` (falls back to `CascadiaMono.ttf`) |
//! | Consolas | `consola.ttf` |
//! | Courier New | `cour.ttf` |
//! | Lucida Console | `lucon.ttf` |
//!
//! A family whose file is not installed falls back to the shipped face, so the editor always has
//! a drawable font.
//!
//! ## Ligatures
//!
//! egui's text stack shapes with HarfBuzz but exposes no per-pass OpenType feature switch, so a
//! ligature *setting* cannot toggle the font's own `liga`/`calt` lookups. The option is therefore
//! implemented in the editor's display layer: [`render_line`] replaces a two-character operator
//! sequence with the single Unicode glyph the ligature is drawn as, plus a space for the cell the
//! other character occupied. The rendered line keeps the source line's character count, which is
//! what keeps the caret, the diagnostic underline and the bracket cell on the same columns as the
//! buffer. String literals are never rewritten, and a glyph the font cannot draw at exactly one
//! cell is left as the original pair. The buffer itself is never changed by this option.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use egui::{Context, FontData, FontFamily, FontId, FontInsert, FontPriority, InsertFontFamily};

use crate::settings::{self, EditorFont, TypographySettings};

/// A11Y-005: the display substitutions the ligature option applies.
///
/// Every entry is `(source sequence, single-glyph rendering)`. The rendering occupies the source
/// sequence's cells (one glyph and `len - 1` spaces), so columns never move. Longest sequences
/// come first so `<=>` is not read as `<=`.
pub const LIGATURES: [(&str, char); 9] = [
    ("<=>", '\u{21d4}'),
    ("->", '\u{2192}'),
    ("=>", '\u{21d2}'),
    ("!=", '\u{2260}'),
    ("<=", '\u{2264}'),
    (">=", '\u{2265}'),
    ("==", '\u{2261}'),
    (":=", '\u{2254}'),
    ("&&", '\u{2227}'),
];

/// The interface scale factor for a persisted percentage, clamped to the offered range.
///
/// It multiplies egui's `pixels_per_point`, which eframe already sets from the OS DPI scale, so
/// the setting is an accessibility zoom *on top of* the OS DPI rather than a replacement for it.
pub fn scale_factor(percent: u32) -> f32 {
    percent.clamp(
        settings::MIN_UI_SCALE_PERCENT,
        settings::MAX_UI_SCALE_PERCENT,
    ) as f32
        / 100.0
}

/// One resolved typography: the family, the font id and the two geometry numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    /// The configured family (for the header's summary and the settings window).
    pub font: EditorFont,
    /// The resolved family (the shipped monospace stack, or a registered system family).
    pub family: FontFamily,
    /// The editor font size in points.
    pub size: f32,
    /// The line height as a multiple of `size`.
    pub line_height: f32,
    /// True when the ligature setting is on.
    pub ligatures: bool,
}

impl Typography {
    /// Resolves the persisted settings into the family and metrics the editor paints with.
    ///
    /// The family is only a name here; [`ensure_installed`] registers its file with the context
    /// before the first layout. An unavailable family falls back to the shipped monospace stack,
    /// so the editor always has a drawable face even on a machine without Consolas.
    pub fn new(settings: &TypographySettings) -> Typography {
        let family = if settings.font == EditorFont::Hack {
            FontFamily::Monospace
        } else if font_file(settings.font).is_some() {
            FontFamily::Name(family_name(settings.font).into())
        } else {
            FontFamily::Monospace
        };
        Typography {
            font: settings.font,
            family,
            size: settings.font_size(),
            line_height: settings.line_height(),
            ligatures: settings.ligatures,
        }
    }

    /// The font the editor lays out with.
    pub fn font_id(&self) -> FontId {
        FontId::new(self.size, self.family.clone())
    }

    /// The font the gutter line numbers use: two points smaller, never below 9.
    pub fn gutter_font(&self) -> FontId {
        FontId::new((self.size - 2.0).max(9.0), self.family.clone())
    }

    /// The row height in points. Every vertical decoration is derived from this.
    ///
    /// The editor raises it to the font's own row height when the setting asks for a tighter
    /// line than the glyphs need, so a 12 pt/100% choice cannot clip an ascender.
    pub fn row_height(&self) -> f32 {
        (self.size * self.line_height).max(self.size)
    }

    /// The settings value this typography was resolved from, for the pane header and a warning.
    pub fn summary(&self) -> String {
        format!(
            "{} {:.0}pt/{:.2}× liga {}",
            self.font.label(),
            self.size,
            self.line_height,
            if self.ligatures { "on" } else { "off" }
        )
    }
}

/// The font file for `font` under the Windows font directory, or `None` for the shipped face.
pub fn font_file(font: EditorFont) -> Option<&'static str> {
    match font {
        EditorFont::Hack => None,
        EditorFont::Cascadia => {
            if settings_font_exists("CascadiaCode.ttf") {
                Some("CascadiaCode.ttf")
            } else if settings_font_exists("CascadiaMono.ttf") {
                Some("CascadiaMono.ttf")
            } else {
                None
            }
        }
        EditorFont::Consolas => settings_font_exists("consola.ttf").then_some("consola.ttf"),
        EditorFont::CourierNew => settings_font_exists("cour.ttf").then_some("cour.ttf"),
        EditorFont::LucidaConsole => settings_font_exists("lucon.ttf").then_some("lucon.ttf"),
    }
}

impl EditorFont {
    /// True when the family's font file (or the shipped face) is available on this machine.
    pub fn available(self) -> bool {
        match self {
            EditorFont::Hack => true,
            other => font_file(other).is_some_and(settings_font_exists),
        }
    }
}

/// The stable egui font name for a family.
pub fn family_name(font: EditorFont) -> &'static str {
    match font {
        EditorFont::Cascadia => "editor-cascadia",
        EditorFont::Consolas => "editor-consolas",
        EditorFont::CourierNew => "editor-courier-new",
        EditorFont::LucidaConsole => "editor-lucida-console",
        EditorFont::Hack => "editor-hack",
    }
}

/// The Windows font directory (`%WINDIR%\Fonts`).
fn font_dir() -> PathBuf {
    let windir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    windir.join("Fonts")
}

/// True when the font file exists. Cached: the file system is asked once per name.
fn settings_font_exists(file: &'static str) -> bool {
    static EXISTS: OnceLock<Mutex<HashMap<&'static str, bool>>> = OnceLock::new();
    let cache = EXISTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(cached) = cache.lock().expect("font cache").get(file) {
        return *cached;
    }
    let exists = font_dir().join(file).is_file();
    cache.lock().expect("font cache").insert(file, exists);
    exists
}

/// The bytes of a font file, read once per process.
fn font_bytes(file: &'static str) -> Option<&'static [u8]> {
    static BYTES: OnceLock<Mutex<HashMap<&'static str, Option<&'static [u8]>>>> = OnceLock::new();
    let cache = BYTES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(cached) = cache.lock().expect("font bytes").get(file) {
        return *cached;
    }
    let path = font_dir().join(file);
    let bytes: Option<&'static [u8]> = std::fs::read(path).ok().map(|bytes| {
        // Leaked once per process: the font data lives for the life of the process by design,
        // and there are at most a handful of files.
        Box::leak(bytes.into_boxed_slice()) as &'static [u8]
    });
    cache.lock().expect("font bytes").insert(file, bytes);
    bytes
}

/// Registers the resolved family with the context, once.
///
/// The call is idempotent: egui already stores the font by name, and the name is stable per
/// family, so a second frame (or a second window on the same context) does no work. It runs from
/// the app's first pass before any text is laid out, so the editor never paints a frame with a
/// missing family. (`Context::add_font` applies immediately while no galley has been laid out in
/// the current pass, which is exactly when this is called.)
pub fn ensure_installed(ctx: &Context, typography: &TypographySettings) {
    let name = family_name(typography.font);
    let Some(file) = font_file(typography.font) else {
        return;
    };
    let installed = ctx.fonts(|fonts| fonts.definitions().font_data.contains_key(name));
    if installed {
        return;
    }
    let Some(bytes) = font_bytes(file) else {
        return;
    };
    let family = FontFamily::Name(name.into());
    ctx.add_font(FontInsert {
        name: name.to_string(),
        data: FontData::from_owned(bytes.to_vec()).into(),
        families: vec![InsertFontFamily {
            family,
            priority: FontPriority::Highest,
        }],
    });
}

/// True when `typography`'s family is registered in `ctx` (the test-facing probe).
#[cfg(test)]
pub fn is_installed(ctx: &Context, typography: &TypographySettings) -> bool {
    if typography.font == EditorFont::Hack {
        return true;
    }
    let name = family_name(typography.font);
    ctx.fonts(|fonts| fonts.definitions().font_data.contains_key(name))
}

/// A11Y-005: the display text the editor paints for one buffer line.
///
/// With `ligatures` off this is the line unchanged. With it on, each [`LIGATURES`] sequence
/// outside a string literal is replaced by its glyph plus spaces for the remaining cells, so
/// `source.chars().count()` is preserved exactly and every column-based decoration still points
/// at the same cell. `glyph_advance` measures the candidate glyph in the editor's font; a glyph
/// that is not exactly one cell wide (or that the font cannot draw) keeps the original pair, so
/// the paint is never misaligned by a fallback face's advance.
pub fn render_line(
    line: &str,
    ligatures: bool,
    cell: f32,
    glyph_advance: &mut impl FnMut(char) -> Option<f32>,
) -> String {
    if !ligatures {
        return line.to_string();
    }
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0usize;
    let mut quote: Option<u8> = None;
    while i < line.len() {
        let byte = bytes[i];
        if let Some(active) = quote {
            out.push(byte as char);
            i += 1;
            if byte == b'\\' {
                // Copy the escaped character verbatim; it cannot close the literal.
                if let Some(next) = line[i..].chars().next() {
                    out.push(next);
                    i += next.len_utf8();
                }
                continue;
            }
            if byte == active {
                quote = None;
            }
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            quote = Some(byte);
            out.push(byte as char);
            i += 1;
            continue;
        }
        let rest = &line[i..];
        let mut substituted = false;
        for (sequence, glyph) in LIGATURES {
            if !rest.starts_with(sequence) {
                continue;
            }
            // A substitution that is not exactly one cell wide would move every following
            // character off the cell grid, which is what the alignment tests forbid.
            let one_cell = glyph_advance(glyph).is_some_and(|advance| (advance - cell).abs() <= 0.5);
            if one_cell {
                out.push(glyph);
                for _ in 1..sequence.len() {
                    out.push(' ');
                }
                i += sequence.len();
                substituted = true;
                break;
            }
        }
        if substituted {
            continue;
        }
        let ch = rest.chars().next().expect("within the line");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{EditorFont, TypographySettings};

    fn typography(font: EditorFont, size: u32, line_height: u32, ligatures: bool) -> TypographySettings {
        TypographySettings {
            font,
            font_size_pt: size,
            line_height_percent: line_height,
            ligatures,
        }
    }

    /// A context with the font stack initialised; its first-pass texture deltas are consumed so a
    /// dropped `FullOutput` cannot trip epaint's texture-delta assertion.
    fn fresh_ctx() -> Context {
        let ctx = Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        output.textures_delta.clear();
        ctx
    }

    /// One empty pass: egui applies a font added by `Context::add_font` at the end of a pass, so
    /// a layout started in the same pass as the registration could still use the old stack.
    fn settle_fonts(ctx: &Context) {
        let mut output = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        output.textures_delta.clear();
    }

    /// Lays out `text` and returns `(char, advance_width)` per shaped glyph.
    fn glyphs(ctx: &Context, font_id: &FontId, text: &str) -> Vec<(char, f32)> {
        let job = egui::text::LayoutJob::simple_singleline(
            text.to_owned(),
            font_id.clone(),
            egui::Color32::WHITE,
        );
        let galley = ctx.fonts_mut(|fonts| fonts.layout_job(job));
        galley
            .rows
            .iter()
            .flat_map(|row| row.row.glyphs.iter())
            .map(|glyph| (glyph.chr, glyph.advance_width))
            .collect()
    }

    /// The advance of one character in the shipped monospace face at the code scale step.
    fn shipped_advance(character: char) -> Option<f32> {
        let ctx = fresh_ctx();
        let font = crate::ui::type_scale::Role::Code.font();
        glyphs(&ctx, &font, &character.to_string())
            .first()
            .map(|(_, advance)| *advance)
    }

    #[test]
    fn the_scale_factor_is_clamped_to_the_offered_range() {
        assert_eq!(scale_factor(100), 1.0);
        assert_eq!(scale_factor(125), 1.25);
        assert_eq!(scale_factor(150), 1.5);
        assert_eq!(scale_factor(200), 2.0);
        assert_eq!(scale_factor(0), 1.0, "below the range is the 100% floor");
        assert_eq!(scale_factor(900), 2.0, "above the range is the 200% ceiling");
    }

    #[test]
    fn row_height_is_size_times_line_height_and_never_clips_the_glyph() {
        for (size, percent, expected) in [
            (12, 100, 12.0),
            (18, 150, 27.0),
            (24, 200, 48.0),
            (13, 125, 16.25),
        ] {
            let settings = typography(EditorFont::Hack, size, percent, false);
            let resolved = Typography::new(&settings);
            assert_eq!(resolved.size, size as f32);
            assert_eq!(resolved.line_height, percent as f32 / 100.0);
            assert!(
                (resolved.row_height() - expected).abs() < 1e-6,
                "{size}pt at {percent}% must have row height {expected}, got {}",
                resolved.row_height()
            );
            assert!(
                resolved.row_height() >= resolved.size,
                "the row must be at least the glyph size"
            );
            assert_eq!(
                resolved.gutter_font().size,
                (size as f32 - 2.0).max(9.0),
                "the gutter number is two points smaller"
            );
        }
        // The 12-24 pt range the requirement names is exactly what the settings clamp to.
        let tiny = typography(EditorFont::Hack, 2, 100, false);
        assert_eq!(Typography::new(&tiny).size, 12.0);
        let huge = typography(EditorFont::Hack, 99, 100, false);
        assert_eq!(Typography::new(&huge).size, 24.0);
    }

    #[test]
    fn the_shipped_default_is_the_monospace_stack_and_needs_no_font_file() {
        let settings = TypographySettings::default();
        let resolved = Typography::new(&settings);
        assert_eq!(resolved.family, FontFamily::Monospace);
        assert_eq!(resolved.size, 13.0);
        assert!(!resolved.ligatures);
        assert_eq!(font_file(EditorFont::Hack), None);
        let ctx = fresh_ctx();
        ensure_installed(&ctx, &settings);
        assert!(is_installed(&ctx, &settings));
        let widths = glyphs(&ctx, &resolved.font_id(), "MM");
        assert_eq!(widths.len(), 2);
        assert!(widths[0].1 > 0.0, "the shipped face lays out an M");
    }

    #[test]
    fn every_offered_family_reports_whether_its_file_is_installed() {
        assert!(EditorFont::Hack.available(), "the shipped face is always available");
        for font in EditorFont::ALL {
            if font == EditorFont::Hack {
                continue;
            }
            assert_eq!(
                font.available(),
                font_file(font).is_some(),
                "{font:?}: availability must agree with the resolved file"
            );
            if let Some(file) = font_file(font) {
                assert!(
                    font_dir().join(file).is_file(),
                    "{file} must exist when it is reported as resolved"
                );
            }
        }
    }

    #[test]
    fn a_system_family_is_registered_and_used_when_installed() {
        if !EditorFont::Cascadia.available() {
            eprintln!("SKIP Cascadia is not installed on this machine");
            return;
        }
        let ctx = fresh_ctx();
        let settings = typography(EditorFont::Cascadia, 18, 125, false);
        ensure_installed(&ctx, &settings);
        assert!(is_installed(&ctx, &settings));
        settle_fonts(&ctx);
        let resolved = Typography::new(&settings);
        let widths = glyphs(&ctx, &resolved.font_id(), "M");
        assert_eq!(widths.len(), 1);
        assert!(widths[0].1 > 0.0, "the registered family lays out an M");
        // The character map is the direct observation that the named family resolves to the
        // inserted font, not to the shipped fallback.
        let owners = ctx.fonts_mut(|fonts| fonts.characters(&resolved.family).get(&'-').cloned());
        assert_eq!(
            owners,
            Some(vec![family_name(EditorFont::Cascadia).to_string()]),
            "the registered family owns the glyphs"
        );
    }

    #[test]
    fn the_ligature_table_is_one_cell_wide_in_the_shipped_face() {
        let cell = shipped_advance('M').expect("an M");
        for (sequence, glyph) in LIGATURES {
            let advance = shipped_advance(glyph);
            assert!(
                advance.is_some_and(|advance| (advance - cell).abs() <= 0.5),
                "{sequence} -> {glyph:?} must be drawable at exactly one cell (advance={advance:?}, cell={cell})"
            );
        }
    }

    #[test]
    fn the_ligature_setting_substitutes_pairs_without_changing_the_column_count() {
        let cell = shipped_advance('M').expect("an M");
        let mut advance = shipped_advance;
        let source = "a->b => c != d <= e >= f == g := h && i";
        let rendered = render_line(source, true, cell, &mut advance);
        assert_eq!(
            rendered.chars().count(),
            source.chars().count(),
            "the rendering must keep one char per buffer cell: {rendered:?}"
        );
        for (sequence, glyph) in LIGATURES {
            if source.contains(sequence) {
                assert!(
                    rendered.contains(glyph),
                    "{sequence} must render as {glyph:?}: {rendered:?}"
                );
                assert!(
                    !rendered.contains(sequence),
                    "{sequence} must not survive the substitution: {rendered:?}"
                );
            }
        }
        // Off is byte-identical.
        let plain = render_line(source, false, cell, &mut advance);
        assert_eq!(plain, source);
    }

    #[test]
    fn ligatures_never_rewrite_string_literals_or_escapes() {
        let cell = shipped_advance('M').expect("an M");
        let mut advance = shipped_advance;
        let source = r#"print("a->b") 'x != y' "esc \" ->" ->"#;
        let rendered = render_line(source, true, cell, &mut advance);
        assert!(
            rendered.contains("\"a->b\""),
            "a string literal must stay byte-exact: {rendered:?}"
        );
        assert!(
            rendered.contains("'x != y'"),
            "a single-quoted literal must stay byte-exact: {rendered:?}"
        );
        assert!(
            rendered.contains("esc \\\" ->"),
            "an escaped quote must not open a literal: {rendered:?}"
        );
        assert!(
            rendered.ends_with("\u{2192} "),
            "the operator after the literal is still substituted: {rendered:?}"
        );
    }

    #[test]
    fn a_glyph_that_is_not_one_cell_keeps_the_original_pair() {
        let cell = shipped_advance('M').expect("an M");
        // A probe that reports a two-cell advance must refuse every substitution.
        let mut advance = |_glyph: char| Some(cell * 2.0);
        assert_eq!(render_line("a->b", true, cell, &mut advance), "a->b");
        // A probe that cannot draw the glyph keeps the pair too.
        let mut missing = |_glyph: char| None;
        assert_eq!(render_line("a->b", true, cell, &mut missing), "a->b");
        assert_eq!(render_line("a->b", false, cell, &mut missing), "a->b");
    }
}
