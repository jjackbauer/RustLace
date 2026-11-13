//! A11Y-014: one ASCII/Unicode setting for every rendered mathematical glyph.
//!
//! The engine's own display formatter is ASCII (`sqrt(2)*pi <= inf`); its optional Unicode
//! printer differs only in the named constants and the relation tokens (`√(2)×π ≤ ∞`). The
//! window renders values that arrive as display strings, so the setting is applied here, in one
//! place, to every surface that shows a value: the structured RESULT tree, the Workspace tables,
//! the Logs result lines and the Inspector expression tree. Nothing in this module knows about
//! egui or the engine: it is a pure text transform plus a font-coverage probe supplied by the
//! caller, so it is testable without a window and the four surfaces cannot disagree.
//!
//! # The documented fallback (no tofu boxes)
//!
//! Rendering one string goes through exactly four steps, in this order:
//!
//! 1. **Mode mapping.** [`GlyphMode::Unicode`] rewrites the ASCII mathematical idioms the engine
//!    emits (`!=`, `<=`, `>=`, `sqrt(`, the standalone constants `pi`/`inf`, and `*` between two
//!    operands) into `≠ ≤ ≥ √ π ∞ ×`. [`GlyphMode::Ascii`] rewrites those Unicode glyphs (and the
//!    rest of the table below) back into ASCII. Natural-language text is never transliterated:
//!    `café` stays `café` in both modes.
//! 2. **Font coverage.** The caller supplies `has_glyph`: a character the installed font can draw
//!    is kept as it is.
//! 3. **Documented fallback.** A character the font cannot draw is replaced by the ASCII spelling
//!    in [`FALLBACKS`] — `×` becomes `*`, `√` becomes `sqrt`, `π` becomes `pi`, `∞` becomes `inf`.
//!    The replacement is part of this module's contract and is listed in the Help > Keyboard
//!    Shortcuts dialog and in `docs/goal-desktop-v4/round-42/accessibility.md`.
//! 4. **Last resort.** A character that is neither drawable nor in the table (for example an
//!    emoji from a print line, or a mathematical-alphanumeric code point outside the table)
//!    becomes `?`. A visible question mark is the documented degradation; a tofu box is never
//!    drawn by this path because the glyph is never handed to the font.
//!
//! Because step 3 consults the table rather than a second copy of the mode mapping, a glyph set
//! collected from a real engine sample can be checked one character at a time: every character is
//! either drawable or has a mapping, which is the no-tofu assertion in the tests below and in
//! `ui::a11y`.

use std::collections::BTreeSet;

/// Which mathematical notation the window renders values in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GlyphMode {
    /// ASCII notation, the engine's own default: `sqrt(2)*pi <= inf`.
    #[default]
    Ascii,
    /// Unicode notation: `√(2)×π ≤ ∞`.
    Unicode,
}

impl GlyphMode {
    /// The mode for a boolean setting (`true` = Unicode).
    pub fn from_unicode(enabled: bool) -> GlyphMode {
        if enabled {
            GlyphMode::Unicode
        } else {
            GlyphMode::Ascii
        }
    }

    /// The setting value this mode persists as.
    #[cfg_attr(not(test), allow(dead_code))] // asserted by the mode round-trip test
    pub fn unicode(self) -> bool {
        matches!(self, GlyphMode::Unicode)
    }

    /// The stable name used by logs and tests.
    pub fn name(self) -> &'static str {
        match self {
            GlyphMode::Ascii => "ascii",
            GlyphMode::Unicode => "unicode",
        }
    }

    /// The label the settings control shows.
    pub fn label(self) -> &'static str {
        match self {
            GlyphMode::Ascii => "ASCII (sqrt(2)*pi)",
            GlyphMode::Unicode => "Unicode (√(2)×π)",
        }
    }

    /// The other mode.
    #[cfg_attr(not(test), allow(dead_code))] // asserted by the mode round-trip test
    pub fn toggled(self) -> GlyphMode {
        match self {
            GlyphMode::Ascii => GlyphMode::Unicode,
            GlyphMode::Unicode => GlyphMode::Ascii,
        }
    }
}

