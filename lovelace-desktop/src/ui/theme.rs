//! VIS-002 / VIS-009: the workbench's two complete visual themes and the contrast checker.
//!
//! # Why one module owns every colour
//!
//! Before this module the panes painted `const` colours declared beside each paint routine. A
//! constant cannot change at runtime, so a light theme was impossible and dark was the only
//! rendering. Every colour the workbench paints is now a [`Tokens`] field in [`DARK`] or
//! [`LIGHT`], the active set is selected by [`ThemeId`] (persisted in the settings document),
//! and the accessors below are what the paint routines call. `theme.rs` is the only module in
//! `ui/` allowed to construct a [`Color32`]; a test in this file enforces that, so a new literal
//! colour in a paint routine fails the suite instead of silently escaping the theme.
//!
//! # The two mechanisms
//!
//! - **Explicit paint.** Every surface, text run, border, selection, syntax token, plot chrome
//!   colour and semantic status colour the panes paint comes from the accessors. A theme switch
//!   changes the values those accessors return; the document, the editors, the caret and the
//!   focus are untouched because the token set is not part of the document model.
//! - **egui widgets.** Buttons, text fields, tabs, menus, windows, tooltips, scroll-bars and
//!   selection inside text are painted by egui, not by the panes. [`visuals`] derives the
//!   complete [`egui::Visuals`] from the same tokens (every widget state, window and panel fill,
//!   extreme/code background, hyperlink, warning and error foreground, text cursor, IME
//!   underlines and both shadows), so no egui control is left on egui's default palette.
//!
//! # Contrast checker
//!
//! [`checks`] computes WCAG 2.1 contrast ratios from the *rendered* token values: the same
//! [`Color32`]s the paint routines use. Translucent states (the disabled-control text and the
//! window shadows) are composited over their backdrop exactly as the renderer blends them, and
//! the ratio is computed from the resulting opaque pixel. Normal text must reach 4.5:1, large
//! text and non-text UI indicators 3:1. Exempt pairs are still measured and printed with their
//! ratio and the reason they are exempt; only non-exempt failures make `--contrast-check` exit
//! non-zero.
//!
//! The pair list is deliberately broader than the current paint sites (a text role is measured
//! against every surface it may legitimately appear on), so a future paint change that reuses a
//! role on a different surface usually fails the checker rather than shipping unreadable text.

use egui::{
    style::{ImeComposition, Selection, TextCursorStyle, WidgetVisuals, Widgets},
    Color32, CornerRadius, Shadow, Stroke, Visuals,
};

use crate::settings::ThemePreference;

use super::syntax::Palette;

/// The active theme selector. Alias of the settings type so the persisted value and the runtime
/// selector cannot drift into two enums.
pub type ThemeId = ThemePreference;

/// The opacity egui applies to a disabled control. Kept here so the checker composites the same
/// value the renderer uses.
pub const DISABLED_ALPHA: f32 = 0.5;

/// Every colour one theme paints, grouped by the surface class it belongs to.
///
/// The fields are public because the paint routines live in sibling modules; nothing outside
/// this module constructs a token value.
#[derive(Clone, Copy, Debug)]
pub struct Tokens {
    /// Which theme this token set is.
    #[cfg_attr(not(test), allow(dead_code))]
    pub id: ThemeId,
    /// True for the dark theme; drives egui's text transfer function.
    pub dark: bool,

    // ---- surfaces -------------------------------------------------------------------------
    /// The window's deepest background, visible between panes.
    pub app_bg: Color32,
    /// A pane body background.
    pub panel_bg: Color32,
    /// The editor's text surface.
    pub editor_bg: Color32,
    /// The editor's line-number gutter.
    pub gutter_bg: Color32,
    /// Pane headers and the raised top/bottom bars.
    pub header_bg: Color32,
    /// Window, menu, popup, tooltip and modal backgrounds.
    pub window_bg: Color32,
    /// Text-field background.
    pub field_bg: Color32,
    /// Button background at rest.
    pub button_bg: Color32,
    /// Button background under the pointer.
    pub button_hover_bg: Color32,
    /// Button background while pressed or focused.
    pub button_active_bg: Color32,
    /// The caret row's highlight.
    pub current_line: Color32,
    /// Splitter trough.
    pub splitter_bg: Color32,
    /// Splitter trough under the pointer or keyboard focus.
    pub splitter_hot: Color32,
    /// The plot canvas background behind the engine SVG.
    pub plot_bg: Color32,
    /// Text selection highlight.
    pub selection_bg: Color32,
    /// Every non-current find match.
    pub find_match_bg: Color32,
    /// The find match the bar points at.
    pub find_current_bg: Color32,
    /// A matched bracket's cell fill.
    pub bracket_match_bg: Color32,
    /// An unmatched bracket's cell fill.
    pub bracket_unmatched_bg: Color32,
    /// Completion popup background.
    pub completion_bg: Color32,
    /// The completion row the keyboard is on.
    pub completion_selected_bg: Color32,
    /// Signature-help popup background.
    pub signature_bg: Color32,
    /// Decorative pane/window separation lines and splitter grip at rest.
    pub border: Color32,
    /// Boundaries of interactive controls (fields, buttons, plot canvas, windows).
    pub control_border: Color32,
    /// The plot canvas outline.
    pub plot_border: Color32,
    /// Window and popup shadow; deliberately translucent (see [`roles`]).
    pub shadow: Color32,

    // ---- text -----------------------------------------------------------------------------
    /// Primary text.
    pub text_primary: Color32,
    /// Secondary/dim text.
    pub text_secondary: Color32,
    /// Line numbers.
    pub gutter_text: Color32,
    /// The text caret.
    pub caret: Color32,
    /// Text drawn on a selection highlight.
    pub selection_text: Color32,
    /// The keyboard-focus ring.
    pub focus_ring: Color32,

    // ---- semantic status -------------------------------------------------------------------
    /// The product accent: names, result values, running status.
    pub accent: Color32,
    /// Success / finished.
    pub positive: Color32,
    /// Warning / timing / stale.
    pub warning: Color32,
    /// Failure / diagnostic / cancel.
    pub error: Color32,
    /// In-flight progress.
    pub progress: Color32,
    /// Prompt echoes in the log.
    pub log_echo: Color32,

    // ---- editor chrome ---------------------------------------------------------------------
    /// Matched-bracket underline and box.
    pub bracket_match: Color32,
    /// Unmatched-bracket underline and box.
    pub bracket_unmatched: Color32,
    /// Diagnostic underline in the text.
    pub diagnostic_underline: Color32,
    /// Diagnostic bar in the gutter.
    pub diagnostic_marker: Color32,
    /// Completion popup border.
    pub completion_border: Color32,
    /// Completion signature/detail text.
    pub completion_detail: Color32,
    /// Completion kind: function.
    pub completion_function: Color32,
    /// Completion kind: variable.
    pub completion_variable: Color32,
    /// The underline under the current find match.
    pub find_current_mark: Color32,
    /// Signature-popup border.
    pub signature_border: Color32,
    /// The active parameter in the signature popup.
    pub signature_active: Color32,
    /// Signature-popup body text.
    pub signature_text: Color32,
    /// Plot empty-state text.
    pub plot_empty_text: Color32,

    // ---- syntax ----------------------------------------------------------------------------
    /// The eleven syntax token colours.
    pub syntax: Palette,
}

