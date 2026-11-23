//! SurroundTag visual language: deep slate studio surfaces, cyan precision
//! accents, and amber surround/target accents.

use std::collections::BTreeMap;

use egui::{
    Color32, CornerRadius, FontFamily, FontId, Frame, Margin, Response, RichText, Stroke,
    TextStyle, Ui,
};

pub const BG: Color32 = Color32::from_rgb(0x13, 0x17, 0x20);
pub const CANVAS_BG: Color32 = Color32::from_rgb(0x0B, 0x0E, 0x14);
pub const CARD: Color32 = Color32::from_rgb(0x1B, 0x21, 0x30);
pub const CARD_ALT: Color32 = Color32::from_rgb(0x20, 0x27, 0x38);
pub const CARD_HIGH: Color32 = Color32::from_rgb(0x27, 0x30, 0x43);
pub const BORDER: Color32 = Color32::from_rgb(0x2A, 0x34, 0x46);
pub const BORDER_SOFT: Color32 = Color32::from_rgb(0x22, 0x2B, 0x3B);
pub const TEXT: Color32 = Color32::from_rgb(0xE6, 0xED, 0xF7);
pub const MUTED: Color32 = Color32::from_rgb(0x8C, 0x9A, 0xB0);
pub const DIM: Color32 = Color32::from_rgb(0x64, 0x72, 0x8A);
pub const CYAN: Color32 = Color32::from_rgb(0x38, 0xBD, 0xF8);
pub const CYAN_BRIGHT: Color32 = Color32::from_rgb(0x7D, 0xD3, 0xFC);
pub const CYAN_INK: Color32 = Color32::from_rgb(0x06, 0x1B, 0x29);
pub const CYAN_SOFT: Color32 = Color32::from_rgb(0x12, 0x36, 0x4A);
pub const AMBER: Color32 = Color32::from_rgb(0xF5, 0x9E, 0x0B);
pub const AMBER_BRIGHT: Color32 = Color32::from_rgb(0xFF, 0xD0, 0x8A);
pub const AMBER_INK: Color32 = Color32::from_rgb(0x2A, 0x18, 0x00);
pub const AMBER_SOFT: Color32 = Color32::from_rgb(0x45, 0x31, 0x0C);
pub const SUCCESS: Color32 = Color32::from_rgb(0x5C, 0xD6, 0x8C);
pub const ERROR: Color32 = Color32::from_rgb(0xFF, 0x68, 0x70);
pub const WARNING: Color32 = AMBER;
pub const PAPER: Color32 = Color32::from_rgb(0xF3, 0xF0, 0xE8);
pub const PAPER_INK: Color32 = Color32::from_rgb(0x1E, 0x27, 0x36);

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|style| {
        let mut visuals = egui::Visuals::dark();
        visuals.dark_mode = true;
        visuals.override_text_color = Some(TEXT);
        visuals.weak_text_color = Some(MUTED);
        visuals.panel_fill = BG;
        visuals.window_fill = CARD_ALT;
        visuals.extreme_bg_color = Color32::from_rgb(0x0C, 0x10, 0x18);
        visuals.faint_bg_color = CARD;
        visuals.code_bg_color = CARD_ALT;
        visuals.window_corner_radius = CornerRadius::same(12);
        visuals.window_stroke = Stroke::new(1.0, BORDER);
        visuals.menu_corner_radius = CornerRadius::same(8);
        visuals.warn_fg_color = WARNING;
        visuals.error_fg_color = ERROR;
        visuals.hyperlink_color = CYAN;
        visuals.selection = egui::style::Selection {
            bg_fill: CYAN_SOFT,
            stroke: Stroke::new(1.0, CYAN),
        };
        visuals.slider_trailing_fill = true;
        visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
        visuals.button_frame = true;
        visuals.striped = false;

        let widgets = &mut visuals.widgets;
        widgets.noninteractive.bg_fill = CARD;
        widgets.noninteractive.weak_bg_fill = CARD;
        widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_SOFT);
        widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        widgets.noninteractive.corner_radius = CornerRadius::same(8);

        widgets.inactive.bg_fill = CARD_ALT;
        widgets.inactive.weak_bg_fill = CARD_ALT;
        widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        widgets.inactive.corner_radius = CornerRadius::same(8);

        widgets.hovered.bg_fill = CARD_HIGH;
        widgets.hovered.weak_bg_fill = CARD_HIGH;
        widgets.hovered.bg_stroke = Stroke::new(1.0, CYAN);
        widgets.hovered.fg_stroke = Stroke::new(1.0, CYAN_BRIGHT);
        widgets.hovered.corner_radius = CornerRadius::same(8);

        widgets.active.bg_fill = CYAN_SOFT;
        widgets.active.weak_bg_fill = CYAN_SOFT;
        widgets.active.bg_stroke = Stroke::new(1.0, CYAN_BRIGHT);
        widgets.active.fg_stroke = Stroke::new(1.0, CYAN_BRIGHT);
        widgets.active.corner_radius = CornerRadius::same(8);

        widgets.open.bg_fill = CARD_HIGH;
        widgets.open.weak_bg_fill = CARD_HIGH;
        widgets.open.bg_stroke = Stroke::new(1.0, BORDER);
        widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
        widgets.open.corner_radius = CornerRadius::same(8);

        style.visuals = visuals;
        style.spacing.item_spacing = egui::vec2(8.0, 7.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        style.spacing.interact_size = egui::vec2(36.0, 24.0);
        style.spacing.slider_width = 118.0;
        style.spacing.combo_width = 132.0;
        style.spacing.indent = 14.0;
        style.spacing.scroll = egui::style::ScrollStyle::solid();
        style.spacing.scroll.bar_width = 8.0;
        style.spacing.scroll.bar_inner_margin = 4.0;
        style.spacing.scroll.bar_outer_margin = 4.0;
        style.animation_time = 0.12;

        style.text_styles = BTreeMap::from([
            (
                TextStyle::Heading,
                FontId::new(23.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(12.5, FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(10.8, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(12.0, FontFamily::Monospace),
            ),
        ]);
    });
}

pub fn card() -> Frame {
    Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(12))
}

