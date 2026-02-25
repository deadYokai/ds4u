use egui::{Context, CornerRadius, vec2};

use crate::theme::Theme;

pub fn apply_style(ctx: &Context, theme: &Theme) {
    let mut style = (*ctx.style()).clone();
    let c = &theme.colors;

    style.visuals.dark_mode = theme.dark_mode;
    style.visuals.window_fill = c.window_bg();
    style.visuals.panel_fill = c.panel_bg();
    style.visuals.extreme_bg_color = c.extreme_bg();

    style.visuals.selection.bg_fill = c.accent();
    style.visuals.widgets.active.bg_fill = c.accent();
    style.visuals.widgets.hovered.bg_fill = c.widget_hovered();
    style.visuals.widgets.inactive.bg_fill = c.widget_inactive();
    style.visuals.widgets.noninteractive.bg_fill = c.widget_inactive();

    style.visuals.override_text_color = Some(c.text());

    style.visuals.window_corner_radius = CornerRadius::same(8);
    style.visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(6);

    style.spacing.item_spacing = vec2(10.0, 10.0);
    style.spacing.button_padding = vec2(16.0, 8.0);

    ctx.set_style(style);
}