/// The dark token set (the default).
pub const DARK: Tokens = Tokens {
    id: ThemeId::Dark,
    dark: true,
    app_bg: Color32::from_rgb(0x0b, 0x0e, 0x12),
    panel_bg: Color32::from_rgb(0x17, 0x1b, 0x21),
    editor_bg: Color32::from_rgb(0x14, 0x18, 0x1d),
    gutter_bg: Color32::from_rgb(0x10, 0x13, 0x18),
    header_bg: Color32::from_rgb(0x22, 0x28, 0x30),
    window_bg: Color32::from_rgb(0x1d, 0x23, 0x2b),
    field_bg: Color32::from_rgb(0x10, 0x14, 0x1a),
    button_bg: Color32::from_rgb(0x2a, 0x32, 0x3c),
    button_hover_bg: Color32::from_rgb(0x34, 0x40, 0x4c),
    button_active_bg: Color32::from_rgb(0x3c, 0x4a, 0x58),
    current_line: Color32::from_rgb(0x1f, 0x2a, 0x33),
    splitter_bg: Color32::from_rgb(0x0d, 0x10, 0x14),
    splitter_hot: Color32::from_rgb(0x2b, 0x4a, 0x47),
    plot_bg: Color32::from_rgb(0x0c, 0x10, 0x14),
    selection_bg: Color32::from_rgb(0x20, 0x3a, 0x4a),
    find_match_bg: Color32::from_rgb(0x3a, 0x2f, 0x0f),
    find_current_bg: Color32::from_rgb(0x4a, 0x3a, 0x0c),
    bracket_match_bg: Color32::from_rgb(0x12, 0x3a, 0x34),
    bracket_unmatched_bg: Color32::from_rgb(0x46, 0x20, 0x1f),
    completion_bg: Color32::from_rgb(0x1b, 0x22, 0x2a),
    completion_selected_bg: Color32::from_rgb(0x23, 0x40, 0x3d),
    signature_bg: Color32::from_rgb(0x1b, 0x22, 0x2a),
    border: Color32::from_rgb(0x33, 0x3a, 0x44),
    control_border: Color32::from_rgb(0x8b, 0x98, 0xa9),
    plot_border: Color32::from_rgb(0x8b, 0x98, 0xa9),
    shadow: Color32::from_black_alpha(96),
    text_primary: Color32::from_rgb(0xd8, 0xde, 0xe9),
    text_secondary: Color32::from_rgb(0x9f, 0xab, 0xbd),
    gutter_text: Color32::from_rgb(0x8b, 0x98, 0xa9),
    caret: Color32::from_rgb(0xf0, 0xf4, 0xf8),
    selection_text: Color32::from_rgb(0xf0, 0xf4, 0xf8),
    focus_ring: Color32::from_rgb(0x6f, 0xd3, 0xc8),
    accent: Color32::from_rgb(0x4e, 0xc9, 0xb0),
    positive: Color32::from_rgb(0x7e, 0xc6, 0x99),
    warning: Color32::from_rgb(0xd7, 0xba, 0x7d),
    error: Color32::from_rgb(0xff, 0x7b, 0x72),
    progress: Color32::from_rgb(0x7f, 0xb0, 0xd8),
    log_echo: Color32::from_rgb(0xc8, 0xb6, 0xff),
    bracket_match: Color32::from_rgb(0x4e, 0xc9, 0xb0),
    bracket_unmatched: Color32::from_rgb(0xff, 0x7b, 0x72),
    diagnostic_underline: Color32::from_rgb(0xff, 0x7b, 0x72),
    diagnostic_marker: Color32::from_rgb(0xff, 0x7b, 0x72),
    completion_border: Color32::from_rgb(0x4e, 0xc9, 0xb0),
    completion_detail: Color32::from_rgb(0x9f, 0xab, 0xbd),
    completion_function: Color32::from_rgb(0x4e, 0xc9, 0xb0),
    completion_variable: Color32::from_rgb(0xd8, 0xde, 0xe9),
    find_current_mark: Color32::from_rgb(0xf0, 0xc8, 0x60),
    signature_border: Color32::from_rgb(0x4e, 0xc9, 0xb0),
    signature_active: Color32::from_rgb(0xf0, 0xc8, 0x60),
    signature_text: Color32::from_rgb(0xd8, 0xde, 0xe9),
    plot_empty_text: Color32::from_rgb(0x9f, 0xab, 0xbd),
    syntax: Palette {
        plain: Color32::from_rgb(0xd8, 0xde, 0xe9),
        number: Color32::from_rgb(0xb8, 0xd4, 0x8e),
        rational: Color32::from_rgb(0xe8, 0xc4, 0x7f),
        string: Color32::from_rgb(0xe8, 0xad, 0x92),
        interpolation: Color32::from_rgb(0xf2, 0xae, 0xd4),
        keyword: Color32::from_rgb(0xd7, 0x9a, 0xde),
        operator: Color32::from_rgb(0x63, 0xc7, 0xd4),
        function: Color32::from_rgb(0x4e, 0xc9, 0xb0),
        builtin: Color32::from_rgb(0x5f, 0xc8, 0xff),
        delimiter: Color32::from_rgb(0xa8, 0xb5, 0xc6),
        symbol: Color32::from_rgb(0xcf, 0xbc, 0xff),
    },
};

/// The light token set.
pub const LIGHT: Tokens = Tokens {
    id: ThemeId::Light,
    dark: false,
    app_bg: Color32::from_rgb(0xf2, 0xf4, 0xf7),
    panel_bg: Color32::from_rgb(0xff, 0xff, 0xff),
    editor_bg: Color32::from_rgb(0xff, 0xff, 0xff),
    gutter_bg: Color32::from_rgb(0xf0, 0xf3, 0xf7),
    header_bg: Color32::from_rgb(0xe6, 0xeb, 0xf2),
    window_bg: Color32::from_rgb(0xff, 0xff, 0xff),
    field_bg: Color32::from_rgb(0xff, 0xff, 0xff),
    button_bg: Color32::from_rgb(0xe8, 0xed, 0xf4),
    button_hover_bg: Color32::from_rgb(0xdb, 0xe3, 0xee),
    button_active_bg: Color32::from_rgb(0xcf, 0xd9, 0xe6),
    current_line: Color32::from_rgb(0xf1, 0xf5, 0xfb),
    splitter_bg: Color32::from_rgb(0xe2, 0xe7, 0xee),
    splitter_hot: Color32::from_rgb(0xc3, 0xd9, 0xd4),
    plot_bg: Color32::from_rgb(0xfb, 0xfc, 0xfd),
    selection_bg: Color32::from_rgb(0xcf, 0xe3, 0xff),
    find_match_bg: Color32::from_rgb(0xff, 0xf3, 0xc4),
    find_current_bg: Color32::from_rgb(0xff, 0xe0, 0x8a),
    bracket_match_bg: Color32::from_rgb(0xd3, 0xf2, 0xe3),
    bracket_unmatched_bg: Color32::from_rgb(0xff, 0xe0, 0xde),
    completion_bg: Color32::from_rgb(0xff, 0xff, 0xff),
    completion_selected_bg: Color32::from_rgb(0xdb, 0xe7, 0xf8),
    signature_bg: Color32::from_rgb(0xff, 0xff, 0xff),
    border: Color32::from_rgb(0xc3, 0xcc, 0xd8),
    control_border: Color32::from_rgb(0x64, 0x70, 0x7f),
    plot_border: Color32::from_rgb(0x64, 0x70, 0x7f),
    shadow: Color32::from_black_alpha(40),
    text_primary: Color32::from_rgb(0x1b, 0x24, 0x30),
    text_secondary: Color32::from_rgb(0x4a, 0x56, 0x66),
    gutter_text: Color32::from_rgb(0x5a, 0x66, 0x75),
    caret: Color32::from_rgb(0x12, 0x3a, 0x5c),
    selection_text: Color32::from_rgb(0x10, 0x23, 0x3a),
    focus_ring: Color32::from_rgb(0x0a, 0x5c, 0x52),
    accent: Color32::from_rgb(0x0b, 0x6b, 0x60),
    positive: Color32::from_rgb(0x17, 0x69, 0x3a),
    warning: Color32::from_rgb(0x7a, 0x53, 0x00),
    error: Color32::from_rgb(0xb3, 0x26, 0x1e),
    progress: Color32::from_rgb(0x14, 0x53, 0x8c),
    log_echo: Color32::from_rgb(0x5b, 0x3a, 0x8c),
    bracket_match: Color32::from_rgb(0x0b, 0x6b, 0x60),
    bracket_unmatched: Color32::from_rgb(0xb3, 0x26, 0x1e),
    diagnostic_underline: Color32::from_rgb(0xb3, 0x26, 0x1e),
    diagnostic_marker: Color32::from_rgb(0xb3, 0x26, 0x1e),
    completion_border: Color32::from_rgb(0x0b, 0x6b, 0x60),
    completion_detail: Color32::from_rgb(0x4a, 0x56, 0x66),
    completion_function: Color32::from_rgb(0x0b, 0x6b, 0x60),
    completion_variable: Color32::from_rgb(0x1b, 0x24, 0x30),
    find_current_mark: Color32::from_rgb(0x7a, 0x53, 0x00),
    signature_border: Color32::from_rgb(0x0b, 0x6b, 0x60),
    signature_active: Color32::from_rgb(0x7a, 0x53, 0x00),
    signature_text: Color32::from_rgb(0x1b, 0x24, 0x30),
    plot_empty_text: Color32::from_rgb(0x4a, 0x56, 0x66),
    syntax: Palette {
        plain: Color32::from_rgb(0x1f, 0x23, 0x28),
        number: Color32::from_rgb(0x0a, 0x6b, 0x47),
        rational: Color32::from_rgb(0x7a, 0x4f, 0x00),
        string: Color32::from_rgb(0xa3, 0x15, 0x15),
        interpolation: Color32::from_rgb(0x8e, 0x1f, 0xa0),
        keyword: Color32::from_rgb(0x7a, 0x1f, 0xa2),
        operator: Color32::from_rgb(0x0b, 0x6b, 0x8a),
        function: Color32::from_rgb(0x0b, 0x6b, 0x60),
        builtin: Color32::from_rgb(0x0b, 0x57, 0xd0),
        delimiter: Color32::from_rgb(0x4a, 0x55, 0x64),
        symbol: Color32::from_rgb(0x5b, 0x3a, 0x8c),
    },
};

