//! VIS-006: the reusable control states.
//!
//! Every interactive control in the workbench is one of [`ControlKind`] and is in one of
//! [`ControlState`] at a time. [`ControlFlags`] is the *input*: the widget's enabled flag and the
//! interaction flags egui's [`Response`] reports. [`ControlFlags::resolve`] is the one
//! state-selection rule; [`paint`] maps a `(kind, state)` to the visible paint (fill, border,
//! text, cue). The matrix window ([`matrix_window`]) renders the whole table in the running
//! window, so the VIS-006 capture is the same table the widgets use.
//!
//! # Why disabled is not "lower alpha"
//!
//! egui paints a disabled widget by multiplying its whole style by `disabled_alpha` (0.5). That
//! makes a disabled control look like a translucent enabled one, which is exactly the failure
//! VIS-006 names. The disabled paint here is instead a *different* fill (the recessed field
//! surface), a **dashed** border, a **diagonal hatch** over the control and the opaque secondary
//! text token — a shape carrier, not an opacity. [`paint_disabled_cue`] draws that cue over a
//! real focusable widget after egui painted it, and the widget's AccessKit node keeps the
//! `disabled` state egui set for `add_enabled(false, ...)`.
//!
//! # Precedence
//!
//! `Disabled > Error > Pressed > Focused > Selected > Hovered > Rest`. A disabled control is
//! never also "hovered", an error field shows the error even while pressed, focus outranks
//! selection because the keyboard needs to find the control, and selection outranks hover.

use egui::{
    Color32, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui,
};

use super::semantic::SemanticRole;
use super::{theme, type_scale};

/// The controls the state table covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ControlKind {
    /// A command button, including the toolbar and dialog buttons.
    Button,
    /// A single-line text input.
    Input,
    /// A document or view tab.
    Tab,
    /// A menu button or a menu row.
    Menu,
    /// A popup window or completion/signature surface.
    Popup,
    /// A pane divider.
    Splitter,
}

impl ControlKind {
    /// Every kind, in the order the matrix renders them.
    pub const ALL: [ControlKind; 6] = [
        ControlKind::Button,
        ControlKind::Input,
        ControlKind::Tab,
        ControlKind::Menu,
        ControlKind::Popup,
        ControlKind::Splitter,
    ];

    /// The kind's name in the matrix and the report.
    pub fn name(self) -> &'static str {
        match self {
            ControlKind::Button => "Button",
            ControlKind::Input => "Input",
            ControlKind::Tab => "Tab",
            ControlKind::Menu => "Menu",
            ControlKind::Popup => "Popup",
            ControlKind::Splitter => "Splitter",
        }
    }
}

/// The seven states every control implements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ControlState {
    /// Enabled, not interacted with.
    Rest,
    /// The pointer is over the control.
    Hover,
    /// The pointer (or space/enter) is holding the control down.
    Pressed,
    /// Not actionable: the enabled flag is false.
    Disabled,
    /// The control owns keyboard focus.
    Focused,
    /// A tab/view/row that is selected among its siblings.
    Selected,
    /// The control's value is invalid (the precision field, a rejected value).
    Error,
}

impl ControlState {
    /// Every state, in precedence order (strongest first).
    pub const ALL: [ControlState; 7] = [
        ControlState::Disabled,
        ControlState::Error,
        ControlState::Pressed,
        ControlState::Focused,
        ControlState::Selected,
        ControlState::Hover,
        ControlState::Rest,
    ];