/// The documented fallback spelling of every mathematical glyph this window renders.
///
/// This is the single table used both to leave Unicode mode and to replace a glyph the installed
/// font cannot draw. It contains the glyphs the engine's Unicode printer emits (`≠ ≤ ≥ π ∞`), the
/// glyphs this window introduces in Unicode mode (`× √`), and the astral mathematical-alphanumeric
/// code points the engine's symbol names could be rendered with. A code point that is not listed
/// and is not drawable renders as `?`; it is never handed to the font as tofu.
pub const FALLBACKS: &[(char, &str)] = &[
    // Operators and relations.
    ('×', "*"),
    ('·', "*"),
    ('≤', "<="),
    ('≥', ">="),
    ('≠', "!="),
    ('≈', "~="),
    ('±', "+-"),
    ('−', "-"),
    ('→', "->"),
    ('∈', "in"),
    // Named constants and radicals.
    ('π', "pi"),
    ('∞', "inf"),
    ('√', "sqrt"),
    ('Σ', "sum"),
    ('∫', "integral"),
    ('∅', "empty"),
    ('∂', "d"),
    ('∆', "delta"),
    ('Δ', "delta"),
    // Superscripts the formatter can produce from a display string.
    ('¹', "^1"),
    ('²', "^2"),
    ('³', "^3"),
    // Punctuation that must never come from a font that lacks it.
    ('…', "..."),
    ('°', "deg"),
    ('µ', "u"),
    ('“', "\""),
    ('”', "\""),
    ('‘', "'"),
    ('’', "'"),
    ('–', "-"),
    ('—', "-"),
    // Astral Mathematical Alphanumeric Symbols (U+1D400-U+1D7FF). The engine's printer is ASCII
    // today; these entries are what makes an astral glyph reachable from a user value without a
    // tofu box, and they are the characters the A11Y-014 tests pin.
    ('\u{1D44E}', "a"),       // 𝑥-adjacent: MATHEMATICAL ITALIC SMALL A
    ('\u{1D456}', "i"),       // MATHEMATICAL ITALIC SMALL I
    ('\u{1D465}', "x"),       // MATHEMATICAL ITALIC SMALL X
    ('\u{1D54F}', "X"),       // MATHEMATICAL DOUBLE-STRUCK CAPITAL X
    ('\u{1D703}', "theta"),   // MATHEMATICAL ITALIC SMALL THETA
    ('\u{1D70B}', "pi"),      // MATHEMATICAL ITALIC SMALL PI
    ('\u{1D70E}', "sigma"),   // MATHEMATICAL ITALIC SMALL SIGMA
    ('\u{1D70F}', "tau"),     // MATHEMATICAL ITALIC SMALL TAU
];

/// The fallback spelling of `c`, if one is documented.
pub fn fallback(c: char) -> Option<&'static str> {
    FALLBACKS
        .iter()
        .find(|(glyph, _)| *glyph == c)
        .map(|(_, replacement)| *replacement)
}

/// True when `c` needs a glyph lookup at all: printable, non-ASCII characters do; ASCII and the
/// control characters a value can carry do not.
fn needs_glyph(c: char) -> bool {
    !c.is_ascii() && !c.is_control()
}

/// The ASCII spelling of a value that may contain Unicode mathematical glyphs.
///
/// Only the glyphs in [`FALLBACKS`] are transliterated; every other character (including
/// natural-language text such as `café`) is left untouched.
pub fn to_ascii(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match fallback(c) {
            Some(replacement) => out.push_str(replacement),
            None => out.push(c),
        }
    }
    out
}

/// True when `chars[index]` begins a whole word `word` (a left boundary check).
fn word_at(chars: &[char], index: usize, word: &str) -> bool {
    let length = word.chars().count();
    if index + length > chars.len() {
        return false;
    }
    let left_ok = index == 0 || !is_word_char(chars[index - 1]);
    let right_ok = index + length == chars.len() || !is_word_char(chars[index + length]);
    left_ok
        && right_ok
        && word
            .chars()
            .enumerate()
            .all(|(offset, expected)| chars[index + offset] == expected)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// True when the character at `index` (or the nearest non-space to its left) can end an operand.
fn operand_before(chars: &[char], index: usize) -> bool {
    let mut cursor = index;
    while cursor > 0 {
        cursor -= 1;
        let c = chars[cursor];
        if c == ' ' {
            continue;
        }
        return c.is_alphanumeric() || matches!(c, ')' | ']' | '.' | '%' | '}' | '\'' | '"');
    }
    false
}

/// True when the character at `index` (or the nearest non-space to its right) can start an
/// operand.
fn operand_after(chars: &[char], index: usize) -> bool {
    let mut cursor = index + 1;
    while cursor < chars.len() {
        let c = chars[cursor];
        if c == ' ' {
            cursor += 1;
            continue;
        }
        return c.is_alphanumeric() || matches!(c, '(' | '[' | '.' | '{' | '\'' | '"');
    }
    false
}

/// The Unicode spelling of an ASCII value: the inverse of [`to_ascii`], plus `×` and `√`.
///
/// The rewrite is a single left-to-right pass so a produced glyph can never be rewritten again.
pub fn to_unicode(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        let next = chars.get(index + 1).copied();
        let relation = next.and_then(|n| match (c, n) {
            ('!', '=') => Some('≠'),
            ('<', '=') => Some('≤'),
            ('>', '=') => Some('≥'),
            _ => None,
        });
        if let Some(glyph) = relation {
            out.push(glyph);
            index += 2;
            continue;
        }
        // `sqrt(` is the engine's radical spelling. The left boundary keeps a user symbol such
        // as `mysqrt(` from being rewritten.
        if c == 's'
            && word_at(&chars, index, "sqrt")
            && chars.get(index + 4) == Some(&'(')
            && (index == 0 || !is_word_char(chars[index - 1]))
        {
            out.push('√');
            index += 4;
            continue;
        }
        if c == 'p' && word_at(&chars, index, "pi") {
            out.push('π');
            index += 2;
            continue;
        }
        if c == 'i' && word_at(&chars, index, "inf") {
            out.push('∞');
            index += 3;
            continue;
        }
        if c == '*' && operand_before(&chars, index) && operand_after(&chars, index) {
            out.push('×');
            index += 1;
            continue;
        }
        out.push(c);
        index += 1;
    }
    out
}