/// The token set for an explicit theme.
pub fn tokens_for(theme: ThemeId) -> &'static Tokens {
    match theme {
        ThemeId::Dark => &DARK,
        ThemeId::Light => &LIGHT,
    }
}

use std::cell::Cell;

thread_local! {
    /// The theme the current UI thread paints with. A thread-local, not a process global, so
    /// two windows on two threads (and two tests in one binary) cannot change each other's
    /// colours.
    static ACTIVE: Cell<ThemeId> = const { Cell::new(ThemeId::Dark) };
}

/// Selects the theme the accessors below return on this UI thread.
pub fn set_active(theme: ThemeId) {
    ACTIVE.with(|active| active.set(theme));
}

/// The theme the accessors below currently return.
pub fn active() -> ThemeId {
    ACTIVE.with(|active| active.get())
}

/// The active theme's token set. Every paint routine calls this through the accessors.
pub fn tokens() -> &'static Tokens {
    tokens_for(active())
}

/// Writes one theme's [`Visuals`] and scroll style into both of egui's theme slots and selects
/// it.
///
/// Writing both slots (`Dark` and `Light`) is deliberate: if the OS reports a theme change,
/// egui re-selects a style slot, and both slots hold the same token-derived style. A theme
/// switch therefore cannot fall back to egui's default dark or light palette, and neither can a
/// system theme change.
pub fn apply(ctx: &egui::Context, theme: ThemeId) {
    for slot in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(slot, |style| apply_style(style, theme));
    }
    ctx.set_theme(match theme {
        ThemeId::Dark => egui::Theme::Dark,
        ThemeId::Light => egui::Theme::Light,
    });
    set_active(theme);
}

/// Applies one theme's tokens to a whole egui [`Style`].
///
/// `style.visuals` carries every colour; `style.spacing.scroll` is not part of [`Visuals`] but
/// is a painted UI component too, so it is set here alongside them.
pub fn apply_style(style: &mut egui::Style, theme: ThemeId) {
    style.visuals = visuals_for(theme);
    style.spacing.scroll = scroll_style();
}

/// The themed scroll-bar style.
///
/// egui's floating default fades the handle to zero opacity and paints a translucent fill over
/// the content, which is an unthemed state that cannot meet the 3:1 non-text threshold. The
/// workbench uses solid, always-visible bars: the handle is the widget foreground (a text token)
/// and the trough is the field background, both measured by the contrast checker.
pub fn scroll_style() -> egui::style::ScrollStyle {
    let mut scroll = egui::style::ScrollStyle::solid();
    scroll.foreground_color = true;
    scroll.bar_width = 8.0;
    scroll
}

/// The complete egui style derived from one theme's tokens.
pub fn visuals_for(theme: ThemeId) -> Visuals {
    visuals(tokens_for(theme))
}

/// Derives egui's complete widget style from `t`.
///
/// Every colour-bearing field the workbench can reach is set here. Fields intentionally left at
/// egui's value are non-colour behaviour (`corner_radius` sizes, `resize_corner_size`,
/// `interact_cursor`, animation timings and similar); the test in this module asserts that every
/// painted colour is token-derived.
pub fn visuals(t: &Tokens) -> Visuals {
    let mut visuals = if t.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let corner = CornerRadius::same(2);
    visuals.dark_mode = t.dark;
    visuals.override_text_color = Some(t.text_primary);
    visuals.weak_text_color = Some(t.text_secondary);
    visuals.weak_text_alpha = 1.0;
    visuals.selection = Selection {
        bg_fill: t.selection_bg,
        stroke: Stroke::new(1.0, t.selection_text),
    };
    visuals.hyperlink_color = t.accent;
    visuals.faint_bg_color = t.current_line;
    visuals.extreme_bg_color = t.field_bg;
    visuals.text_edit_bg_color = Some(t.field_bg);
    visuals.code_bg_color = t.completion_bg;
    visuals.warn_fg_color = t.warning;
    visuals.error_fg_color = t.error;
    visuals.window_fill = t.window_bg;
    visuals.window_stroke = Stroke::new(1.0, t.control_border);
    // The window chrome (file bar, toolbar, tab strip, status strip and the gaps between panes)
    // is the deepest surface; pane headers are the raised step above it.
    visuals.panel_fill = t.app_bg;
    visuals.text_cursor = TextCursorStyle {
        stroke: Stroke::new(1.5, t.caret),
        ..visuals.text_cursor
    };
    visuals.ime_composition = ImeComposition {
        active_underline_stroke: Stroke::new(2.0, t.focus_ring),
        inactive_underline_stroke: Stroke::new(2.0, t.text_secondary),
        legacy_visuals: true,
    };
    visuals.window_shadow = Shadow {
        offset: [6, 10],
        blur: 12,
        spread: 0,
        color: t.shadow,
    };
    visuals.popup_shadow = Shadow {
        offset: [4, 8],
        blur: 10,
        spread: 0,
        color: t.shadow,
    };
    visuals.disabled_alpha = DISABLED_ALPHA;

    let widget = |bg: Color32, border: Color32, fg: Color32| WidgetVisuals {
        bg_fill: bg,
        weak_bg_fill: bg,
        bg_stroke: Stroke::new(1.0, border),
        corner_radius: corner,
        fg_stroke: Stroke::new(1.0, fg),
        expansion: 0.0,
    };
    visuals.widgets = Widgets {
        noninteractive: widget(t.panel_bg, t.border, t.text_secondary),
        inactive: widget(t.button_bg, t.control_border, t.text_primary),
        hovered: widget(t.button_hover_bg, t.control_border, t.text_primary),
        // egui paints a focused button with the active style; giving it the focus ring makes
        // keyboard focus visible on every control, not only inside the editor.
        active: widget(t.button_active_bg, t.focus_ring, t.text_primary),
        open: widget(t.button_hover_bg, t.focus_ring, t.text_primary),
    };
    visuals
}