pub fn inset() -> Frame {
    Frame::new()
        .fill(CARD_ALT)
        .stroke(Stroke::new(1.0, BORDER_SOFT))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(10))
}

pub fn section_title(ui: &mut Ui, title: &str, detail: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(13.5).strong().color(TEXT));
        if let Some(detail) = detail {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(detail).size(10.5).color(MUTED));
            });
        }
    });
}

pub fn panel_header(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).size(15.0).strong().color(TEXT));
    ui.label(RichText::new(subtitle).size(10.5).color(MUTED));
}

pub fn hint(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(10.8).color(MUTED));
}

pub fn pill(ui: &mut Ui, text: impl Into<String>, color: Color32) {
    let text = text.into();
    let galley =
        ui.painter()
            .layout_no_wrap(text, FontId::new(10.5, FontFamily::Proportional), color);
    let padding = egui::vec2(7.0, 2.5);
    let size = galley.size() + padding * 2.0;
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(9),
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 28),
    );
    ui.painter().galley(rect.min + padding, galley, color);
}

pub fn primary_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(
        egui::Button::new(
            RichText::new(text.into())
                .size(12.5)
                .strong()
                .color(CYAN_INK),
        )
        .fill(CYAN)
        .stroke(Stroke::new(1.0, CYAN_BRIGHT))
        .corner_radius(CornerRadius::same(7)),
    )
}

pub fn amber_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(
        egui::Button::new(
            RichText::new(text.into())
                .size(12.5)
                .strong()
                .color(AMBER_INK),
        )
        .fill(AMBER)
        .stroke(Stroke::new(1.0, AMBER_BRIGHT))
        .corner_radius(CornerRadius::same(7)),
    )
}

pub fn ghost_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text.into()).size(12.0).color(TEXT))
            .fill(CARD_ALT)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(7)),
    )
}

pub fn danger_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text.into()).size(12.0).color(ERROR))
            .fill(CARD_ALT)
            .stroke(Stroke::new(1.0, Color32::from_rgb(0x6B, 0x2A, 0x30)))
            .corner_radius(CornerRadius::same(7)),
    )
}

pub fn toggle_chip(ui: &mut Ui, selected: bool, text: impl Into<String>) -> Response {
    let (fill, stroke, ink) = if selected {
        (CYAN_SOFT, CYAN, CYAN_BRIGHT)
    } else {
        (CARD_ALT, BORDER, MUTED)
    };
    ui.add(
        egui::Button::new(RichText::new(text.into()).size(10.8).color(ink))
            .fill(fill)
            .stroke(Stroke::new(1.0, stroke))
            .corner_radius(CornerRadius::same(7))
            .min_size(egui::vec2(0.0, 22.0)),
    )
}

pub fn divider(ui: &mut Ui) {
    ui.add_space(2.0);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0, BORDER_SOFT),
    );
    ui.add_space(2.0);
}

pub fn color32(rgba: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3])
}

pub fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub fn label_wrapped(ui: &mut Ui, text: &str, size: f32, color: Color32) {
    ui.add(egui::Label::new(RichText::new(text).size(size).color(color)).wrap());
}
