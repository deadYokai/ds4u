use egui::{
    Color32, CornerRadius, Rect, Response, RichText, Sense, Stroke, Ui, UiBuilder, pos2, vec2,
};

use crate::theme::ThemeColors;

pub const ROW_HEIGHT: f32 = 50.0;
pub const ROW_PAD_X: f32 = 26.0;
pub const LBL_WIDTH: f32 = 180.0;
pub const VAL_WIDTH: f32 = 64.0;

#[inline]
pub fn with_alpha(color: Color32, a: u8) -> Color32 {
    let [r, g, b, _] = color.to_array();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

#[inline]
pub fn text_alpha(c: &ThemeColors, a: u8) -> Color32 {
    with_alpha(c.text(), a)
}

#[inline]
pub fn accent_alpha(c: &ThemeColors, a: u8) -> Color32 {
    with_alpha(c.accent(), a)
}

pub fn ds_section(ui: &mut Ui, c: &ThemeColors, text: &str) {
    ui.add_space(9.0);
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD_X);
        ds_panel_title(ui, c, text.to_uppercase());
    });
    ui.add_space(3.0);
}

#[inline]
pub fn hovered_alpha(c: &ThemeColors, a: u8) -> Color32 {
    with_alpha(
        Color32::from_rgb(
            c.widget_hovered[0],
            c.widget_hovered[1],
            c.widget_hovered[2],
        ),
        a,
    )
}

#[inline]
pub fn accent_of(ui: &Ui) -> Color32 {
    ui.visuals().selection.bg_fill
}

#[inline]
pub fn warning_of(ui: &Ui) -> Color32 {
    ui.visuals().warn_fg_color
}

#[inline]
pub fn text_of(ui: &Ui) -> Color32 {
    ui.visuals()
        .override_text_color
        .unwrap_or(ui.visuals().text_color())
}

#[inline]
pub fn sep_color(c: &ThemeColors) -> Color32 {
    with_alpha(c.text(), 18)
}

pub fn ds_row<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let w = ui.available_width();
    let top_left = ui.cursor().min;
    let bg_idx = ui.painter().add(egui::Shape::Noop);

    let inner_rect = Rect::from_min_max(
        pos2(top_left.x + ROW_PAD_X, top_left.y),
        pos2(top_left.x + w - ROW_PAD_X, top_left.y + ROW_HEIGHT),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let result = add(&mut child);

    let used = child.min_rect();
    let row_h = used.height().max(ROW_HEIGHT);
    let rect = Rect::from_min_size(top_left, vec2(w, row_h));

    ui.allocate_rect(rect, Sense::hover());

    let hovered = ui.rect_contains_pointer(rect);
    let accent = accent_of(ui);
    let text = text_of(ui);

    let mut shapes: Vec<egui::Shape> = Vec::new();
    if hovered {
        shapes.push(egui::Shape::rect_filled(rect, 0.0, with_alpha(accent, 10)));
        let top = Rect::from_min_size(pos2(rect.min.x, rect.min.y), vec2(rect.width(), 2.0));
        push_gradient_line(&mut shapes, top, accent, 220);
        let bot = Rect::from_min_size(pos2(rect.min.x, rect.max.y - 2.0), vec2(rect.width(), 2.0));
        push_gradient_line(&mut shapes, bot, accent, 220);
    }
    let baseline = Rect::from_min_size(pos2(rect.min.x, rect.max.y - 1.0), vec2(rect.width(), 1.0));
    push_gradient_line(&mut shapes, baseline, text, 38);
    ui.painter().set(bg_idx, egui::Shape::Vec(shapes));

    result
}