/// Renders `text` for display in `mode`, replacing a glyph the font cannot draw with its
/// documented fallback (or `?` when none exists).
///
/// `has_glyph` is the caller's font-coverage probe. It is only called for printable non-ASCII
/// characters, so the ASCII fast path costs one branch per character.
pub fn render(text: &str, mode: GlyphMode, mut has_glyph: impl FnMut(char) -> bool) -> String {
    let mapped = match mode {
        GlyphMode::Ascii => to_ascii(text),
        GlyphMode::Unicode => to_unicode(text),
    };
    let mut out = String::with_capacity(mapped.len());
    for c in mapped.chars() {
        if !needs_glyph(c) || has_glyph(c) {
            out.push(c);
            continue;
        }
        match fallback(c) {
            Some(replacement) => out.push_str(replacement),
            None => out.push('?'),
        }
    }
    out
}

/// Every non-ASCII character in `text`, sorted, for a glyph coverage report.
pub fn non_ascii(text: &str) -> BTreeSet<char> {
    text.chars().filter(|c| !c.is_ascii()).collect()
}

/// The characters of `text` that neither the probe nor [`FALLBACKS`] can render.
///
/// An empty result is the no-tofu condition: every glyph the text asks for is drawable or has a
/// documented replacement.
#[cfg_attr(not(test), allow(dead_code))] // the no-tofu probe's failing case, pinned by a test
pub fn uncovered(text: &str, mut has_glyph: impl FnMut(char) -> bool) -> Vec<char> {
    let mut missing = Vec::new();
    for c in non_ascii(text) {
        if has_glyph(c) || fallback(c).is_some() {
            continue;
        }
        missing.push(c);
    }
    missing
}

/// The glyph set the A11Y-014 sample exercises: the engine's Unicode printer's glyphs plus the two
/// this window adds and one astral mathematical italic character.
pub const SAMPLE_GLYPHS: &str = "×≤≥≠√π∞𝜋";

#[cfg(test)]
mod tests {
    use super::*;

    fn ascii_only(c: char) -> bool {
        c.is_ascii()
    }

    #[test]
    fn unicode_mode_produces_the_engine_printer_glyphs_plus_times_and_radical() {
        let rendered = to_unicode("sqrt(2)*pi != 0 <= inf");
        assert_eq!(rendered, "√(2)×π ≠ 0 ≤ ∞");
        assert!(rendered.contains('×'));
        assert!(rendered.contains('≤'));
        assert!(rendered.contains('√'));
        assert!(rendered.contains('π'));
    }

    #[test]
    fn ascii_mode_is_the_engine_default_and_maps_every_glyph_back() {
        assert_eq!(to_ascii("√(2)×π ≤ ∞ ≠ 0"), "sqrt(2)*pi <= inf != 0");
        assert_eq!(to_ascii("3·x"), "3*x");
        assert_eq!(to_unicode("3*x^2"), "3×x^2");
    }

    #[test]
    fn natural_language_text_is_not_transliterated() {
        // `pixel`, `pin` and `info` are words, not the named constants.
        assert_eq!(to_unicode("pixel = pin + info"), "pixel = pin + info");
        assert_eq!(to_ascii("café = 1"), "café = 1");
        assert_eq!(to_unicode("mysqrt(x)"), "mysqrt(x)");
    }

