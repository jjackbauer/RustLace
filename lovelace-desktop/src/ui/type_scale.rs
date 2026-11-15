//! VIS-004 / VIS-005: the workbench's one documented type scale.
//!
//! Every text role the window paints is a [`Role`]. A role owns a size, a weight family and an
//! intended line spacing, and [`Role::font`] is the only way a paint routine may build a
//! [`FontId`] for UI text. `style.text_styles` is filled from the same table ([`install`]), so a
//! `.small()` / `.monospace()` / default label cannot fall back to egui's 9/13/18 defaults: the
//! smallest text the window can paint is [`CAPTION_PT`] (11 pt), not egui's 9 pt.
//!
//! # The scale
//!
//! | role | pt | weight | line | used by |
//! | --- | --- | --- | --- | --- |
//! | [`Role::Title`] | 20 | strong | 24 | the product title, dialog headings |
//! | [`Role::PaneTitle`] | 15 | strong | 18 | every pane header, empty-state headlines |
//! | [`Role::Body`] | 13 | regular | 15 | labels, buttons, table cells, prose |
//! | [`Role::Code`] | 13 | regular (monospace) | 16.25 | code, output, logs, inspector payloads |
//! | [`Role::Caption`] | 11 | regular | 12.65 | metadata, hints, legends, count lines |
//! | [`Role::Badge`] | 11 | strong | 13 | status markers and the compact markers (`[OK]`, `[ERR]`, `!`) |
//!
//! # Weights
//!
//! egui 0.37 has no per-run OpenType weight switch, so a weight is a resolved *family*: the
//! strong roles use a registered semibold face (`seguisb.ttf`, falling back to `segoeuib.ttf`)
//! and the regular roles use egui's proportional family. The face is registered by
//! [`ensure_installed`] on the first pass, before any galley is laid out ([`crate::ui::app`]
//! calls it at the top of the frame, exactly like the editor font). A machine without either
//! face resolves strong roles to the proportional family and still paints — the scale degrades
//! in weight, never in size.
//!
//! # Why an audit test
//!
//! A scale is only a hierarchy if nothing bypasses it. [`tests::the_ui_source_sets_no_font_size_outside_the_scale`]
//! reads every production `src/ui/*.rs` file at test time and fails on a numeric literal passed
//! to `FontId::proportional`, `FontId::monospace`, `FontId::new` or `RichText::size`. An ad-hoc
//! size therefore fails the suite instead of quietly shipping.

use std::sync::OnceLock;

use egui::{Context, FontData, FontFamily, FontId, FontInsert, FontPriority, InsertFontFamily};

/// VIS-004: the display title size, in points.
pub const TITLE_PT: f32 = 20.0;
/// VIS-004: the pane-header title size, in points.
pub const PANE_TITLE_PT: f32 = 15.0;
/// VIS-004/VIS-005: the body size, in points. The legibility floor for prose and controls.
pub const BODY_PT: f32 = 13.0;
/// VIS-004/VIS-005: the monospaced code/output size, in points. The legibility floor for code.
pub const CODE_PT: f32 = 13.0;
/// VIS-004/VIS-005: the caption/metadata size, in points: the smallest text the UI paints.
pub const CAPTION_PT: f32 = 11.0;
/// VIS-004: the badge/status-marker size, in points.
pub const BADGE_PT: f32 = 11.0;

/// VIS-005: the smallest size any role may paint. A role below this fails the legibility check.
pub const MIN_RENDERED_PT: f32 = CAPTION_PT;

