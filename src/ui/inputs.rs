use std::f32::consts::FRAC_PI_2;

use egui::epaint::{PathShape, PathStroke};
use egui::{include_image, pos2, vec2, Align2, Color32, CornerRadius, FontId, Image, Painter, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::app::DS4UApp;
use crate::inputs::*;

include!(concat!(env!("OUT_DIR"), "/svg_coords.rs"));

const SVG_VIEWPORT: f32 = 128.0;

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
        let rx           = state.map_or(0x80u8, |s| s.right_x);
        let ry           = state.map_or(0x80u8, |s| s.right_y);
        let touch_count  = state.map_or(0u8,  |s| s.touch_count);
        let touch_pts    = state.map(|s| &s.touch_points);

        let l3 = buttons & BTN_L3 != 0;
        let r3 = buttons & BTN_R3 != 0;

        let stick_colors = [
            Color32::from_gray(60),
            Color32::from_rgb(0, 200, 255),
            Color32::WHITE,
        ];

        let side = 512.0; 
        let desired_size = vec2(side, side);
        let (response, painter) = ui.allocate_painter(desired_size, Sense::hover());
        let canvas = response.rect;

        Image::new(include_image!("../../assets/controller_body.svg"))
            .maintain_aspect_ratio(true)
            .paint_at(ui, canvas);

        let scale = canvas.width() / SVG_VIEWPORT;

        let map = |x: f32, y: f32| -> Pos2 {
            pos2(
                canvas.min.x + (x / SVG_VIEWPORT) * canvas.width(),
                canvas.min.y + (y / SVG_VIEWPORT) * canvas.width(),)
        };

        let stick_r = 7.0 * scale;

        Self::render_live_stick(
            &painter,
            map(SVG_STICK_L.0, SVG_STICK_L.1),
            stick_r, [lx, ly], l3,
            self.sticks.left_deadzone, stick_colors,
        );
        Self::render_live_stick(
            &painter,
            map(SVG_STICK_R.0, SVG_STICK_R.1),
            stick_r, [rx, ry], r3,
            self.sticks.right_deadzone, stick_colors,
        );


        let shoulder_r = 4.0 * scale;
        Self::render_button(&painter, map(SVG_L1.0, SVG_L1.1), shoulder_r, buttons & BTN_L1 != 0);
        Self::render_button(&painter, map(SVG_R1.0, SVG_R1.1), shoulder_r, buttons & BTN_R1 != 0);

        let ps_r = 3.0 * scale;
        Self::render_button(&painter,
            map(SVG_PS_BTN.0, SVG_PS_BTN.1), ps_r, buttons & BTN_PS != 0);

        Self::render_button(&painter, map(SVG_MIC_BTN.0, SVG_MIC_BTN.1), 2.5 * scale, buttons & BTN_MUTE != 0);


        let trig_sz = vec2(6.0 * scale, 14.0 * scale);
        Self::render_trigger_bar(&painter, map(SVG_L2.0, SVG_L2.1), trig_sz, l2_raw, buttons & BTN_L2 != 0);
        Self::render_trigger_bar(&painter, map(SVG_R2.0, SVG_R2.1), trig_sz, r2_raw, buttons & BTN_R2 != 0);



        let meta_r = 2.5 * scale;
        Self::render_button(&painter, map(SVG_CREATE_BTN.0,  SVG_CREATE_BTN.1),  meta_r, buttons & BTN_CREATE  != 0);
        Self::render_button(&painter, map(SVG_OPTIONS_BTN.0, SVG_OPTIONS_BTN.1), meta_r, buttons & BTN_OPTIONS != 0);



        let fb_r = 3.0 * scale;
        Self::render_button(&painter,
            map(SVG_SQUARE.0, SVG_SQUARE.1), fb_r, buttons & BTN_SQUARE != 0);
        Self::render_button(&painter,
            map(SVG_CROSS.0, SVG_CROSS.1), fb_r, buttons & BTN_CROSS != 0);
        Self::render_button(&painter,
            map(SVG_CIRCLE.0, SVG_CIRCLE.1), fb_r, buttons & BTN_CIRCLE != 0);
        Self::render_button(&painter,
            map(SVG_TRIANGLE.0, SVG_TRIANGLE.1), fb_r, buttons & BTN_TRIANGLE != 0);

        let dp_size = 4.0 * scale;
        Self::render_dpad_button(
            &painter,
            map(SVG_DPAD_T.0, SVG_DPAD_T.1),
            dp_size,
            &[DPAD_N, DPAD_NE, DPAD_NW],
            dpad,
            2
        );
        Self::render_dpad_button(
            &painter,
            map(SVG_DPAD_B.0, SVG_DPAD_B.1),
            dp_size,
            &[DPAD_S, DPAD_SE, DPAD_SW],
            dpad,
            0
        );
        Self::render_dpad_button(
            &painter,
            map(SVG_DPAD_L.0, SVG_DPAD_L.1),
            dp_size,
            &[DPAD_W, DPAD_NW, DPAD_SW],
            dpad,
            1
        );
        Self::render_dpad_button(
            &painter,
            map(SVG_DPAD_R.0, SVG_DPAD_R.1),
            dp_size,
            &[DPAD_E, DPAD_NE, DPAD_SE],
            dpad,
            3
        );

        {
            let tp_w = 46.0;
            let tp_h = 26.0;
            let tp_center = map(SVG_TOUCHPAD.0, SVG_TOUCHPAD.1);
            let tp_rect   = Rect::from_center_size(tp_center, vec2(tp_w * scale, tp_h * scale));
            let rounding  = CornerRadius::same(3);

            if buttons & BTN_TOUCHPAD != 0 {
                painter.rect_filled(
                    tp_rect, rounding,
                    Color32::from_rgba_unmultiplied(90, 160, 255, 50),
                );
                painter.rect_stroke(
                    tp_rect, rounding,
                    Stroke::new(1.5, Color32::from_rgb(90, 160, 255)), StrokeKind::Outside
                );
            }

            if let Some(pts) = touch_pts {
                for pt in pts.iter().filter(|p| p.active) {
                    let svgx = SVG_TOUCHPAD.0 - tp_w * 0.5
                        + (pt.x as f32 / TOUCHPAD_MAX_X as f32) * tp_w;
                    let svgy = SVG_TOUCHPAD.1 - tp_h * 0.5
                        + (pt.y as f32 / TOUCHPAD_MAX_Y as f32) * tp_h;
                    let dot = map(svgx, svgy);
                    painter.circle_filled(dot, 2.5 * scale, Color32::from_rgb(0, 200, 255));
                    painter.circle_stroke(dot, 2.5 * scale, Stroke::new(0.8, Color32::WHITE));
                }
            }
        }
    }

    fn cubic_bezier(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, steps: usize) -> Vec<Pos2> {
        let mut pts = Vec::with_capacity(steps + 1);

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let u = 1.0 - t;

            let x =
                u*u*u*p0.x +
                3.0*u*u*t*p1.x +
                3.0*u*t*t*p2.x +
                t*t*t*p3.x;

            let y =
                u*u*u*p0.y +
                3.0*u*u*t*p1.y +
                3.0*u*t*t*p2.y +
                t*t*t*p3.y;

            pts.push(Pos2::new(x, y));
        }

        pts
    }

    fn render_dpad_button(
        p: &Painter, center: Pos2,
        radius: f32,
        dpad: &[u8; 3],
        dpad_active: u8,
        rotation: u8
    ) {
        let mut points = Vec::new();

        let w = radius;
        let h = radius;

        let cx = center.x;
        let cy = center.y;

        let left  = cx - w * 0.5;
        let right = cx + w * 0.5;
        let top   = cy - h * 0.5;
        let bot   = cy + h * 0.5;

        let tip = Pos2::new(cx, top - h * 0.6);

        let p0 = Pos2::new(left, top);
        let p1 = Pos2::new(cx, tip.y);
        let p2 = Pos2::new(right, top);

        points.push(p0);

        points.extend(Self::cubic_bezier(
                p0,
                Pos2::new(left, top - h * 0.2),
                Pos2::new(cx - w * 0.2, tip.y),
                p1,
                10,
        ));

        points.extend(Self::cubic_bezier(
                p1,
                Pos2::new(cx + w * 0.2, tip.y),
                Pos2::new(right, top - h * 0.2),
                p2,
                10,
        ));

        points.push(Pos2::new(right, bot));
        points.push(Pos2::new(left, bot));

        let rot = rotation % 4;

        for p in &mut points {
            let dx = p.x - center.x;
            let dy = p.y - center.y;

            let (rx, ry) = match rot {
                0 => (dx, dy),
                1 => (-dy, dx),
                2 => (-dx, -dy),
                3 => (dy, -dx),
                _ => unreachable!(),
            };

            p.x = center.x + rx;
            p.y = center.y + ry;
        }

        let s = PathShape {
            points,
            closed: true,
            fill: if dpad.contains(&dpad_active) { Color32::from_rgb(90, 160, 255) } 
            else { Color32::TRANSPARENT },
            stroke: PathStroke::new(1.0, Color32::WHITE),
        };

        p.add(s);
    }

    fn render_trigger_bar(
        p: &Painter,
        anchor: Pos2,
        size: Vec2,
        analog: u8,
        digital: bool,
    ) {
        let rect = Rect::from_min_size(pos2(anchor.x - size.x * 0.5, anchor.y), size);
        let rounding = CornerRadius::same(2);

        p.rect_filled(rect, rounding, Color32::from_gray(35));

        let fill_h = (analog as f32 / 255.0) * size.y;
        if fill_h > 0.5 {
            let fill_rect = Rect::from_min_size(
                pos2(rect.min.x, rect.max.y - fill_h),
                vec2(size.x, fill_h),
            );
            p.rect_filled(
                fill_rect, rounding,
                if digital { Color32::from_rgb(90, 160, 255) } else { Color32::from_rgb(50, 90, 160) },
            );
        }

        p.rect_stroke(rect, rounding, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Outside);

    }

    fn render_button(
        p: &Painter,
        center: Pos2,
        radius: f32,
        pressed: bool,
    ) {
        let fill = if pressed {
            Color32::from_rgb(90, 160, 255)
        } else {
            Color32::TRANSPARENT
        };

        p.circle_filled(center, radius, fill);
        p.circle_stroke(center, radius, Stroke::new(1.5, Color32::WHITE));
    }

    fn render_live_stick(
        p: &Painter,
        center: Pos2,
        radius: f32,
        raw: [u8; 2],
        pressed: bool,
        deadzone: f32,
        colors: [Color32; 3]
    ) {
        p.circle_filled(center, radius, if pressed {colors[1]} else {Color32::TRANSPARENT});
        p.circle_stroke(center, radius,
            egui::Stroke::new(1.0, Color32::WHITE));

        if deadzone > 0.0 {
            p.circle_filled(center, radius * deadzone.clamp(0.0, 1.0),
                Color32::from_rgba_unmultiplied(200, 50, 50, 60));
        }

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
