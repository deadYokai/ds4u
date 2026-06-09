use egui::style::ScrollStyle;
use egui::{Color32, Context, CornerRadius, FontFamily, FontId, Stroke, TextStyle, vec2};

use crate::theme::Theme;

pub fn apply_style(ctx: &Context, theme: &Theme) {
    let mut style = (*ctx.style()).clone();
    let c = &theme.colors;

    style.visuals.dark_mode = theme.dark_mode;
    style.visuals.window_fill = c.window_bg();
    style.visuals.panel_fill = c.panel_bg();
    style.visuals.extreme_bg_color = c.extreme_bg();

    style.visuals.selection.bg_fill = c.accent();
    style.visuals.selection.stroke = Stroke::new(1.0, c.accent());

    style.visuals.warn_fg_color = c.warning();
    style.visuals.error_fg_color = c.error();

    let transparent = Color32::TRANSPARENT;
    style.visuals.widgets.noninteractive.bg_fill = transparent;
    style.visuals.widgets.noninteractive.weak_bg_fill = transparent;
    style.visuals.widgets.inactive.bg_fill = transparent;
    style.visuals.widgets.inactive.weak_bg_fill = transparent;

    style.visuals.widgets.active.bg_fill = c.accent();
    style.visuals.widgets.active.weak_bg_fill = c.accent();
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, c.accent());
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.5, c.text());

    style.visuals.override_text_color = Some(c.text());

    style.visuals.window_corner_radius = CornerRadius::same(4);
    style.visuals.widgets.noninteractive.corner_radius = CornerRadius::same(2);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(2);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(2);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(2);

    let sep = Color32::from_rgba_unmultiplied(c.text().r(), c.text().g(), c.text().b(), 97);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, sep);
    style.visuals.window_stroke = Stroke::new(1.0, sep);
    style.visuals.widgets.inactive.bg_fill =
        Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 97);
    style.visuals.widgets.hovered.bg_fill =
        Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 166);

    style.spacing.item_spacing = vec2(8.0, 6.0);
    style.spacing.button_padding = vec2(14.0, 6.0);
    style.spacing.slider_width = 220.0;
    style.spacing.scroll = ScrollStyle {
        bar_width: 8.0,
        bar_inner_margin: 0.0,
        bar_outer_margin: 2.0,
        floating: false,
        ..ScrollStyle::solid()
    };

    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(27.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
        (
            TextStyle::Button,
            FontId::new(15.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
    ]
    .into_iter()
    .collect();

    ctx.set_style(style);
}