/// VIS-004: the title's intended line spacing, in points.
pub const TITLE_LINE_PT: f32 = 24.0;
/// VIS-004: the pane title's intended line spacing, in points.
pub const PANE_TITLE_LINE_PT: f32 = 18.0;
/// VIS-004: the body's shortest acceptable line spacing, in points: the measured row of the
/// shipped proportional face at 13 pt (15.0 pt = 1.15x). Paragraphs that wrap ask for the
/// prose ratio explicitly ([`PROSE_LINE_RATIO`]).
pub const BODY_LINE_PT: f32 = 15.0;
/// VIS-004/VIS-005: the code's intended line spacing, in points. Equals the editor's default
/// row height (13 pt at the default 125% line height setting).
pub const CODE_LINE_PT: f32 = 16.25;
/// VIS-004: the caption's shortest acceptable line spacing, in points (11 pt at 1.15x).
pub const CAPTION_LINE_PT: f32 = 12.65;
/// VIS-004: the badge's shortest acceptable line spacing, in points.
pub const BADGE_LINE_PT: f32 = 13.0;

/// The minimum line spacing as a multiple of the font size (VIS-005).
///
/// 1.15 is the row the proportional face actually lays out for a single-line label; a role below
/// it would have its ascenders and descenders touch. Wrapped prose uses [`PROSE_LINE_RATIO`]
/// explicitly, and the editor's code rows use the user's 100-200% line-height setting (1.25 at the
/// shipped default).
pub const MIN_LINE_RATIO: f32 = 1.15;

/// VIS-005: the line spacing used for wrapped prose (empty-state guidance, dialogs) so a
/// multi-line paragraph is not set as tightly as a one-line label.
pub const PROSE_LINE_RATIO: f32 = 1.35;

/// The registered name of the semibold face used by the strong roles.
pub const STRONG_FAMILY: &str = "ui-strong";

/// The semibold faces a strong role may be painted with, in preference order.
///
/// `seguisb.ttf`/`segoeuib.ttf` are the Windows faces. `NimbusSans-Bold.otf` is the URW base35
/// Helvetica clone that the standard Linux font set (ghostscript's `fonts-urw-base35`) installs;
/// it is the Linux strong face because its 1.20 em line box meets VIS-005's 1.15x minimum at
/// every strong size. `DejaVuSans-Bold.ttf` is the last resort: DejaVu is ubiquitous but its
/// 1.164 em line box rounds to 17 px at the 15 pt pane title, below the scale's minimum, so it
/// is only used when nothing taller is installed.
const STRONG_FACES: [&'static str; 4] = [
    "seguisb.ttf",
    "segoeuib.ttf",
    "NimbusSans-Bold.otf",
    "DejaVuSans-Bold.ttf",
];

/// The weight axis of the scale.
///
/// `Strong` is a real semibold face when the machine has one; `Regular` is egui's proportional
/// family. The enum is recorded per role so a test (and the manual) state the intended weight,
/// not just the size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    /// The regular face.
    Regular,
    /// The semibold face, or the regular family when no semibold file is installed.
    Strong,
}

/// One text role in the scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// The display title (product title, dialog heading, empty-state headline).
    Title,
    /// A pane header title.
    PaneTitle,
    /// Body text: labels, buttons, table cells.
    Body,
    /// Monospaced code and output.
    Code,
    /// Captions, metadata and hints.
    Caption,
    /// Status badges and markers.
    Badge,
}

impl Role {
    /// Every role, in descending size order.
    pub const ALL: [Role; 6] = [
        Role::Title,
        Role::PaneTitle,
        Role::Body,
        Role::Code,
        Role::Caption,
        Role::Badge,
    ];