// -------------------------------------------------------------------------------------------
// Token accessors. Paint routines call these, never a `const` or a literal.
// -------------------------------------------------------------------------------------------

macro_rules! accessor {
    ($name:ident, $field:ident) => {
        /// The active theme's token.
        #[allow(dead_code)] // not every role is painted by a direct accessor (some only via visuals)
        pub fn $name() -> Color32 {
            tokens().$field
        }
    };
}

accessor!(app_bg, app_bg);
accessor!(panel_bg, panel_bg);
accessor!(editor_bg, editor_bg);
accessor!(gutter_bg, gutter_bg);
accessor!(header_bg, header_bg);
accessor!(window_bg, window_bg);
accessor!(field_bg, field_bg);
accessor!(button_bg, button_bg);
accessor!(button_hover_bg, button_hover_bg);
accessor!(button_active_bg, button_active_bg);
accessor!(current_line, current_line);
accessor!(splitter_bg, splitter_bg);
accessor!(splitter_hot, splitter_hot);
accessor!(plot_bg, plot_bg);
accessor!(selection_bg, selection_bg);
accessor!(find_match_bg, find_match_bg);
accessor!(find_current_bg, find_current_bg);
accessor!(bracket_match_bg, bracket_match_bg);
accessor!(bracket_unmatched_bg, bracket_unmatched_bg);
accessor!(completion_bg, completion_bg);
accessor!(completion_selected_bg, completion_selected_bg);
accessor!(signature_bg, signature_bg);
accessor!(border, border);
accessor!(control_border, control_border);
accessor!(plot_border, plot_border);
accessor!(text, text_primary);
accessor!(dim, text_secondary);
accessor!(gutter_text, gutter_text);
accessor!(caret, caret);
accessor!(selection_text, selection_text);
accessor!(focus_ring, focus_ring);
accessor!(accent, accent);
accessor!(positive, positive);
accessor!(warning, warning);
accessor!(error, error);
accessor!(progress, progress);
accessor!(log_echo, log_echo);
accessor!(bracket_match, bracket_match);
accessor!(bracket_unmatched, bracket_unmatched);
accessor!(diagnostic_underline, diagnostic_underline);
accessor!(diagnostic_marker, diagnostic_marker);
accessor!(completion_border, completion_border);
accessor!(completion_detail, completion_detail);
accessor!(completion_function, completion_function);
accessor!(completion_variable, completion_variable);
accessor!(find_current_mark, find_current_mark);
accessor!(signature_border, signature_border);
accessor!(signature_active, signature_active);
accessor!(signature_text, signature_text);
accessor!(plot_empty_text, plot_empty_text);

/// The active syntax palette.
pub fn syntax() -> &'static Palette {
    &tokens().syntax
}

/// The identity colour for the plot texture tint. A texture tint multiplies the image, so this
/// must be fully opaque white; it is not a surface or text role.
pub fn texture_tint() -> Color32 {
    Color32::WHITE
}

/// The compatibility name for the dark syntax palette (used by the highlighter's tests).
#[cfg_attr(not(test), allow(dead_code))]
pub fn dark_syntax() -> &'static Palette {
    &DARK.syntax
}

// -------------------------------------------------------------------------------------------
// VIS-009: the contrast checker.
// -------------------------------------------------------------------------------------------

/// How a foreground/background pair is classified by the WCAG rule it must satisfy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairClass {
    /// Normal text: WCAG AA 4.5:1.
    NormalText,
    /// Large text (>= 18.66 px bold or 24 px regular): WCAG AA 3:1.
    #[cfg_attr(not(test), allow(dead_code))]
    LargeText,
    /// Non-text UI indicator (focus ring, caret, border, underline): 3:1.
    NonText,
    /// Deliberately not required to meet a ratio; the reason is recorded and printed.
    Exempt(&'static str),
}

impl PairClass {
    /// The minimum ratio this class must reach, or 0 for an exempt pair.
    pub fn required(self) -> f64 {
        match self {
            PairClass::NormalText => 4.5,
            PairClass::LargeText | PairClass::NonText => 3.0,
            PairClass::Exempt(_) => 0.0,
        }
    }

    /// The lowercase class name used by `--contrast-check`.
    pub fn name(self) -> &'static str {
        match self {
            PairClass::NormalText => "normal-text",
            PairClass::LargeText => "large-text",
            PairClass::NonText => "non-text",
            PairClass::Exempt(_) => "exempt",
        }
    }
}

/// One measured pair, as printed by `--contrast-check` and asserted by the tests.
#[derive(Clone, Debug)]
pub struct PairResult {
    /// The theme the pair belongs to.
    pub theme: ThemeId,
    /// Human-readable role pair, e.g. `"text on panel"`.
    pub role: String,
    /// The rendered foreground colour (composited if translucent).
    pub fg: Color32,
    /// The rendered background colour.
    pub bg: Color32,
    /// The rule the pair must satisfy.
    pub class: PairClass,
    /// The computed WCAG contrast ratio.
    pub ratio: f64,
    /// True when the ratio meets the class threshold (exempt pairs always pass).
    pub pass: bool,
}