    /// The state's name in the matrix and the report.
    pub fn name(self) -> &'static str {
        match self {
            ControlState::Rest => "Rest",
            ControlState::Hover => "Hover",
            ControlState::Pressed => "Pressed",
            ControlState::Disabled => "Disabled",
            ControlState::Focused => "Focused",
            ControlState::Selected => "Selected",
            ControlState::Error => "Error",
        }
    }

    /// The non-colour carrier this state adds, as the matrix spells it out.
    pub fn carrier(self) -> &'static str {
        match self {
            ControlState::Rest => "border + fill",
            ControlState::Hover => "raised fill + brighter border",
            ControlState::Pressed => "sunken inset + 2 px border",
            ControlState::Disabled => "dashed border + diagonal hatch + `not actionable`",
            ControlState::Focused => "2 px focus ring",
            ControlState::Selected => "accent bar + selected fill",
            ControlState::Error => "2 px error border + `!`",
        }
    }

    /// The short carrier name the matrix column uses (the long form is the report's).
    pub fn carrier_short(self) -> &'static str {
        match self {
            ControlState::Rest => "border",
            ControlState::Hover => "raised",
            ControlState::Pressed => "inset",
            ControlState::Disabled => "dashed+hatch",
            ControlState::Focused => "ring",
            ControlState::Selected => "bar",
            ControlState::Error => "border+!",
        }
    }
}

/// The input flags a widget reports for one pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlFlags {
    /// `Response::enabled`.
    pub enabled: bool,
    /// `Response::hovered`.
    pub hovered: bool,
    /// `Response::is_pointer_button_down_on` or a keyboard activation in progress.
    pub pressed: bool,
    /// `Response::has_focus`.
    pub focused: bool,
    /// The widget is the selected one among its siblings.
    pub selected: bool,
    /// The widget's value is invalid.
    pub error: bool,
}

impl Default for ControlFlags {
    fn default() -> Self {
        ControlFlags {
            enabled: true,
            hovered: false,
            pressed: false,
            focused: false,
            selected: false,
            error: false,
        }
    }
}

impl ControlFlags {
    /// Reads the interaction flags from a real egui response.
    ///
    /// `selected` and `error` are not response flags — egui has no notion of "this tab is the
    /// selected one" or "this field's value is invalid" — so the caller passes them; everything
    /// else is the widget's own report, which is what makes the state resolution testable on the
    /// real widget path.
    pub fn from_response(response: &Response, selected: bool, error: bool) -> ControlFlags {
        ControlFlags {
            enabled: response.enabled(),
            hovered: response.hovered(),
            pressed: response.is_pointer_button_down_on() || response.clicked(),
            focused: response.has_focus(),
            selected,
            error,
        }
    }

    /// Resolves the one state this pass paints, by the documented precedence.
    pub fn resolve(&self) -> ControlState {
        if !self.enabled {
            ControlState::Disabled
        } else if self.error {
            ControlState::Error
        } else if self.pressed {
            ControlState::Pressed
        } else if self.focused {
            ControlState::Focused
        } else if self.selected {
            ControlState::Selected
        } else if self.hovered {
            ControlState::Hover
        } else {
            ControlState::Rest
        }
    }
}

/// The shape carrier a state adds on top of its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    /// No extra shape.
    None,
    /// Dashed outline plus a diagonal hatch (disabled).
    DashedHatch,
    /// A second inset outline (focus).
    Ring,
    /// A leading accent bar (selected).
    Bar,
    /// A leading `!` (error).
    Bang,
}

/// The complete paint for one `(kind, state)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlPaint {
    /// The control's fill.
    pub fill: Color32,
    /// The border colour.
    pub border: Color32,
    /// The border width in points.
    pub border_width: f32,
    /// True when the border is dashed.
    pub dashed: bool,
    /// The text colour. Always opaque, including for disabled.
    pub text: Color32,
    /// The content's inset (pressed controls sink by one point).
    pub inset: f32,
    /// The extra shape carrier.
    pub cue: Cue,
}

