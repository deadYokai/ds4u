use egui::{Context, Color32, CornerRadius, vec2};

pub fn apply_style(ctx: &Context) {
    let mut style = (*ctx.style()).clone();

    style.visuals.dark_mode = true;
    style.visuals.window_fill = Color32::from_rgb(12, 18, 28);
    style.visuals.panel_fill = Color32::from_rgb(16, 24, 36);
    style.visuals.extreme_bg_color = Color32::from_rgb(8, 12, 20);

    let accent_color = Color32::from_rgb(0, 122, 250);
    style.visuals.selection.bg_fill = accent_color;
    style.visuals.widgets.active.bg_fill = accent_color;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 60, 90);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(30, 45, 70);

    style.visuals.window_corner_radius = CornerRadius::same(8);
    style.visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(6);

    style.spacing.item_spacing = vec2(10.0, 10.0);
    style.spacing.button_padding = vec2(16.0, 8.0);

    ctx.set_style(style);
}