/// WCAG 2.1 relative luminance of an opaque sRGB colour.
pub fn relative_luminance(color: Color32) -> f64 {
    let [r, g, b, _] = color.to_srgba_unmultiplied();
    let channel = |value: u8| {
        let s = value as f64 / 255.0;
        if s <= 0.040_45 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

/// WCAG 2.1 contrast ratio between two opaque rendered colours.
pub fn contrast_ratio(fg: Color32, bg: Color32) -> f64 {
    let a = relative_luminance(fg);
    let b = relative_luminance(bg);
    let (light, dark) = if a >= b { (a, b) } else { (b, a) };
    (light + 0.05) / (dark + 0.05)
}

/// Composites `fg` over `bg` exactly as the renderer does (premultiplied alpha), returning the
/// opaque pixel the eye sees.
pub fn composite(fg: Color32, bg: Color32) -> Color32 {
    bg.blend(fg)
}

/// Measures one pair whose colours are already the rendered, composited pixels.
pub fn check(theme: ThemeId, role: &str, fg: Color32, bg: Color32, class: PairClass) -> PairResult {
    let ratio = contrast_ratio(fg, bg);
    let required = class.required();
    PairResult {
        theme,
        role: role.to_string(),
        fg,
        bg,
        class,
        ratio,
        pass: ratio >= required,
    }
}

fn text_pairs(
    out: &mut Vec<PairResult>,
    theme: ThemeId,
    fg_role: &str,
    fg: Color32,
    bgs: &[(&str, Color32)],
) {
    for (bg_role, bg) in bgs {
        out.push(check(
            theme,
            &format!("{fg_role} on {bg_role}"),
            fg,
            *bg,
            PairClass::NormalText,
        ));
    }
}

fn non_text(out: &mut Vec<PairResult>, theme: ThemeId, role: &str, fg: Color32, bg: Color32) {
    out.push(check(theme, role, fg, bg, PairClass::NonText));
}

fn exempt(
    out: &mut Vec<PairResult>,
    theme: ThemeId,
    role: &str,
    fg: Color32,
    bg: Color32,
    reason: &'static str,
) {
    out.push(check(theme, role, fg, bg, PairClass::Exempt(reason)));
}

/// Every contrast pair the workbench can paint in `theme`, measured from the rendered tokens.
///
/// The list is intentionally broad: each text role is checked against every surface it can
/// appear on, and every syntax class is checked against every editor highlight it can be painted
/// over. Pairs that WCAG does not require (decorative separators, highlight fills, inactive
/// controls) are `Exempt` but are still measured and printed with a reason.
pub fn checks(theme: ThemeId) -> Vec<PairResult> {
    let t = tokens_for(theme);
    let mut out = Vec::new();

    // Surfaces a text role can legitimately be painted on.
    let all_surfaces: [(&str, Color32); 19] = [
        ("panel", t.panel_bg),
        ("editor", t.editor_bg),
        ("gutter", t.gutter_bg),
        ("header", t.header_bg),
        ("window", t.window_bg),
        ("field", t.field_bg),
        ("button", t.button_bg),
        ("button-hover", t.button_hover_bg),
        ("button-active", t.button_active_bg),
        ("plot", t.plot_bg),
        ("current-line", t.current_line),
        ("completion", t.completion_bg),
        ("completion-selected", t.completion_selected_bg),
        ("signature", t.signature_bg),
        ("find-match", t.find_match_bg),
        ("find-current", t.find_current_bg),
        ("bracket-match", t.bracket_match_bg),
        ("bracket-unmatched", t.bracket_unmatched_bg),
        ("selection", t.selection_bg),
    ];
    let text_surfaces: [(&str, Color32); 11] = [
        ("app", t.app_bg),
        ("panel", t.panel_bg),
        ("editor", t.editor_bg),
        ("header", t.header_bg),
        ("window", t.window_bg),
        ("field", t.field_bg),
        ("button", t.button_bg),
        ("plot", t.plot_bg),
        ("completion", t.completion_bg),
        ("completion-selected", t.completion_selected_bg),
        ("signature", t.signature_bg),
    ];

    // ---- primary and secondary text -------------------------------------------------------
    text_pairs(&mut out, theme, "text", t.text_primary, &all_surfaces);
    text_pairs(&mut out, theme, "dim", t.text_secondary, &text_surfaces);

    text_pairs(
        &mut out,
        theme,
        "gutter-text",
        t.gutter_text,
        &[("gutter", t.gutter_bg), ("current-line", t.current_line)],
    );
    text_pairs(
        &mut out,
        theme,
        "selection-text",
        t.selection_text,
        &[("selection", t.selection_bg)],
    );
    text_pairs(
        &mut out,
        theme,
        "plot-empty-text",
        t.plot_empty_text,
        &[("plot", t.plot_bg), ("panel", t.panel_bg)],
    );

    // ---- semantic status text --------------------------------------------------------------
    let status_surfaces: [(&str, Color32); 6] = [
        ("app", t.app_bg),
        ("panel", t.panel_bg),
        ("header", t.header_bg),
        ("window", t.window_bg),
        ("editor", t.editor_bg),
        ("plot", t.plot_bg),
    ];
    text_pairs(&mut out, theme, "accent", t.accent, &status_surfaces);
    text_pairs(&mut out, theme, "positive", t.positive, &status_surfaces);
    text_pairs(&mut out, theme, "warning", t.warning, &status_surfaces);
    text_pairs(&mut out, theme, "error", t.error, &status_surfaces);
    text_pairs(&mut out, theme, "progress", t.progress, &status_surfaces);
    text_pairs(
        &mut out,
        theme,
        "log-echo",
        t.log_echo,
        &[("panel", t.panel_bg), ("header", t.header_bg)],
    );

    // ---- editor chrome text ----------------------------------------------------------------
    text_pairs(
        &mut out,
        theme,
        "completion-detail",
        t.completion_detail,
        &[
            ("completion", t.completion_bg),
            ("completion-selected", t.completion_selected_bg),
        ],
    );
    text_pairs(
        &mut out,
        theme,
        "signature-text",
        t.signature_text,
        &[("signature", t.signature_bg)],
    );
    text_pairs(
        &mut out,
        theme,
        "signature-active",
        t.signature_active,
        &[("signature", t.signature_bg)],
    );
    text_pairs(
        &mut out,
        theme,
        "completion-function",
        t.completion_function,
        &[
            ("completion", t.completion_bg),
            ("completion-selected", t.completion_selected_bg),
        ],
    );
    text_pairs(
        &mut out,
        theme,
        "completion-variable",
        t.completion_variable,
        &[
            ("completion", t.completion_bg),
            ("completion-selected", t.completion_selected_bg),
        ],
    );

    // ---- syntax classes over every editor highlight ----------------------------------------
    let editor_surfaces: [(&str, Color32); 8] = [
        ("editor", t.editor_bg),
        ("current-line", t.current_line),
        ("selection", t.selection_bg),
        ("find-match", t.find_match_bg),
        ("find-current", t.find_current_bg),
        ("bracket-match", t.bracket_match_bg),
        ("bracket-unmatched", t.bracket_unmatched_bg),
        ("completion-selected", t.completion_selected_bg),
    ];
    let syntax = &t.syntax;
    for (name, color) in [
        ("syntax-plain", syntax.plain),
        ("syntax-number", syntax.number),
        ("syntax-rational", syntax.rational),
        ("syntax-string", syntax.string),
        ("syntax-interpolation", syntax.interpolation),
        ("syntax-keyword", syntax.keyword),
        ("syntax-operator", syntax.operator),
        ("syntax-function", syntax.function),
        ("syntax-builtin", syntax.builtin),
        ("syntax-delimiter", syntax.delimiter),
        ("syntax-symbol", syntax.symbol),
    ] {
        text_pairs(&mut out, theme, name, color, &editor_surfaces);
    }

    // ---- non-text UI indicators ------------------------------------------------------------
    non_text(&mut out, theme, "focus-ring on app", t.focus_ring, t.app_bg);
    non_text(
        &mut out,
        theme,
        "focus-ring on panel",
        t.focus_ring,
        t.panel_bg,
    );
    non_text(
        &mut out,
        theme,
        "focus-ring on editor",
        t.focus_ring,
        t.editor_bg,
    );
    non_text(
        &mut out,
        theme,
        "focus-ring on header",
        t.focus_ring,
        t.header_bg,
    );
    non_text(
        &mut out,
        theme,
        "focus-ring on window",
        t.focus_ring,
        t.window_bg,
    );
    non_text(
        &mut out,
        theme,
        "focus-ring on field",
        t.focus_ring,
        t.field_bg,
    );
    non_text(
        &mut out,
        theme,
        "focus-ring on button",
        t.focus_ring,
        t.button_bg,
    );
    non_text(
        &mut out,
        theme,
        "focus-ring on button-hover",
        t.focus_ring,
        t.button_hover_bg,
    );
    non_text(
        &mut out,
        theme,
        "focus-ring on button-active",
        t.focus_ring,
        t.button_active_bg,
    );
    non_text(
        &mut out,
        theme,
        "focus-ring on plot",
        t.focus_ring,
        t.plot_bg,
    );
    non_text(
        &mut out,
        theme,
        "control-border on app",
        t.control_border,
        t.app_bg,
    );
    non_text(
        &mut out,
        theme,
        "control-border on panel",
        t.control_border,
        t.panel_bg,
    );
    non_text(
        &mut out,
        theme,
        "control-border on header",
        t.control_border,
        t.header_bg,
    );
    non_text(
        &mut out,
        theme,
        "control-border on window",
        t.control_border,
        t.window_bg,
    );
    non_text(
        &mut out,
        theme,
        "control-border on field",
        t.control_border,
        t.field_bg,
    );
    non_text(
        &mut out,
        theme,
        "control-border on button",
        t.control_border,
        t.button_bg,
    );
    non_text(
        &mut out,
        theme,
        "control-border on button-hover",
        t.control_border,
        t.button_hover_bg,
    );
    non_text(
        &mut out,
        theme,
        "control-border on button-active",
        t.control_border,
        t.button_active_bg,
    );
    non_text(
        &mut out,
        theme,
        "plot-border on plot",
        t.plot_border,
        t.plot_bg,
    );
    non_text(
        &mut out,
        theme,
        "plot-border on panel",
        t.plot_border,
        t.panel_bg,
    );
    non_text(&mut out, theme, "caret on editor", t.caret, t.editor_bg);
    non_text(
        &mut out,
        theme,
        "caret on current-line",
        t.caret,
        t.current_line,
    );
    // egui paints a solid scroll bar's handle in the widget foreground and its trough in the
    // extreme background; both are tokens, and the handle must reach 3:1 against the trough.
    non_text(
        &mut out,
        theme,
        "scrollbar-handle on field",
        t.text_primary,
        t.field_bg,
    );
    non_text(
        &mut out,
        theme,
        "splitter-grip on splitter",
        t.control_border,
        t.splitter_bg,
    );
    non_text(
        &mut out,
        theme,
        "splitter-hot-grip on splitter-hot",
        t.accent,
        t.splitter_hot,
    );
    non_text(
        &mut out,
        theme,
        "bracket-match-mark on editor",
        t.bracket_match,
        t.editor_bg,
    );
    non_text(
        &mut out,
        theme,
        "bracket-match-mark on bracket-match",
        t.bracket_match,
        t.bracket_match_bg,
    );
    non_text(
        &mut out,
        theme,
        "bracket-unmatched-mark on editor",
        t.bracket_unmatched,
        t.editor_bg,
    );
    non_text(
        &mut out,
        theme,
        "bracket-unmatched-mark on bracket-unmatched",
        t.bracket_unmatched,
        t.bracket_unmatched_bg,
    );
    non_text(
        &mut out,
        theme,
        "diagnostic-underline on editor",
        t.diagnostic_underline,
        t.editor_bg,
    );
    non_text(
        &mut out,
        theme,
        "diagnostic-underline on current-line",
        t.diagnostic_underline,
        t.current_line,
    );
    non_text(
        &mut out,
        theme,
        "diagnostic-underline on selection",
        t.diagnostic_underline,
        t.selection_bg,
    );
    non_text(
        &mut out,
        theme,
        "diagnostic-marker on gutter",
        t.diagnostic_marker,
        t.gutter_bg,
    );
    non_text(
        &mut out,
        theme,
        "find-current-mark on find-current",
        t.find_current_mark,
        t.find_current_bg,
    );
    non_text(
        &mut out,
        theme,
        "completion-border on completion",
        t.completion_border,
        t.completion_bg,
    );
    non_text(
        &mut out,
        theme,
        "completion-border on completion-selected",
        t.completion_border,
        t.completion_selected_bg,
    );
    non_text(
        &mut out,
        theme,
        "signature-border on signature",
        t.signature_border,
        t.signature_bg,
    );

    // ---- documented exemptions -------------------------------------------------------------
    exempt(
        &mut out,
        theme,
        "disabled text on panel",
        composite(t.text_primary.gamma_multiply(DISABLED_ALPHA), t.panel_bg),
        t.panel_bg,
        "inactive control text: WCAG 1.4.3 exempts inactive components; the enabled text passes",
    );
    exempt(
        &mut out,
        theme,
        "disabled text on button",
        composite(t.text_primary.gamma_multiply(DISABLED_ALPHA), t.button_bg),
        t.button_bg,
        "inactive control text: WCAG 1.4.3 exempts inactive components; the enabled text passes",
    );
    exempt(
        &mut out,
        theme,
        "pane-border on panel",
        t.border,
        t.panel_bg,
        "decorative pane separation, not a control boundary; control borders are checked at 3:1",
    );
    exempt(
        &mut out,
        theme,
        "pane-border on header",
        t.border,
        t.header_bg,
        "decorative pane separation, not a control boundary; control borders are checked at 3:1",
    );
    exempt(
        &mut out,
        theme,
        "header on panel",
        t.header_bg,
        t.panel_bg,
        "decorative raised-header separation; header text is checked against the header fill",
    );
    exempt(
        &mut out,
        theme,
        "selection on editor",
        t.selection_bg,
        t.editor_bg,
        "text-selection fill, not a component boundary; the text over it passes at 4.5:1",
    );
    exempt(
        &mut out,
        theme,
        "find-match on editor",
        t.find_match_bg,
        t.editor_bg,
        "find highlight fill, not a component boundary; the text over it passes at 4.5:1",
    );
    exempt(
        &mut out,
        theme,
        "find-current on editor",
        t.find_current_bg,
        t.editor_bg,
        "current find highlight fill, not a component boundary; its text and underline pass",
    );
    exempt(
        &mut out,
        theme,
        "bracket-match on editor",
        t.bracket_match_bg,
        t.editor_bg,
        "bracket cell fill; the bracket underline over it is checked at 3:1",
    );
    exempt(
        &mut out,
        theme,
        "bracket-unmatched on editor",
        t.bracket_unmatched_bg,
        t.editor_bg,
        "unmatched cell fill; the bracket box over it is checked at 3:1",
    );
    exempt(
        &mut out,
        theme,
        "current-line on editor",
        t.current_line,
        t.editor_bg,
        "subtle current-line decoration; the text over it is checked at 4.5:1",
    );
    exempt(
        &mut out,
        theme,
        "splitter on app",
        t.splitter_bg,
        t.app_bg,
        "splitter trough fill; the splitter grip is checked at 3:1",
    );
    exempt(
        &mut out,
        theme,
        "pane-border on splitter",
        t.border,
        t.splitter_bg,
        "decorative pane separation; the splitter grip is checked at 3:1",
    );
    exempt(
        &mut out,
        theme,
        "completion-selected on completion",
        t.completion_selected_bg,
        t.completion_bg,
        "completion selection fill; the entries over it are checked at 4.5:1",
    );
    exempt(
        &mut out,
        theme,
        "shadow on panel",
        composite(t.shadow, t.panel_bg),
        t.panel_bg,
        "window shadow, decorative depth only; its alpha is intentional, not an unset colour",
    );

    out
}

/// The ratio and threshold line for one pair; used by `--contrast-check` and the docs.
pub fn format_pair(result: &PairResult) -> String {
    let theme = match result.theme {
        ThemeId::Dark => "dark",
        ThemeId::Light => "light",
    };
    let required = result.class.required();
    let exempt = match result.class {
        PairClass::Exempt(reason) => format!(" EXEMPT reason=\"{reason}\""),
        _ => String::new(),
    };
    let required_text = if required == 0.0 {
        "-".to_string()
    } else {
        format!("{required:.2}")
    };
    format!(
        "theme={theme} pair=\"{}\" fg=#{} bg=#{} class={} ratio={:.2} required={} {}{}",
        result.role,
        hex(result.fg),
        hex(result.bg),
        result.class.name(),
        result.ratio,
        required_text,
        if result.pass { "PASS" } else { "FAIL" },
        exempt
    )
}

fn hex(color: Color32) -> String {
    let [r, g, b, _] = color.to_srgba_unmultiplied();
    format!("{r:02x}{g:02x}{b:02x}")
}

/// The complete `--contrast-check` report and whether every required pair passed.
pub fn report() -> (String, bool) {
    let mut text = String::new();
    let mut all_ok = true;
    for theme in [ThemeId::Dark, ThemeId::Light] {
        let results = checks(theme);
        let failures: Vec<&PairResult> = results.iter().filter(|result| !result.pass).collect();
        let exempt_count = results
            .iter()
            .filter(|result| matches!(result.class, PairClass::Exempt(_)))
            .count();
        let theme_name = match theme {
            ThemeId::Dark => "dark",
            ThemeId::Light => "light",
        };
        text.push_str(&format!(
            "theme={theme_name} pairs={} exempt={exempt_count} failures={}\n",
            results.len(),
            failures.len()
        ));
        for result in &results {
            text.push_str(&format_pair(result));
            text.push('\n');
        }
        if !failures.is_empty() {
            all_ok = false;
            for failure in failures {
                text.push_str(&format!(
                    "VIOLATION theme={theme_name} pair=\"{}\" ratio={:.2} required={:.2}\n",
                    failure.role,
                    failure.ratio,
                    failure.class.required()
                ));
            }
        }
    }
    (text, all_ok)
}

/// `--contrast-check`: prints every pair and exits non-zero when a required pair fails.
pub fn contrast_cli() -> i32 {
    let (text, ok) = report();
    print!("{text}");
    if ok {
        println!(
            "contrast: every required pair passes (normal text 4.5:1, large text/non-text 3:1)"
        );
        0
    } else {
        eprintln!("contrast: at least one required pair fails");
        1
    }
}

// -------------------------------------------------------------------------------------------
// Audit helpers and tests.
// -------------------------------------------------------------------------------------------

/// One named painted role, for the completeness and no-default-colour audit.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct Role {
    /// The semantic role name.
    pub name: &'static str,
    /// The rendered colour.
    pub color: Color32,
    /// True for deliberately translucent decoration (window/popup shadow).
    pub translucent: bool,
}

/// Every colour role in `t`, enumerated independently of the struct layout so the audit can
/// assert completeness, opacity and non-transparency per role.
#[cfg_attr(not(test), allow(dead_code))]
pub fn roles(t: &Tokens) -> Vec<Role> {
    let opaque = |name, color| Role {
        name,
        color,
        translucent: false,
    };
    let mut roles = vec![
        opaque("app", t.app_bg),
        opaque("panel", t.panel_bg),
        opaque("editor", t.editor_bg),
        opaque("gutter", t.gutter_bg),
        opaque("header", t.header_bg),
        opaque("window", t.window_bg),
        opaque("field", t.field_bg),
        opaque("button", t.button_bg),
        opaque("button-hover", t.button_hover_bg),
        opaque("button-active", t.button_active_bg),
        opaque("current-line", t.current_line),
        opaque("splitter", t.splitter_bg),
        opaque("splitter-hot", t.splitter_hot),
        opaque("plot", t.plot_bg),
        opaque("selection", t.selection_bg),
        opaque("find-match", t.find_match_bg),
        opaque("find-current", t.find_current_bg),
        opaque("bracket-match-bg", t.bracket_match_bg),
        opaque("bracket-unmatched-bg", t.bracket_unmatched_bg),
        opaque("completion", t.completion_bg),
        opaque("completion-selected", t.completion_selected_bg),
        opaque("signature", t.signature_bg),
        opaque("border", t.border),
        opaque("control-border", t.control_border),
        opaque("plot-border", t.plot_border),
        Role {
            name: "shadow",
            color: t.shadow,
            translucent: true,
        },
        opaque("text", t.text_primary),
        opaque("dim", t.text_secondary),
        opaque("gutter-text", t.gutter_text),
        opaque("caret", t.caret),
        opaque("selection-text", t.selection_text),
        opaque("focus-ring", t.focus_ring),
        opaque("accent", t.accent),
        opaque("positive", t.positive),
        opaque("warning", t.warning),
        opaque("error", t.error),
        opaque("progress", t.progress),
        opaque("log-echo", t.log_echo),
        opaque("bracket-match", t.bracket_match),
        opaque("bracket-unmatched", t.bracket_unmatched),
        opaque("diagnostic-underline", t.diagnostic_underline),
        opaque("diagnostic-marker", t.diagnostic_marker),
        opaque("completion-border", t.completion_border),
        opaque("completion-detail", t.completion_detail),
        opaque("completion-function", t.completion_function),
        opaque("completion-variable", t.completion_variable),
        opaque("find-current-mark", t.find_current_mark),
        opaque("signature-border", t.signature_border),
        opaque("signature-active", t.signature_active),
        opaque("signature-text", t.signature_text),
        opaque("plot-empty-text", t.plot_empty_text),
    ];
    for (name, color) in [
        ("syntax-plain", t.syntax.plain),
        ("syntax-number", t.syntax.number),
        ("syntax-rational", t.syntax.rational),
        ("syntax-string", t.syntax.string),
        ("syntax-interpolation", t.syntax.interpolation),
        ("syntax-keyword", t.syntax.keyword),
        ("syntax-operator", t.syntax.operator),
        ("syntax-function", t.syntax.function),
        ("syntax-builtin", t.syntax.builtin),
        ("syntax-delimiter", t.syntax.delimiter),
        ("syntax-symbol", t.syntax.symbol),
    ] {
        roles.push(opaque(name, color));
    }
    roles
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The semantic families VIS-002 requires, each of which must exist in both themes. The names
    /// are role names from [`roles`], so this test fails if a family loses its token.
    const REQUIRED_ROLES: &[&str] = &[
        // surfaces
        "app",
        "panel",
        "editor",
        "header",
        "window",
        "field",
        "plot",
        // text
        "text",
        "dim",
        "gutter-text",
        // borders / focus
        "border",
        "control-border",
        "focus-ring",
        // selection
        "selection",
        "selection-text",
        // syntax (all eleven classes)
        "syntax-plain",
        "syntax-number",
        "syntax-rational",
        "syntax-string",
        "syntax-interpolation",
        "syntax-keyword",
        "syntax-operator",
        "syntax-function",
        "syntax-builtin",
        "syntax-delimiter",
        "syntax-symbol",
        // plot chrome
        "plot-border",
        "plot-empty-text",
        // semantic status
        "positive",
        "error",
        "warning",
        "progress",
        "accent",
    ];

    #[test]
    fn both_themes_define_every_semantic_role() {
        let dark = roles(&DARK);
        let light = roles(&LIGHT);
        assert_eq!(
            dark.len(),
            light.len(),
            "both themes must define the same number of roles"
        );
        let dark_names: Vec<&str> = dark.iter().map(|role| role.name).collect();
        let light_names: Vec<&str> = light.iter().map(|role| role.name).collect();
        assert_eq!(
            dark_names, light_names,
            "the two themes must define exactly the same role names"
        );
        for required in REQUIRED_ROLES {
            assert!(
                dark_names.contains(required),
                "dark theme is missing the {required} role"
            );
            assert!(
                light_names.contains(required),
                "light theme is missing the {required} role"
            );
        }
        assert_eq!(dark.len(), 62, "the token table gained or lost a role");
        assert_ne!(DARK.id, LIGHT.id);
    }

    #[test]
    fn no_painted_role_is_default_or_transparent() {
        for theme in [ThemeId::Dark, ThemeId::Light] {
            for role in roles(tokens_for(theme)) {
                assert!(
                    !matches!(role.color, Color32::TRANSPARENT | Color32::PLACEHOLDER),
                    "{:?} role {} is transparent/unset",
                    theme,
                    role.name
                );
                if role.translucent {
                    assert!(
                        role.color.a() > 0,
                        "{:?} role {} is a translucent role with zero alpha",
                        theme,
                        role.name
                    );
                } else {
                    assert_eq!(
                        role.color.a(),
                        255,
                        "{:?} role {} must be opaque (a default or unset colour)",
                        theme,
                        role.name
                    );
                }
            }
        }
    }

    #[test]
    fn every_visual_colour_is_derived_from_a_token() {
        for theme in [ThemeId::Dark, ThemeId::Light] {
            let t = tokens_for(theme);
            let visuals = visuals_for(theme);
            assert_eq!(visuals.panel_fill, t.app_bg);
            assert_eq!(visuals.window_fill, t.window_bg);
            assert_eq!(visuals.window_stroke.color, t.control_border);
            assert_eq!(visuals.extreme_bg_color, t.field_bg);
            assert_eq!(visuals.text_edit_bg_color, Some(t.field_bg));
            assert_eq!(visuals.code_bg_color, t.completion_bg);
            assert_eq!(visuals.override_text_color, Some(t.text_primary));
            assert_eq!(visuals.weak_text_color, Some(t.text_secondary));
            assert_eq!(visuals.selection.bg_fill, t.selection_bg);
            assert_eq!(visuals.selection.stroke.color, t.selection_text);
            assert_eq!(visuals.hyperlink_color, t.accent);
            assert_eq!(visuals.warn_fg_color, t.warning);
            assert_eq!(visuals.error_fg_color, t.error);
            assert_eq!(visuals.text_cursor.stroke.color, t.caret);
            assert_eq!(visuals.widgets.inactive.bg_fill, t.button_bg);
            assert_eq!(visuals.widgets.inactive.weak_bg_fill, t.button_bg);
            assert_eq!(visuals.widgets.hovered.bg_fill, t.button_hover_bg);
            assert_eq!(visuals.widgets.active.bg_fill, t.button_active_bg);
            assert_eq!(visuals.widgets.active.bg_stroke.color, t.focus_ring);
            assert_eq!(visuals.widgets.inactive.bg_stroke.color, t.control_border);
            assert_eq!(visuals.widgets.noninteractive.bg_stroke.color, t.border);
            assert_eq!(
                visuals.widgets.noninteractive.fg_stroke.color,
                t.text_secondary
            );
            let scroll = scroll_style();
            assert!(
                !scroll.floating,
                "scroll bars must not fade to zero opacity"
            );
            assert!(
                scroll.foreground_color,
                "scroll handle uses a token foreground"
            );
            for state in [
                visuals.widgets.noninteractive,
                visuals.widgets.inactive,
                visuals.widgets.hovered,
                visuals.widgets.active,
                visuals.widgets.open,
            ] {
                assert_ne!(state.bg_fill, Color32::TRANSPARENT);
                assert_ne!(
                    state.weak_bg_fill,
                    Color32::TRANSPARENT,
                    "a button fill fell through to egui's default"
                );
                assert_ne!(state.fg_stroke.color, Color32::TRANSPARENT);
            }
            // The active visuals must differ from egui's stock palette, or a theme switch would
            // silently leave controls on the default colours.
            let stock = if t.dark {
                Visuals::dark()
            } else {
                Visuals::light()
            };
            assert_ne!(visuals.panel_fill, stock.panel_fill);
            assert_ne!(visuals.window_fill, stock.window_fill);
            assert_ne!(
                visuals.widgets.inactive.weak_bg_fill,
                stock.widgets.inactive.weak_bg_fill
            );
            assert_ne!(
                visuals.widgets.hovered.weak_bg_fill,
                stock.widgets.hovered.weak_bg_fill
            );
        }
    }

    #[test]
    fn the_real_palettes_pass_every_contrast_requirement() {
        for theme in [ThemeId::Dark, ThemeId::Light] {
            let results = checks(theme);
            assert!(!results.is_empty());
            let failures: Vec<String> = results
                .iter()
                .filter(|result| !result.pass)
                .map(|result| {
                    format!(
                        "{} ratio {:.2} < {:.2}",
                        result.role,
                        result.ratio,
                        result.class.required()
                    )
                })
                .collect();
            assert!(
                failures.is_empty(),
                "{theme:?} contrast failures: {failures:#?}"
            );
            assert!(results
                .iter()
                .any(|result| result.class == PairClass::NormalText));
            assert!(results
                .iter()
                .any(|result| result.class == PairClass::NonText));
        }
    }

    #[test]
    fn the_contrast_checker_rejects_a_deliberately_failing_pair() {
        // Mid-grey on white is 3.03:1: it passes the large-text/non-text threshold and fails the
        // normal-text one, which proves the checker applies the right threshold to each class.
        let fg = Color32::from_rgb(0x8a, 0x8a, 0x8a);
        let bg = Color32::WHITE;
        let normal = check(
            ThemeId::Dark,
            "deliberate normal",
            fg,
            bg,
            PairClass::NormalText,
        );
        let large = check(
            ThemeId::Dark,
            "deliberate large",
            fg,
            bg,
            PairClass::LargeText,
        );
        let non_text = check(
            ThemeId::Dark,
            "deliberate non-text",
            fg,
            bg,
            PairClass::NonText,
        );
        assert!(normal.ratio > 3.0 && normal.ratio < 4.5, "{}", normal.ratio);
        assert!(!normal.pass, "normal text below 4.5:1 must fail");
        assert!(large.pass, "the same pair is acceptable as large text");
        assert!(
            non_text.pass,
            "the same pair is acceptable as a non-text indicator"
        );

        // A pair that fails even the 3:1 threshold must fail every class.
        let low = check(
            ThemeId::Light,
            "deliberate low",
            Color32::from_rgb(0x9a, 0x9a, 0x9a),
            Color32::WHITE,
            PairClass::NonText,
        );
        assert!(low.ratio < 3.0, "{}", low.ratio);
        assert!(!low.pass);
    }

    #[test]
    fn exemptions_are_measured_and_reported_not_ignored() {
        let results = checks(ThemeId::Dark);
        let exempts: Vec<&PairResult> = results
            .iter()
            .filter(|result| matches!(result.class, PairClass::Exempt(_)))
            .collect();
        assert!(!exempts.is_empty());
        for result in exempts {
            assert!(result.ratio > 0.0, "{} has no measured ratio", result.role);
            match result.class {
                PairClass::Exempt(reason) => assert!(
                    !reason.is_empty(),
                    "{} is exempt without a reason",
                    result.role
                ),
                _ => unreachable!(),
            }
        }
    }

    /// VIS-002 audit: no paint module may construct its own colour. `theme.rs` is the one place
    /// allowed to, so a literal anywhere else fails this test instead of bypassing both themes.
    #[test]
    fn no_paint_module_contains_a_hard_coded_colour() {
        let sources = [
            ("app.rs", include_str!("app.rs")),
            ("editor.rs", include_str!("editor.rs")),
            ("plot.rs", include_str!("plot.rs")),
            ("syntax.rs", include_str!("syntax.rs")),
            ("measure.rs", include_str!("measure.rs")),
        ];
        let forbidden = [
            "Color32::from_rgb",
            "Color32::from_rgba",
            "Color32::from_gray",
            "Color32::WHITE",
            "Color32::BLACK",
            "Color32::GRAY",
            "Color32::LIGHT_GRAY",
            "Color32::DARK_GRAY",
            "Color32::TRANSPARENT",
            "Color32::PLACEHOLDER",
            "Visuals::dark()",
            "Visuals::light()",
        ];
        for (name, source) in sources {
            for needle in forbidden {
                assert!(
                    !source.contains(needle),
                    "{name} still hard-codes {needle}; every painted colour must come from theme.rs"
                );
            }
            assert!(
                source.contains("theme::"),
                "{name} never calls the theme accessors"
            );
        }
    }

    #[test]
    fn the_report_prints_every_pair_and_its_ratio() {
        let (text, ok) = report();
        assert!(ok, "the shipped palettes must pass");
        let expected = checks(ThemeId::Dark).len() + checks(ThemeId::Light).len();
        let printed = text
            .lines()
            .filter(|line| line.starts_with("theme=") && line.contains("pair="))
            .count();
        assert_eq!(printed, expected, "the report must print every pair");
        assert!(text.contains("ratio="), "no ratios in the report");
    }

    #[test]
    fn luminance_and_compositing_match_the_wcag_definition() {
        let black = contrast_ratio(Color32::BLACK, Color32::WHITE);
        assert!((black - 21.0).abs() < 0.001, "black on white is 21:1");
        let same = contrast_ratio(Color32::from_rgb(1, 2, 3), Color32::from_rgb(1, 2, 3));
        assert!((same - 1.0).abs() < 0.001, "a colour against itself is 1:1");
        // 50% white over black is mid-grey, whose ratio to black is ~5.3:1.
        let blended = composite(
            Color32::from_rgba_unmultiplied(255, 255, 255, 128),
            Color32::BLACK,
        );
        let [r, g, b, a] = blended.to_srgba_unmultiplied();
        assert_eq!(a, 255, "the composited pixel is opaque");
        assert!(r > 120 && r < 140, "got {r}");
        assert!(g.abs_diff(r) <= 1 && b.abs_diff(r) <= 1, "neutral grey");
    }
}