/// The paint for one kind in one state.
///
/// The rest/hover/pressed/focused fills are the same `WidgetVisuals` fills
/// [`theme::visuals`] gives egui's `inactive`/`hovered`/`active` widgets, so the table cannot
/// describe a button egui does not paint. The disabled/error/selected paints are the additions
/// this module owns.
pub fn paint(kind: ControlKind, state: ControlState) -> ControlPaint {
    let rest_fill = match kind {
        ControlKind::Input | ControlKind::Popup => theme::field_bg(),
        ControlKind::Splitter => theme::splitter_bg(),
        ControlKind::Button | ControlKind::Tab | ControlKind::Menu => theme::button_bg(),
    };
    let hover_fill = match kind {
        ControlKind::Splitter => theme::splitter_hot(),
        _ => theme::button_hover_bg(),
    };
    match state {
        ControlState::Rest => ControlPaint {
            fill: rest_fill,
            border: theme::control_border(),
            border_width: 1.0,
            dashed: false,
            text: theme::text(),
            inset: 0.0,
            cue: Cue::None,
        },
        ControlState::Hover => ControlPaint {
            fill: hover_fill,
            border: theme::dim(),
            border_width: 1.0,
            dashed: false,
            text: theme::text(),
            inset: 0.0,
            cue: Cue::None,
        },
        ControlState::Pressed => ControlPaint {
            fill: theme::button_active_bg(),
            border: theme::focus_ring(),
            border_width: 2.0,
            dashed: false,
            text: theme::text(),
            inset: 1.0,
            cue: Cue::None,
        },
        // VIS-006: disabled is a different surface, a dashed border, a hatch and the opaque
        // secondary text token. It is never the enabled paint at half opacity.
        ControlState::Disabled => ControlPaint {
            fill: theme::app_bg(),
            border: theme::control_border(),
            border_width: 1.0,
            dashed: true,
            text: theme::dim(),
            inset: 0.0,
            cue: Cue::DashedHatch,
        },
        ControlState::Focused => ControlPaint {
            fill: rest_fill,
            border: theme::focus_ring(),
            border_width: 2.0,
            dashed: false,
            text: theme::text(),
            inset: 0.0,
            cue: Cue::Ring,
        },
        ControlState::Selected => ControlPaint {
            fill: theme::button_active_bg(),
            border: theme::accent(),
            border_width: 2.0,
            dashed: false,
            text: theme::text(),
            inset: 0.0,
            cue: Cue::Bar,
        },
        ControlState::Error => ControlPaint {
            fill: theme::field_bg(),
            border: theme::error(),
            border_width: 2.0,
            dashed: false,
            text: theme::error(),
            inset: 0.0,
            cue: Cue::Bang,
        },
    }
}

/// Paints one specimen rectangle for the matrix, or a custom control.
///
/// `label` is the visible control text; the state's cue is painted on top of the fill so the
/// specimen shows exactly what [`paint`] specifies.
pub fn paint_specimen(ui: &mut Ui, kind: ControlKind, state: ControlState, label: &str) -> Rect {
    let desired = egui::vec2(150.0, 22.0);
    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    let paint = paint(kind, state);
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, paint.fill);
    if paint.dashed {
        paint_dashed_rect(painter, rect.shrink(0.5), paint.border, 4.0, 3.0);
    } else {
        painter.rect_stroke(
            rect.shrink(paint.border_width * 0.5),
            2.0,
            Stroke::new(paint.border_width, paint.border),
            StrokeKind::Inside,
        );
    }
    if paint.cue == Cue::DashedHatch {
        paint_hatch_edges(painter, rect.shrink(2.0), paint.border);
    }
    if paint.cue == Cue::Ring {
        painter.rect_stroke(
            rect.shrink(3.0),
            1.0,
            Stroke::new(1.0, paint.border),
            StrokeKind::Inside,
        );
    }
    if paint.cue == Cue::Bar {
        let bar = Rect::from_min_size(
            egui::pos2(rect.left() + 2.0, rect.top() + 3.0),
            egui::vec2(3.0, rect.height() - 6.0),
        );
        painter.rect_filled(bar, 1.0, paint.border);
    }
    let mut text = label.to_string();
    if paint.cue == Cue::Bang {
        text.insert_str(0, "! ");
    }
    let text_pos = rect.center() + egui::vec2(0.0, paint.inset);
    painter.text(
        text_pos,
        egui::Align2::CENTER_CENTER,
        text,
        type_scale::Role::Caption.font(),
        paint.text,
    );
    rect
}