    /// The stable role name used by the audit report and the legibility CLI.
    pub fn name(self) -> &'static str {
        match self {
            Role::Title => "title",
            Role::PaneTitle => "pane-title",
            Role::Body => "body",
            Role::Code => "code",
            Role::Caption => "caption",
            Role::Badge => "badge",
        }
    }

    /// The role's size in points.
    pub fn pt(self) -> f32 {
        match self {
            Role::Title => TITLE_PT,
            Role::PaneTitle => PANE_TITLE_PT,
            Role::Body => BODY_PT,
            Role::Code => CODE_PT,
            Role::Caption => CAPTION_PT,
            Role::Badge => BADGE_PT,
        }
    }

    /// The role's weight axis.
    pub fn weight(self) -> Weight {
        match self {
            Role::Title | Role::PaneTitle | Role::Badge => Weight::Strong,
            Role::Body | Role::Code | Role::Caption => Weight::Regular,
        }
    }

    /// The role's intended line spacing in points.
    pub fn line_pt(self) -> f32 {
        match self {
            Role::Title => TITLE_LINE_PT,
            Role::PaneTitle => PANE_TITLE_LINE_PT,
            Role::Body => BODY_LINE_PT,
            Role::Code => CODE_LINE_PT,
            Role::Caption => CAPTION_LINE_PT,
            Role::Badge => BADGE_LINE_PT,
        }
    }

    /// The role's intended line spacing as a multiple of its size.
    pub fn line_ratio(self) -> f32 {
        self.line_pt() / self.pt()
    }

    /// The role's resolved font. This is the only font constructor the UI uses for text.
    pub fn font(self) -> FontId {
        let family = match self {
            Role::Code => FontFamily::Monospace,
            role if role.weight() == Weight::Strong => strong_family(),
            _ => FontFamily::Proportional,
        };
        FontId::new(self.pt(), family)
    }

    /// VIS-005: a wrapped paragraph's line spacing in points (1.35x). Use it on the
    /// `RichText` of guidance text that may wrap, so a paragraph is not set at label spacing.
    pub fn prose_line_pt(self) -> f32 {
        self.pt() * PROSE_LINE_RATIO
    }
}

/// True when the machine has a semibold face for the strong roles.
pub fn strong_face_available() -> bool {
    strong_bytes().is_some()
}

/// The family strong roles paint with: the registered semibold face, or the regular family.
pub fn strong_family() -> FontFamily {
    if strong_face_available() {
        FontFamily::Name(STRONG_FAMILY.into())
    } else {
        FontFamily::Proportional
    }
}

/// The bytes of the first installed semibold face, read once per process.
fn strong_bytes() -> Option<&'static [u8]> {
    static BYTES: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    BYTES
        .get_or_init(|| {
            STRONG_FACES.iter().find_map(|name| {
                super::typography::find_font_file(name).and_then(|path| std::fs::read(path).ok())
            })
        })
        .as_deref()
}

/// Registers the semibold face with `ctx` once, before the first galley of the process.
///
/// It is called at the top of [`crate::ui::app`]'s frame, beside the editor font registration.
/// A second call is a no-op (egui stores the font by name). When the machine has no semibold
/// face there is nothing to register and strong roles paint with the proportional family.
pub fn ensure_installed(ctx: &Context) {
    let Some(bytes) = strong_bytes() else {
        return;
    };
    let installed = ctx.fonts(|fonts| fonts.definitions().font_data.contains_key(STRONG_FAMILY));
    if installed {
        return;
    }
    ctx.add_font(FontInsert {
        name: STRONG_FAMILY.to_string(),
        data: FontData::from_owned(bytes.to_vec()).into(),
        families: vec![InsertFontFamily {
            family: FontFamily::Name(STRONG_FAMILY.into()),
            priority: FontPriority::Highest,
        }],
    });
}

/// VIS-004: fills a style's text styles from the scale.
///
/// After this call `.small()` is 11 pt (not egui's 9), a default label is 13 pt, monospace is
/// 13 pt and `.heading()` is the 20 pt title. The workbench rarely uses the style lookup
/// directly — the panes call [`Role::font`] — but every egui widget that does (tooltips,
/// buttons, text edits, menus) inherits the scale through this function.
pub fn install(style: &mut egui::Style) {
    use egui::TextStyle;
    style
        .text_styles
        .insert(TextStyle::Heading, Role::Title.font());
    style.text_styles.insert(TextStyle::Body, Role::Body.font());
    style
        .text_styles
        .insert(TextStyle::Button, Role::Body.font());
    style
        .text_styles
        .insert(TextStyle::Monospace, Role::Code.font());
    style
        .text_styles
        .insert(TextStyle::Small, Role::Caption.font());
}

