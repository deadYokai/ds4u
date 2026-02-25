use egui::{Align2, Color32, CornerRadius, Painter, Pos2, RichText, Ui, pos2, vec2};

use crate::app::DS4UApp;
use crate::inputs::*;

impl DS4UApp {   
    pub(crate) fn render_inputs_section(&self, ui: &mut Ui) {
        ui.heading(RichText::new("Controller Inputs").size(28.0));

        ui.add_space(10.0);

        ui.label(RichText::new("Live visualisation")
            .size(14.0)
            .color(Color32::GRAY));

        ui.add_space(30.0);

        let state        = self.controller_state.as_ref();
        let buttons      = state.map_or(0,     |s| s.buttons);
        let dpad         = state.map_or(DPAD_NEUTRAL, |s| s.dpad);
        let l2_raw       = state.map_or(0u8,  |s| s.l2);
        let r2_raw       = state.map_or(0u8,  |s| s.r2);
        let lx           = state.map_or(0x80u8, |s| s.left_x);
        let ly           = state.map_or(0x80u8, |s| s.left_y);
        let rx_ax        = state.map_or(0x80u8, |s| s.right_x);
        let ry_ax        = state.map_or(0x80u8, |s| s.right_y);
        let touch_count  = state.map_or(0u8,  |s| s.touch_count);
        let touch_pts    = state.map(|s| &s.touch_points);

        let canvas_w = 700.0;
        let canvas_h = 360.0;

        let (canvas, _) = ui.allocate_exact_size(
            vec2(canvas_w, canvas_h),
            egui::Sense::hover(),
        );

        let p = ui.painter_at(canvas);
        let o = canvas.min;

        let px = |x: f32| o.x + x;
        let py = |y: f32| o.y + y;
        let pt = |x: f32, y: f32| pos2(o.x + x, o.y + y);

        let col_body       = Color32::from_rgb(28, 38, 58);
        let col_body_edge  = Color32::from_rgb(48, 65, 95);
        let col_btn_off    = Color32::from_rgb(38, 52, 78);
        let col_btn_edge   = Color32::from_rgb(55, 75, 110);
        let col_label      = Color32::from_rgb(140, 155, 180);
        let col_accent     = Color32::from_rgb(0, 122, 250);

        let col_triangle   = Color32::from_rgb(0,   180, 140);
        let col_circle     = Color32::from_rgb(210,  55,  55);
        let col_cross      = Color32::from_rgb(80,  140, 220);
        let col_square     = Color32::from_rgb(190,  80, 180);

        let col_dpad_active = Color32::from_rgb(200, 210, 230);
        let col_shoulder_active = col_accent;
        let col_system_active   = col_accent;

        p.rect_filled(
            egui::Rect::from_min_max(pt(60.0, 40.0), pt(640.0, 255.0)),
            CornerRadius::same(56),
            col_body,
        );
        p.rect_stroke(
            egui::Rect::from_min_max(pt(60.0, 40.0), pt(640.0, 255.0)),
            CornerRadius::same(56),
            egui::Stroke::new(1.5, col_body_edge),
            egui::StrokeKind::Outside,
        );

        p.rect_filled(
            egui::Rect::from_min_max(pt(82.0, 195.0), pt(218.0, 345.0)),
            CornerRadius { nw: 8, ne: 8, sw: 50, se: 50 },
            col_body,
        );
        p.rect_stroke(
            egui::Rect::from_min_max(pt(82.0, 195.0), pt(218.0, 345.0)),
            CornerRadius { nw: 8, ne: 8, sw: 50, se: 50 },
            egui::Stroke::new(1.5, col_body_edge),
            egui::StrokeKind::Outside,
        );

        p.rect_filled(
            egui::Rect::from_min_max(pt(482.0, 195.0), pt(618.0, 345.0)),
            CornerRadius { nw: 8, ne: 8, sw: 50, se: 50 },
            col_body,
        );
        p.rect_stroke(
            egui::Rect::from_min_max(pt(482.0, 195.0), pt(618.0, 345.0)),
            CornerRadius { nw: 8, ne: 8, sw: 50, se: 50 },
            egui::Stroke::new(1.5, col_body_edge),
            egui::StrokeKind::Outside,
        );

        let l2_rect = egui::Rect::from_min_max(pt(62.0, 12.0), pt(202.0, 38.0));
        let r2_rect = egui::Rect::from_min_max(pt(498.0, 12.0), pt(638.0, 38.0));

        for rect in [l2_rect, r2_rect] {
            p.rect_filled(rect, CornerRadius::same(5), Color32::from_rgb(18, 26, 42));
            p.rect_stroke(rect, CornerRadius::same(5),
            egui::Stroke::new(1.0, col_body_edge),
            egui::StrokeKind::Outside);
        }

        let l2_fill_w = l2_rect.width() * (l2_raw as f32 / 255.0);
        if l2_fill_w > 0.0 {
            let fill = l2_rect.with_max_x(l2_rect.min.x + l2_fill_w);
            p.rect_filled(fill, CornerRadius::same(5), col_accent);
        }

        let r2_fill_w = r2_rect.width() * (r2_raw as f32 / 255.0);
        if r2_fill_w > 0.0 {
            let fill = r2_rect.with_min_x(r2_rect.max.x - r2_fill_w);
            p.rect_filled(fill, CornerRadius::same(5), col_accent);
        }

        p.text(pt(132.0, 25.0), Align2::CENTER_CENTER, "L2",
        egui::FontId::proportional(11.0), col_label);
        p.text(pt(568.0, 25.0), Align2::CENTER_CENTER, "R2",
        egui::FontId::proportional(11.0), col_label);

        let l1_pressed = buttons & BTN_L1 != 0;
        let r1_pressed = buttons & BTN_R1 != 0;

        let l1_rect = egui::Rect::from_min_max(pt(65.0, 40.0), pt(200.0, 62.0));
        let r1_rect = egui::Rect::from_min_max(pt(500.0, 40.0), pt(635.0, 62.0));

        p.rect_filled(l1_rect, CornerRadius { nw: 4, ne: 4, sw: 4, se: 4 },
            if l1_pressed { col_shoulder_active } else { col_btn_off });
        p.rect_filled(r1_rect, CornerRadius::same(4),
        if r1_pressed { col_shoulder_active } else { col_btn_off });

        p.text(pt(132.0, 51.0), Align2::CENTER_CENTER, "L1",
        egui::FontId::proportional(11.0), col_label);
        p.text(pt(568.0, 51.0), Align2::CENTER_CENTER, "R1",
        egui::FontId::proportional(11.0), col_label);

        let dc = pt(192.0, 152.0);
        let arm_w = 22.0;
        let arm_h = 26.0;
        let cr = CornerRadius::same(3);

        let dpad_rects = [
            (egui::Rect::from_center_size(
                    pos2(dc.x,            dc.y - arm_h),
                    vec2(arm_w, arm_h)), [DPAD_N, DPAD_NE, DPAD_NW], "▲"),
                    (egui::Rect::from_center_size(
                            pos2(dc.x,            dc.y + arm_h),
                            vec2(arm_w, arm_h)), [DPAD_S, DPAD_SE, DPAD_SW], "▼"),
                            (egui::Rect::from_center_size(
                                    pos2(dc.x - arm_h,   dc.y),
                                    vec2(arm_h, arm_w)), [DPAD_W, DPAD_NW, DPAD_SW], "◄"),
                                    (egui::Rect::from_center_size(
                                            pos2(dc.x + arm_h,   dc.y),
                                            vec2(arm_h, arm_w)), [DPAD_E, DPAD_NE, DPAD_SE], "►"),
        ];

        p.rect_filled(
            egui::Rect::from_center_size(dc, vec2(arm_w, arm_w)),
            CornerRadius::same(3),
            col_btn_off,
        );

        for (rect, dirs, label) in &dpad_rects {
            let active = dirs.contains(&dpad);
            p.rect_filled(*rect, cr, if active { col_dpad_active } else { col_btn_off });
            p.rect_stroke(*rect, cr,
                egui::Stroke::new(1.0, col_btn_edge), egui::StrokeKind::Outside);
            p.text(rect.center(), Align2::CENTER_CENTER, *label,
            egui::FontId::proportional(10.0),
            if active { Color32::from_rgb(20, 30, 50) } else { col_label });
        }  

        let fc    = pt(500.0, 152.0);
        let fb_r  = 16.0;
        let fb_d  = 34.0;

        struct FaceBtn {
            cx: f32, cy: f32,
            mask: u32,
            active_col: Color32,
            label: &'static str,
        }
        let face_btns = [
            FaceBtn { cx: fc.x,        cy: fc.y - fb_d, mask: BTN_TRIANGLE,
            active_col: col_triangle, label: "△" },
            FaceBtn { cx: fc.x + fb_d, cy: fc.y,        mask: BTN_CIRCLE,
            active_col: col_circle,   label: "○" },
            FaceBtn { cx: fc.x,        cy: fc.y + fb_d, mask: BTN_CROSS,
            active_col: col_cross,    label: "✕" },
            FaceBtn { cx: fc.x - fb_d, cy: fc.y,        mask: BTN_SQUARE,
            active_col: col_square,   label: "□" },
        ];

        for btn in &face_btns {
            let centre = pos2(px(btn.cx - o.x), py(btn.cy - o.y));
            let active = buttons & btn.mask != 0;
            p.circle_filled(centre, fb_r,
                if active { btn.active_col }
                else      { col_btn_off   });
            p.circle_stroke(centre, fb_r,
                egui::Stroke::new(1.0, col_btn_edge));
            p.text(centre, Align2::CENTER_CENTER, btn.label,
                egui::FontId::proportional(13.0),
                if active { Color32::WHITE } else { col_label });
        }

        let tp_rect = egui::Rect::from_min_max(pt(268.0, 74.0), pt(432.0, 182.0));
        let tp_pressed = buttons & BTN_TOUCHPAD != 0;

        p.rect_filled(tp_rect, CornerRadius::same(10),
        if tp_pressed { Color32::from_rgb(45, 65, 100) } else { Color32::from_rgb(22, 32, 50) });
        p.rect_stroke(tp_rect, CornerRadius::same(10),
        egui::Stroke::new(if tp_pressed { 1.5 } else { 1.0 },
            if tp_pressed { col_accent } else { col_body_edge }),
            egui::StrokeKind::Outside);

        if let Some(pts) = touch_pts {
            for tp in pts.iter().filter(|t| t.active) {
                let tx = tp_rect.min.x + (tp.x as f32 / TOUCHPAD_MAX_X as f32) * tp_rect.width();
                let ty = tp_rect.min.y + (tp.y as f32 / TOUCHPAD_MAX_Y as f32) * tp_rect.height();
                p.circle_filled(pos2(tx, ty), 7.0, col_accent);
                p.circle_stroke(pos2(tx, ty), 7.0,
                egui::Stroke::new(1.0, Color32::WHITE));
            }
        }

        if touch_count == 0 {
            p.text(tp_rect.center(), Align2::CENTER_CENTER, "TOUCHPAD",
            egui::FontId::proportional(10.0), col_label);
        }

        let create_pressed = buttons & BTN_CREATE != 0;
        let create_rect = egui::Rect::from_min_max(pt(236.0, 130.0), pt(264.0, 148.0));
        p.rect_filled(create_rect, CornerRadius::same(5),
        if create_pressed { col_system_active } else { col_btn_off });
        p.rect_stroke(create_rect, CornerRadius::same(5),
        egui::Stroke::new(1.0, col_btn_edge), egui::StrokeKind::Outside);
        p.text(create_rect.center(), Align2::CENTER_CENTER, "≡+",
        egui::FontId::proportional(9.0), col_label);

        let options_pressed = buttons & BTN_OPTIONS != 0;
        let opts_rect = egui::Rect::from_min_max(pt(436.0, 130.0), pt(464.0, 148.0));
        p.rect_filled(opts_rect, CornerRadius::same(5),
        if options_pressed { col_system_active } else { col_btn_off });
        p.rect_stroke(opts_rect, CornerRadius::same(5),
        egui::Stroke::new(1.0, col_btn_edge), egui::StrokeKind::Outside);
        p.text(opts_rect.center(), Align2::CENTER_CENTER, "≡",
        egui::FontId::proportional(9.0), col_label);

        let mute_pressed = buttons & BTN_MUTE != 0;
        let mute_c = pt(350.0, 66.0);
        p.circle_filled(mute_c, 10.0,
            if mute_pressed { col_system_active } else { col_btn_off });
        p.circle_stroke(mute_c, 10.0, egui::Stroke::new(1.0, col_btn_edge));
        p.text(mute_c, Align2::CENTER_CENTER, "🔇",
            egui::FontId::proportional(8.0), col_label);

        let ps_pressed = buttons & BTN_PS != 0;
        let ps_c = pt(350.0, 210.0);
        let ps_col = if ps_pressed {
            Color32::from_rgb(255, 255, 255)
        } else {
            col_btn_off
        };
        p.circle_filled(ps_c, 16.0, ps_col);
        p.circle_stroke(ps_c, 16.0, egui::Stroke::new(1.5, col_btn_edge));
        p.text(ps_c, Align2::CENTER_CENTER, "PS",
            egui::FontId::proportional(9.0),
            if ps_pressed { Color32::from_rgb(20, 30, 50) } else { col_label });

        Self::render_live_stick(
            &p, pt(150.0, 270.0), 42.0,
            [lx, ly], buttons & BTN_L3 != 0, [col_accent, col_btn_off, col_btn_edge],
        );

        Self::render_live_stick(
            &p, pt(440.0, 270.0), 42.0,
            [rx_ax, ry_ax], buttons & BTN_R3 != 0, [col_accent, col_btn_off, col_btn_edge],
        );

        p.text(pt(150.0, 320.0), Align2::CENTER_CENTER, "L3",
        egui::FontId::proportional(10.0), col_label);
        p.text(pt(440.0, 320.0), Align2::CENTER_CENTER, "R3",
        egui::FontId::proportional(10.0), col_label);

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!(
                        "L2 {:3}   R2 {:3}   LX {:3}  LY {:3}   RX {:3}  RY {:3}   Touches {}",
                        l2_raw, r2_raw, lx, ly, rx_ax, ry_ax, touch_count
            )).size(12.0).color(Color32::from_gray(120)).monospace());
        });
    }

    fn render_live_stick(
        p: &Painter,
        center: Pos2,
        radius: f32,
        raw: [u8; 2],
        pressed: bool,
        colors: [Color32; 3]
    ) {
        p.circle_filled(center, radius, colors[1]);
        p.circle_stroke(center, radius,
            egui::Stroke::new(if pressed { 2.5 } else { 1.5 },
                if pressed { colors[0] } else { colors[2] }));

        p.circle_stroke(center, radius * 0.55,
            egui::Stroke::new(0.5, Color32::from_rgb(40, 55, 80)));

        let nx = (raw[0] as f32 - 128.0) / 128.0;
        let ny = (raw[1] as f32 - 128.0) / 128.0;
        let dot = pos2(
            center.x + nx * (radius - 10.0),
            center.y + ny * (radius - 10.0),
        );
        p.circle_filled(dot, 8.0, colors[0]);
        p.circle_stroke(dot, 8.0, egui::Stroke::new(1.0, Color32::WHITE));
    }

}