/// VIS-006: draws the disabled cue over a control egui painted as disabled.
///
/// It is deliberately a *shape* addition: a dashed outline and a diagonal hatch over the widget's
/// rectangle. The widget's own text and fill stay egui's disabled paint; the cue is what makes
/// "not actionable" visible without relying on reduced opacity.
pub fn paint_disabled_cue(ui: &Ui, rect: Rect) {
    let painter = ui.painter();
    paint_dashed_rect(painter, rect.shrink(1.0), theme::control_border(), 4.0, 3.0);
    paint_hatch_edges(painter, rect.shrink(3.0), theme::control_border());
}

/// The hatch is painted in two narrow edge bands, never across the control's label: the shape
/// says "not actionable" without costing the disabled control's text its legibility (VIS-005).
fn paint_hatch_edges(painter: &egui::Painter, rect: Rect, color: Color32) {
    let band = 3.0_f32.min(rect.width() * 0.2);
    if band <= 0.5 {
        return;
    }
    paint_hatch(
        painter,
        Rect::from_min_max(rect.min, egui::pos2(rect.left() + band, rect.bottom())),
        color,
    );
    paint_hatch(
        painter,
        Rect::from_min_max(egui::pos2(rect.right() - band, rect.top()), rect.max),
        color,
    );
}

/// A dashed rectangle: eight segments per edge, `dash` on / `gap` off.
fn paint_dashed_rect(
    painter: &egui::Painter,
    rect: Rect,
    color: Color32,
    dash: f32,
    gap: f32,
) {
    let step = dash + gap;
    let stroke = Stroke::new(1.0, color);
    let mut x = rect.left();
    while x < rect.right() {
        let x2 = (x + dash).min(rect.right());
        painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x2, rect.top())], stroke);
        painter.line_segment([
            egui::pos2(x, rect.bottom()),
            egui::pos2(x2, rect.bottom()),
        ], stroke);
        x += step;
    }
    let mut y = rect.top();
    while y < rect.bottom() {
        let y2 = (y + dash).min(rect.bottom());
        painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.left(), y2)], stroke);
        painter.line_segment([
            egui::pos2(rect.right(), y),
            egui::pos2(rect.right(), y2),
        ], stroke);
        y += step;
    }
}

/// A diagonal hatch, clipped to `rect`, drawn with the control border token.
fn paint_hatch(painter: &egui::Painter, rect: Rect, color: Color32) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let painter = painter.with_clip_rect(rect);
    let stroke = Stroke::new(1.0, color);
    let step = 8.0;
    let mut x = rect.left() - rect.height();
    while x < rect.right() {
        painter.line_segment(
            [
                egui::pos2(x, rect.bottom()),
                egui::pos2(x + rect.height(), rect.top()),
            ],
            stroke,
        );
        x += step;
    }
}