fn push_gradient_line(out: &mut Vec<egui::Shape>, rect: Rect, color: Color32, peak_alpha: u8) {
    let steps = 80;
    let peak = peak_alpha as f32;
    for i in 0..steps {
        let t0 = i as f32 / steps as f32;
        let t1 = (i + 1) as f32 / steps as f32;
        let m = (0.5 - (t0 - 0.5).abs()) * 2.0;
        let a = (m * peak) as u8;
        let x0 = rect.min.x + t0 * rect.width();
        let x1 = rect.min.x + t1 * rect.width();
        out.push(egui::Shape::rect_filled(
            Rect::from_min_max(pos2(x0, rect.min.y), pos2(x1, rect.max.y)),
            0.0,
            with_alpha(color, a),
        ));
    }
}
pub fn ds_label(ui: &mut Ui, text: &str) {
    let col = with_alpha(text_of(ui), 204);
    ui.add_sized(
        vec2(LBL_WIDTH, ROW_HEIGHT),
        egui::Label::new(RichText::new(text).size(20.0).color(col)).selectable(false),
    );
}

pub fn ds_value_pct(ui: &mut Ui, v: f32) {
    let col = text_of(ui);
    ui.add_space(14.0);

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.add_sized(
            vec2(VAL_WIDTH, ROW_HEIGHT),
            egui::Label::new(
                RichText::new(format!("{}%", v.round() as i32))
                    .size(19.0)
                    .strong()
                    .color(col),
            )
            .selectable(false),
        );
    });
}

pub fn ds_slider(
    ui: &mut Ui,
    c: &ThemeColors,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> Response {
    let reserved = VAL_WIDTH + 14.0;
    let w = (ui.available_width() - reserved).max(40.0);
    let (rect, mut resp) = ui.allocate_exact_size(vec2(w, 18.0), Sense::click_and_drag());

    let (min, max) = (*range.start(), *range.end());
    if let Some(pos) = resp.interact_pointer_pos() {
        let t = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
        let new_val = min + t * (max - min);
        if (new_val - *value).abs() > 1e-6 {
            *value = new_val;
            resp.mark_changed();
        }
    }
    let t = ((*value - min) / (max - min).max(1e-6)).clamp(0.0, 1.0);

    let y = rect.center().y;
    let track = Rect::from_min_max(pos2(rect.min.x, y - 2.0), pos2(rect.max.x, y + 2.0));
    let fill = Rect::from_min_max(
        track.min,
        pos2(track.min.x + track.width() * t, track.max.y),
    );
    let badge = warning_of(ui);
    let p = ui.painter();
    p.rect_filled(track, 2.0, with_alpha(badge, 40));
    p.rect_filled(fill, 2.0, c.accent());

    let thumb = pos2(rect.min.x + rect.width() * t, y);
    let (r, ring_a) = if resp.dragged() {
        (9.5_f32, 230_u8)
    } else if resp.hovered() {
        (9.0, 200)
    } else {
        (8.0, 180)
    };
    p.circle_stroke(thumb, r + 0.5, Stroke::new(2.0, accent_alpha(c, ring_a)));
    p.circle_filled(thumb, r, with_alpha(c.text(), 238));
    resp
}

pub fn ds_toggle(ui: &mut Ui, c: &ThemeColors, on: &mut bool) -> Response {
    let size = vec2(46.0, 25.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }

    let p = ui.painter();
    let bg = if *on { c.accent() } else { text_alpha(c, 33) };
    p.rect_filled(rect, CornerRadius::same(13), bg);
    if *on {
        p.rect_stroke(
            rect,
            CornerRadius::same(13),
            Stroke::new(1.0, accent_alpha(c, 200)),
            egui::StrokeKind::Outside,
        );
    }

    let knob_r = 9.5_f32;
    let pad = 3.0_f32;
    let kx = if *on {
        rect.max.x - pad - knob_r
    } else {
        rect.min.x + pad + knob_r
    };
    p.circle_filled(pos2(kx, rect.center().y), knob_r, c.text());

    ui.add_space(13.0);
    let (txt, col) = if *on {
        ("Enabled", c.accent())
    } else {
        ("Disabled", text_alpha(c, 76))
    };
    ui.label(RichText::new(txt).size(20.0).color(col));
    resp
}

pub fn ds_value_text(ui: &mut Ui, text: &str) {
    let col = text_of(ui);
    ui.add_space(14.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.add_sized(
            vec2(VAL_WIDTH, ROW_HEIGHT),
            egui::Label::new(RichText::new(text).size(18.0).strong().color(col)).selectable(false),
        );
    });
}