    #[test]
    fn the_relation_rewrite_covers_tight_and_spaced_forms() {
        assert_eq!(to_unicode("a<=b"), "a≤b");
        assert_eq!(to_unicode("a >= b"), "a ≥ b");
        assert_eq!(to_unicode("x!=y"), "x≠y");
        assert_eq!(to_unicode("a == b"), "a == b");
    }

    #[test]
    fn both_modes_round_trip_the_engine_sample() {
        let ascii = "sqrt(2)*pi != 0 <= inf";
        let unicode = to_unicode(ascii);
        assert_eq!(to_ascii(&unicode), ascii);
        assert_eq!(to_ascii(&to_unicode(ascii)), ascii);
    }

    #[test]
    fn a_glyph_the_font_lacks_takes_its_documented_fallback() {
        // The probe accepts ASCII only: every Unicode glyph must degrade to its table spelling.
        let rendered = render("√(2)×π ≤ ∞", GlyphMode::Unicode, ascii_only);
        assert_eq!(rendered, "sqrt(2)*pi <= inf");
        assert!(rendered.is_ascii(), "no tofu: {rendered:?}");
    }

    #[test]
    fn an_undocumented_glyph_becomes_a_question_mark_and_never_reaches_the_font() {
        // U+2603 SNOWMAN is not in the fallback table and the probe refuses it.
        let rendered = render("snow \u{2603}", GlyphMode::Unicode, ascii_only);
        assert_eq!(rendered, "snow ?");
        assert!(!rendered.contains('\u{2603}'));
        assert!(!rendered.contains('\u{FFFD}'));
    }

    #[test]
    fn an_astral_mathematical_glyph_is_mapped_when_the_font_lacks_it() {
        // U+1D70B MATHEMATICAL ITALIC SMALL PI is astral (outside the BMP).
        let astral = '\u{1D70B}';
        assert!(!astral.is_ascii() && astral as u32 > 0xFFFF);
        assert_eq!(render(&format!("{astral} = 3"), GlyphMode::Unicode, ascii_only), "pi = 3");
        assert_eq!(render(&format!("{astral} = 3"), GlyphMode::Ascii, ascii_only), "pi = 3");
        // With a font that has it, Unicode mode keeps it verbatim.
        assert_eq!(render(&format!("{astral} = 3"), GlyphMode::Unicode, |_| true), format!("{astral} = 3"));
    }

    #[test]
    fn the_no_tofu_probe_reports_nothing_for_the_sample_glyphs() {
        // Any font at all: the table covers every sample glyph.
        assert!(uncovered(SAMPLE_GLYPHS, |_| false).is_empty());
        // With the shipped-font probe (the ui test supplies the real one), still nothing.
        assert!(uncovered(SAMPLE_GLYPHS, |c| c == '×' || c == '≤').is_empty());
    }

    #[test]
    fn the_uncovered_report_names_a_glyph_with_no_mapping() {
        assert_eq!(uncovered("√ π \u{2603}", |_| false), vec!['\u{2603}']);
    }

    #[test]
    fn every_documented_fallback_spelling_is_itself_renderable_ascii() {
        for (glyph, replacement) in FALLBACKS {
            assert!(
                replacement.is_ascii(),
                "{glyph:?} maps to non-ASCII {replacement:?}"
            );
            assert!(!replacement.is_empty());
            // A fallback must not be a glyph that needs another fallback.
            for c in replacement.chars() {
                assert!(c.is_ascii(), "{replacement:?} contains {c:?}");
            }
        }
    }

    #[test]
    fn the_mode_setting_round_trips() {
        assert_eq!(GlyphMode::from_unicode(false), GlyphMode::Ascii);
        assert_eq!(GlyphMode::from_unicode(true), GlyphMode::Unicode);
        assert!(GlyphMode::Unicode.unicode());
        assert!(!GlyphMode::Ascii.unicode());
        assert_eq!(GlyphMode::Ascii.toggled(), GlyphMode::Unicode);
        assert_eq!(GlyphMode::default(), GlyphMode::Ascii, "the engine's own default");
        assert_eq!(GlyphMode::Ascii.name(), "ascii");
        assert_eq!(GlyphMode::Unicode.name(), "unicode");
    }

    #[test]
    fn render_is_idempotent_in_ascii_mode() {
        let once = render("√(2)×π ≤ ∞", GlyphMode::Ascii, ascii_only);
        assert_eq!(render(&once, GlyphMode::Ascii, ascii_only), once);
    }
}