/// VIS-006: the state matrix window.
///
/// Every `kind × state` cell is a real painted specimen with its accessible label
/// (`"Button · Disabled"`), so a UIA dump of the window names all 42 combinations and the
/// screenshot shows the paint table. The window is opened from Help > Control states.
pub fn matrix_window(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }
    egui::Window::new("Control states (VIS-006)")
        .id(egui::Id::unique("vis_006_control_states"))
        .collapsible(false)
        .resizable(false)
        .default_width(980.0)
        .max_width(1000.0)
        .default_pos(egui::pos2(60.0, 60.0))
        .open(open)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(
                    "One specimen per control kind and state. Disabled is a dashed border plus a \
                     diagonal hatch and the opaque secondary text token, never the enabled paint \
                     at half opacity.",
                )
                .font(type_scale::Role::Caption.font())
                .color(theme::dim()),
            );
            egui::ScrollArea::vertical()
                .id_salt("vis_006_states_scroll")
                .max_height((ctx.content_rect().height() - 120.0).clamp(180.0, 760.0))
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        for pair in ControlKind::ALL.chunks(2) {
                            ui.vertical(|ui| {
                                for kind in pair {
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(kind.name())
                                            .font(type_scale::Role::Body.font())
                                            .color(theme::accent()),
                                    );
                                    for state in ControlState::ALL {
                                        ui.horizontal(|ui| {
                                            let label = format!("{} · {}", kind.name(), state.name());
                                            // A real Text node per row, so a UIA dump names every
                                            // kind × state combination instead of only the
                                            // painted specimen.
                                            ui.label(
                                                RichText::new(format!("{:<16}", label))
                                                    .font(type_scale::Role::Caption.font())
                                                    .color(theme::text()),
                                            );
                                            paint_specimen(ui, *kind, state, kind.name());
                                            ui.label(
                                                RichText::new(state.carrier_short())
                                                    .font(type_scale::Role::Caption.font())
                                                    .color(theme::dim()),
                                            );
                                        });
                                    }
                                }
                            });
                        }
                    });
                });
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "Semantic roles: {} (text marker + shape carrier each)",
                    SemanticRole::ALL.len()
                ))
                .font(type_scale::Role::Caption.font())
                .color(theme::dim()),
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Context;

    fn run(ctx: &Context, screen: egui::Vec2, events: Vec<egui::Event>) -> Option<Response> {
        let mut captured = None;
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, screen)),
            ..Default::default()
        };
        input.events = events;
        let mut output = ctx.run_ui(input, |ui| {
            let response = ui.add_enabled(
                true,
                egui::Button::new(RichText::new("Run").font(type_scale::Role::Body.font())),
            );
            captured = Some(response);
        });
        output.textures_delta.clear();
        captured
    }

    #[test]
    fn every_kind_has_every_state_and_each_state_paints_differently() {
        for kind in ControlKind::ALL {
            let paints: Vec<ControlPaint> = ControlState::ALL
                .iter()
                .map(|state| paint(kind, *state))
                .collect();
            for (index, first) in paints.iter().enumerate() {
                for (other, second) in paints.iter().enumerate().skip(index + 1) {
                    assert_ne!(
                        first, second,
                        "{}: {} and {} paint identically",
                        kind.name(),
                        ControlState::ALL[index].name(),
                        ControlState::ALL[other].name()
                    );
                }
            }
        }
    }

    #[test]
    fn disabled_is_a_shape_and_an_opaque_token_not_lower_alpha() {
        for kind in ControlKind::ALL {
            let disabled = paint(kind, ControlState::Disabled);
            let rest = paint(kind, ControlState::Rest);
            assert!(
                disabled.dashed,
                "{}: disabled must have a dashed border",
                kind.name()
            );
            assert_eq!(
                disabled.cue,
                Cue::DashedHatch,
                "{}: disabled must carry the dashed+hatch shape",
                kind.name()
            );
            assert_eq!(
                disabled.text.a(),
                255,
                "{}: disabled text must be an opaque token, not alpha",
                kind.name()
            );
            assert_eq!(rest.text.a(), 255);
            assert_ne!(
                disabled.fill, rest.fill,
                "{}: disabled must use a different surface",
                kind.name()
            );
            assert!(
                disabled.border_width > 0.0 && rest.border_width > 0.0,
                "{}: both states keep an outline",
                kind.name()
            );
        }
    }

    #[test]
    fn every_state_names_a_non_colour_carrier() {
        for state in ControlState::ALL {
            assert!(!state.carrier().is_empty(), "{}", state.name());
        }
        // The disabled carrier is the one VIS-006 specifically requires to be more than colour.
        assert!(ControlState::Disabled.carrier().contains("hatch"));
        assert!(ControlState::Disabled.carrier().contains("not actionable"));
    }

    #[test]
    fn the_state_resolves_from_a_real_widget_response() {
        let ctx = Context::default();
        let screen = egui::vec2(400.0, 200.0);
        // Rest: the pointer is elsewhere.
        let first = run(&ctx, screen, Vec::new()).expect("a response");
        let center = first.rect.center();
        assert_eq!(
            ControlFlags::from_response(&first, false, false).resolve(),
            ControlState::Rest
        );
        // Hover: the pointer is over the button.
        let response = run(
            &ctx,
            screen,
            vec![egui::Event::PointerMoved(center)],
        )
        .expect("a response");
        let flags = ControlFlags::from_response(&response, false, false);
        assert!(flags.hovered, "the pointer must hover the button");
        assert_eq!(flags.resolve(), ControlState::Hover);
        // Pressed: the primary button is held down on it.
        run(&ctx, screen, vec![egui::Event::PointerMoved(center)]);
        let response = run(
            &ctx,
            screen,
            vec![egui::Event::PointerButton {
                pos: center,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            }],
        )
        .expect("a response");
        let flags = ControlFlags::from_response(&response, false, false);
        assert_eq!(flags.resolve(), ControlState::Pressed, "flags={flags:?}");
        // Focused: the widget owns keyboard focus with the pointer away.
        let id = response.id;
        run(
            &ctx,
            screen,
            vec![egui::Event::PointerButton {
                pos: center,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            }],
        );
        ctx.memory_mut(|memory| memory.request_focus(id));
        let response = run(&ctx, screen, Vec::new()).expect("a response");
        let flags = ControlFlags::from_response(&response, false, false);
        assert!(flags.focused, "the widget must own focus: {flags:?}");
        assert_eq!(flags.resolve(), ControlState::Focused);
        // Selected and error are the caller's flags on the same widget path; focus outranks both
        // while it is held, so these use the same flags with focus released.
        let flags = ControlFlags {
            focused: false,
            selected: true,
            ..ControlFlags::from_response(&response, false, false)
        };
        assert_eq!(flags.resolve(), ControlState::Selected);
        let flags = ControlFlags {
            focused: false,
            error: true,
            ..ControlFlags::from_response(&response, false, false)
        };
        assert_eq!(flags.resolve(), ControlState::Error);
        // Disabled dominates every interaction flag.
        let disabled = ControlFlags {
            enabled: false,
            hovered: true,
            pressed: true,
            focused: true,
            selected: true,
            error: true,
        };
        assert_eq!(disabled.resolve(), ControlState::Disabled);
    }

    #[test]
    fn a_disabled_widget_reports_disabled_from_its_own_response() {
        let ctx = Context::default();
        let mut flags = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 120.0))),
                ..Default::default()
            },
            |ui| {
                let response = ui.add_enabled(false, egui::Button::new("Export SVG..."));
                flags = Some(ControlFlags::from_response(&response, false, false));
            },
        );
        output.textures_delta.clear();
        let flags = flags.expect("a response");
        assert!(!flags.enabled);
        assert_eq!(flags.resolve(), ControlState::Disabled);
    }

    #[test]
    fn the_disabled_cue_paints_dashes_and_a_hatch() {
        let ctx = Context::default();
        let mut line_segments = 0usize;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 120.0))),
                ..Default::default()
            },
            |ui| {
                paint_disabled_cue(ui, Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(120.0, 24.0)));
            },
        );
        output.textures_delta.clear();
        fn walk(shape: &egui::Shape, lines: &mut usize) {
            match shape {
                egui::Shape::LineSegment { .. } => *lines += 1,
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, lines);
                    }
                }
                _ => {}
            }
        }
        for clipped in &output.shapes {
            walk(&clipped.shape, &mut line_segments);
        }
        assert!(
            line_segments >= 20,
            "the disabled cue must paint dashes and a hatch, saw {line_segments} segments"
        );
    }

    #[test]
    fn the_matrix_window_names_every_kind_and_state() {
        let ctx = Context::default();
        let mut open = true;
        // An egui window measures its area on the first pass and paints its content on the next
        // one, exactly as it does in the running window; the third pass is the stable frame.
        let mut output = None;
        for _ in 0..3 {
            let mut pass = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1600.0, 1000.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    ui.label("root");
                    let ctx = ctx.clone();
                    matrix_window(&ctx, &mut open);
                },
            );
            pass.textures_delta.clear();
            output = Some(pass);
        }
        let output = output.expect("three passes");
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.text().to_string()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, out);
                    }
                }
                _ => {}
            }
        }
        let mut painted = Vec::new();
        for clipped in &output.shapes {
            walk(&clipped.shape, &mut painted);
        }
        for kind in ControlKind::ALL {
            for state in ControlState::ALL {
                let label = format!("{} · {}", kind.name(), state.name());
                assert!(
                    painted.iter().any(|text| text.trim() == label),
                    "the matrix must name {label:?}; painted: {painted:?}"
                );
            }
        }
    }
}