pub fn ds_value_text_lr(ui: &mut Ui, text: &str) {
    let col = text_of(ui);
    ui.add_space(14.0);
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.add_sized(
            vec2(VAL_WIDTH, ROW_HEIGHT),
            egui::Label::new(RichText::new(text).size(18.0).strong().color(col)).selectable(false),
        );
    });
}

pub fn ds_slider_int(
    ui: &mut Ui,
    c: &ThemeColors,
    value: &mut i32,
    range: std::ops::RangeInclusive<i32>,
) -> Response {
    let (lo, hi) = (*range.start() as f32, *range.end() as f32);
    let mut f = *value as f32;
    let r = ds_slider(ui, c, &mut f, lo..=hi);
    let n = f.round() as i32;
    if n != *value {
        *value = n;
    }
    r
}

pub fn ds_pill_button(ui: &mut Ui, c: &ThemeColors, label: &str, active: bool) -> Response {
    let text_size = 14.0;
    let pad = vec2(14.0, 6.0);
    let galley = ui.painter().layout_no_wrap(
        label.into(),
        egui::FontId::proportional(text_size),
        c.text(),
    );
    let size = galley.size() + pad * 2.0;
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());

    let p = ui.painter();
    let stroke_col = if active {
        c.accent()
    } else if resp.hovered() {
        accent_alpha(c, 180)
    } else {
        accent_alpha(c, 72)
    };
    let fill = if active {
        hovered_alpha(c, 184)
    } else if resp.hovered() {
        hovered_alpha(c, 100)
    } else {
        Color32::TRANSPARENT
    };
    let text_col = if active { c.text() } else { c.text_dim() };
    p.rect_filled(rect, CornerRadius::same(3), fill);
    p.rect_stroke(
        rect,
        CornerRadius::same(3),
        Stroke::new(1.5, stroke_col),
        egui::StrokeKind::Inside,
    );
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(text_size),
        text_col,
    );
    resp
}

pub fn ds_swatch(ui: &mut Ui, color: Color32, active: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(4), color);
    if active || resp.hovered() {
        let base = text_of(ui);
        let ring = if active { base } else { with_alpha(base, 128) };
        p.rect_stroke(
            rect.expand(2.0),
            CornerRadius::same(5),
            Stroke::new(1.5, ring),
            egui::StrokeKind::Outside,
        );
    }
    resp
}

pub fn ds_panel_title(ui: &mut Ui, c: &ThemeColors, text: String) {
    ui.label(
        RichText::new(text.to_uppercase())
            .size(15.0)
            .strong()
            .color(c.accent())
            .extra_letter_spacing(2.0),
    );
}

fn paint_gradient_line_at(p: &egui::Painter, rect: Rect, color: Color32, peak_alpha: u8) {
    let steps = 80;
    let peak = peak_alpha as f32;
    for i in 0..steps {
        let t0 = i as f32 / steps as f32;
        let t1 = (i + 1) as f32 / steps as f32;
        let m = (0.5 - (t0 - 0.5).abs()) * 2.0;
        let a = (m * peak) as u8;
        let x0 = rect.min.x + t0 * rect.width();
        let x1 = rect.min.x + t1 * rect.width();
        p.rect_filled(
            Rect::from_min_max(pos2(x0, rect.min.y), pos2(x1, rect.max.y)),
            0.0,
            with_alpha(color, a),
        );
    }
}

pub fn ds_gradient_line(ui: &mut Ui) {
    let base = text_of(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 2.0), Sense::hover());
    paint_gradient_line_at(ui.painter(), rect, base, 76);
}

pub fn paint_dotgrid(ui: &Ui, rect: Rect) {
    let p = ui.painter();
    let dot = with_alpha(text_of(ui), 12);
    let step = 28.0_f32;
    let mut y = (rect.min.y / step).floor() * step;
    while y < rect.max.y {
        let mut x = (rect.min.x / step).floor() * step;
        while x < rect.max.x {
            p.circle_filled(pos2(x, y), 1.0, dot);
            x += step;
        }
        y += step;
    }
}