/// A monospaced badge glyph fitted into `height` points, never above the scale's badge size.
///
/// The editor's gutter marker is a fixed 8-10 pt square; the glyph it draws is the badge role
/// clamped to that square. Clamping is a derived fit, not a second scale: the size never
/// exceeds [`BADGE_PT`] and never drops the badge below 80% of it.
pub fn fitted_badge(height: f32) -> FontId {
    let size = (height * 0.9).clamp(BADGE_PT * 0.8, BADGE_PT);
    FontId::new(size, FontFamily::Monospace)
}

/// VIS-005: a role's foreground and the surfaces it is measured against.
///
/// The legibility CLI and its tests read this table, so the measured pairs are the painted
/// pairs: body/title/code use the primary text token, captions the secondary token, and every
/// role appears on the pane body at least.
pub fn role_contrast_pairs(role: Role) -> [(&'static str, egui::Color32, egui::Color32); 2] {
    let fg = match role {
        Role::Caption => crate::ui::theme::dim(),
        _ => crate::ui::theme::text(),
    };
    [
        ("pane body", fg, crate::ui::theme::panel_bg()),
        ("pane header", fg, crate::ui::theme::header_bg()),
    ]
}

/// VIS-005: the legibility report for the active theme.
///
/// It measures the rendered galley of one sample string per role at 96 DPI (the capture DPI),
/// reports the role's size, measured row height, line ratio and contrast against the two
/// surfaces it paints on, and returns `(text, passed)`. `--legibility-check` prints it and exits
/// with `passed`.
pub fn legibility_report(ctx: &Context) -> (String, bool) {
    use egui::text::LayoutJob;
    ensure_installed(ctx);
    let mut text = String::new();
    let mut passed = true;
    text.push_str(&format!(
        "lovelace-desktop legibility (VIS-004/VIS-005) theme={:?} dpi=96 smallest={} {:.1} pt\n",
        crate::ui::theme::active(),
        smallest_role().name(),
        smallest_role().pt()
    ));
    text.push_str("role | pt | weight | family | line target | measured row | ratio | contrast on pane body | contrast on pane header\n");
    for role in Role::ALL {
        let font = role.font();
        let job = LayoutJob::simple_singleline(
            "Hamburgefonstiv 0123456789".to_string(),
            font.clone(),
            egui::Color32::WHITE,
        );
        let galley = ctx.fonts_mut(|fonts| fonts.layout_job(job));
        let measured = galley.size().y;
        let ratio = measured / role.pt();
        let mut contrasts = Vec::new();
        for (surface, fg, bg) in role_contrast_pairs(role) {
            let value = crate::ui::theme::contrast_ratio(fg, bg);
            let required = crate::ui::theme::PairClass::NormalText.required();
            let ok = value + 1e-9 >= required;
            passed &= ok;
            contrasts.push(format!(
                "{surface} {value:.2}:1{}",
                if ok { "" } else { " FAIL" }
            ));
        }
        let size_ok = role.pt() + 1e-9 >= MIN_RENDERED_PT;
        let line_ok = ratio + 1e-9 >= MIN_LINE_RATIO;
        passed &= size_ok && line_ok;
        let family = match font.family {
            FontFamily::Monospace => "monospace",
            FontFamily::Proportional => "proportional",
            _ => "semibold",
        };
        text.push_str(&format!(
            "{} | {:.1} | {:?} | {} | {:.2} pt | {:.2} pt | {:.2}x{} | {} | {}\n",
            role.name(),
            role.pt(),
            role.weight(),
            family,
            role.line_pt(),
            measured,
            ratio,
            if line_ok { "" } else { " FAIL" },
            contrasts[0],
            contrasts[1],
        ));
    }
    // The editor's monospaced code is configurable (12-24 pt); the chart below states the floor
    // the shipped default resolves to and the smallest size the setting can select.
    let default_code = crate::settings::TypographySettings::default();
    let min_code = crate::settings::MIN_FONT_SIZE_PT as f32;
    let default_code_pt = default_code.font_size();
    let code_floor_ok = default_code_pt >= CODE_PT && min_code >= CAPTION_PT;
    passed &= code_floor_ok;
    text.push_str(&format!(
        "editor code: default {default_code_pt:.1} pt / {:.2} pt row ({:.2}x); smallest selectable {:.1} pt\n",
        default_code.row_height(),
        default_code.row_height() / default_code_pt,
        min_code
    ));
    text.push_str(&format!(
        "smallest rendered text: {} at {:.1} pt = {:.1} px at 96 DPI; contrast {} / {}\n",
        smallest_role().name(),
        smallest_role().pt(),
        smallest_role().pt(),
        format_contrast(smallest_role(), 0),
        format_contrast(smallest_role(), 1),
    ));
    text.push_str(if passed {
        "RESULT PASS\n"
    } else {
        "RESULT FAIL\n"
    });
    (text, passed)
}

fn format_contrast(role: Role, index: usize) -> String {
    let (surface, fg, bg) = role_contrast_pairs(role)[index];
    format!(
        "{:.2}:1 on {surface}",
        crate::ui::theme::contrast_ratio(fg, bg)
    )
}

/// The smallest role in the scale; the legibility claim names this one.
pub fn smallest_role() -> Role {
    Role::ALL
        .into_iter()
        .min_by(|a, b| a.pt().partial_cmp(&b.pt()).expect("finite sizes"))
        .expect("the scale is not empty")
}

/// `--legibility-check`: prints [`legibility_report`] for both themes and exits non-zero when a
/// role is below the size floor, below the line-spacing floor, or below WCAG AA on a surface it
/// paints on.
pub fn legibility_cli() -> i32 {
    let ctx = Context::default();
    // A context has no font atlas until its first pass; initialise it, register the semibold
    // face and let egui apply it before laying out the measured samples.
    let mut output = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    output.textures_delta.clear();
    ensure_installed(&ctx);
    let mut output = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    output.textures_delta.clear();
    let mut text = String::new();
    let mut passed = true;
    for theme in [
        crate::ui::theme::ThemeId::Dark,
        crate::ui::theme::ThemeId::Light,
    ] {
        crate::ui::theme::set_active(theme);
        let mut style = egui::Style::default();
        crate::ui::theme::apply_style(&mut style, theme);
        let (report, ok) = legibility_report(&ctx);
        text.push_str(&report);
        passed &= ok;
    }
    print!("{text}");
    if passed { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A context whose first pass consumed the texture deltas, so a dropped `FullOutput` cannot
    /// trip epaint's texture assertion (the same helper the editor typography tests use).
    fn fresh_ctx() -> Context {
        let ctx = Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        output.textures_delta.clear();
        ctx
    }

    fn settle(ctx: &Context) {
        let mut output = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        output.textures_delta.clear();
    }

    #[test]
    fn the_scale_is_explicit_ordered_and_legible() {
        assert_eq!(TITLE_PT, 20.0);
        assert_eq!(PANE_TITLE_PT, 15.0);
        assert_eq!(BODY_PT, 13.0);
        assert_eq!(CODE_PT, 13.0);
        assert_eq!(CAPTION_PT, 11.0);
        assert_eq!(BADGE_PT, 11.0);
        assert_eq!(MIN_RENDERED_PT, CAPTION_PT);
        assert!(TITLE_PT > PANE_TITLE_PT, "title must lead the hierarchy");
        assert!(PANE_TITLE_PT > BODY_PT, "pane titles must lead body text");
        assert!(BODY_PT > CAPTION_PT, "body must lead captions");
        assert_eq!(BODY_PT, CODE_PT, "code and body share the size step");
        assert!(BODY_PT >= 13.0, "VIS-005: body text is never below 13 pt");
        assert!(CODE_PT >= 13.0, "VIS-005: code text is never below 13 pt");
        assert!(
            CAPTION_PT >= 11.0,
            "VIS-005: caption text is never below 11 pt"
        );
        for role in Role::ALL {
            assert!(
                role.line_ratio() + 1e-4 >= MIN_LINE_RATIO,
                "{}: line ratio {:.2} is below {MIN_LINE_RATIO}",
                role.name(),
                role.line_ratio()
            );
            assert!(
                role.pt() >= MIN_RENDERED_PT,
                "{}: below the size floor",
                role.name()
            );
        }
        assert_eq!(
            smallest_role(),
            Role::Caption,
            "the caption is the smallest role"
        );
    }

    #[test]
    fn the_style_text_styles_are_the_scale_not_egui_defaults() {
        let mut style = egui::Style::default();
        // egui's defaults are the values this test exists to replace.
        assert_eq!(style.text_styles[&egui::TextStyle::Small].size, 9.0);
        assert_eq!(style.text_styles[&egui::TextStyle::Heading].size, 18.0);
        install(&mut style);
        for (text_style, role) in [
            (egui::TextStyle::Heading, Role::Title),
            (egui::TextStyle::Body, Role::Body),
            (egui::TextStyle::Button, Role::Body),
            (egui::TextStyle::Monospace, Role::Code),
            (egui::TextStyle::Small, Role::Caption),
        ] {
            let font = &style.text_styles[&text_style];
            assert_eq!(
                font.size,
                role.pt(),
                "{text_style:?} must use {}",
                role.name()
            );
            assert_eq!(
                font.family,
                role.font().family,
                "{text_style:?} must use the {} family",
                role.name()
            );
        }
        assert_eq!(style.text_styles[&egui::TextStyle::Small].size, 11.0);
    }

    #[test]
    fn every_role_renders_at_its_size_in_a_real_galley() {
        let ctx = fresh_ctx();
        ensure_installed(&ctx);
        settle(&ctx);
        for role in Role::ALL {
            let job = egui::text::LayoutJob::simple_singleline(
                "Hamburgefonstiv".to_string(),
                role.font(),
                egui::Color32::WHITE,
            );
            let galley = ctx.fonts_mut(|fonts| fonts.layout_job(job));
            assert!(
                galley.size().x > 0.0,
                "{}: the role must lay out visible text (family {:?})",
                role.name(),
                role.font().family
            );
            assert!(
                galley.size().y >= role.pt() * MIN_LINE_RATIO - 1e-3,
                "{}: measured row {:.2} is below {:.2}x{:.2}",
                role.name(),
                galley.size().y,
                MIN_LINE_RATIO,
                role.pt()
            );
        }
    }

    #[test]
    fn the_strong_weight_resolves_to_a_semibold_face_or_the_regular_family() {
        if !strong_face_available() {
            assert_eq!(strong_family(), FontFamily::Proportional);
            for role in [Role::Title, Role::PaneTitle, Role::Badge] {
                assert_eq!(role.font().family, FontFamily::Proportional);
            }
            eprintln!("SKIP no semibold face on this machine");
            return;
        }
        let ctx = fresh_ctx();
        ensure_installed(&ctx);
        settle(&ctx);
        assert_eq!(
            strong_family(),
            FontFamily::Name(STRONG_FAMILY.into()),
            "a semibold face must be the strong family"
        );
        let owners = ctx.fonts_mut(|fonts| fonts.characters(&strong_family()).get(&'E').cloned());
        assert_eq!(
            owners,
            Some(vec![STRONG_FAMILY.to_string()]),
            "the registered semibold face must own the glyphs"
        );
        for role in [Role::Title, Role::PaneTitle, Role::Badge] {
            assert_eq!(
                role.font().family,
                strong_family(),
                "{}: strong family",
                role.name()
            );
            assert_eq!(role.weight(), Weight::Strong);
        }
        for role in [Role::Body, Role::Caption] {
            assert_eq!(role.font().family, FontFamily::Proportional);
            assert_eq!(role.weight(), Weight::Regular);
        }
        assert_eq!(Role::Code.font().family, FontFamily::Monospace);
    }

    #[test]
    fn the_badge_fit_never_exceeds_the_scale() {
        for height in [6.0, 8.0, 10.0, 16.0, 40.0] {
            let font = fitted_badge(height);
            assert!(
                font.size <= BADGE_PT,
                "{height}: {font:?} exceeds the badge step"
            );
            assert!(
                font.size >= BADGE_PT * 0.8 - 1e-6,
                "{height}: {font:?} is too small to read"
            );
            assert_eq!(font.family, FontFamily::Monospace);
        }
    }

    #[test]
    fn the_report_measures_the_shipped_scale_and_passes() {
        let ctx = fresh_ctx();
        ensure_installed(&ctx);
        settle(&ctx);
        let (report, passed) = legibility_report(&ctx);
        assert!(
            passed,
            "the shipped scale must pass its own legibility check:\n{report}"
        );
        assert!(
            report.contains("smallest rendered text: caption at 11.0 pt"),
            "{report}"
        );
        assert!(report.contains("RESULT PASS"), "{report}");
    }

    /// Collects every numeric font size literal in the production `src/ui` sources.
    ///
    /// The patterns are the four ways the UI can name a size: `FontId::proportional`, the
    /// monospace constructor, `FontId::new` and `RichText::size`. A numeric literal after any of
    /// them is an ad-hoc size. Comment text is removed first so the scale's own documentation
    /// may quote sizes.
    fn font_size_literals(source: &str) -> Vec<(usize, String)> {
        const PATTERNS: [&str; 4] = [
            "FontId::proportional(",
            "FontId::monospace(",
            "FontId::new(",
            ".size(",
        ];
        let mut found = Vec::new();
        for (index, line) in source.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for pattern in PATTERNS {
                let mut rest = code;
                while let Some(at) = rest.find(pattern) {
                    let after = &rest[at + pattern.len()..];
                    let literal: String = after
                        .chars()
                        .take_while(|c| c.is_ascii_digit() || *c == '.')
                        .collect();
                    if literal.chars().any(|c| c.is_ascii_digit()) {
                        found.push((index + 1, format!("{pattern}{literal}")));
                    }
                    rest = after;
                }
            }
        }
        found
    }

    #[test]
    fn the_ui_source_sets_no_font_size_outside_the_scale() {
        let ui_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("ui");
        let mut offenders = Vec::new();
        let mut files = 0usize;
        for entry in std::fs::read_dir(&ui_dir).expect("src/ui") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            if path.file_name().and_then(|n| n.to_str()) == Some("type_scale.rs") {
                continue;
            }
            files += 1;
            let source = std::fs::read_to_string(&path).expect("read");
            for (line, literal) in font_size_literals(&source) {
                offenders.push(format!("{}:{line}: {literal}", path.display()));
            }
        }
        assert!(
            files >= 8,
            "the audit must read the whole ui module, saw {files} files"
        );
        assert!(
            offenders.is_empty(),
            "every UI font size must come from the type scale; found ad-hoc sizes:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn the_audit_rejects_a_literal_size() {
        let source = "let f = FontId::monospace(12.0);\nlet t = RichText::new(\"x\").size(14.0);\n// FontId::new(9.0, Proportional) in a comment is allowed";
        let found = font_size_literals(source);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].1.contains("12.0"));
        assert!(found[1].1.contains("14.0"));
    }
}
