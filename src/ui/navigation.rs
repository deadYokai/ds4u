use std::time::{Duration, Instant};

use egui::{Id, Rect, Vec2};

use crate::app::DS4UApp;
use crate::inputs::{
    BTN_CROSS, BTN_L1, BTN_R1, DPAD_E, DPAD_N, DPAD_NE, DPAD_NW, DPAD_S, DPAD_SE, DPAD_SW, DPAD_W,
};
use crate::state::Section;

const REPEAT_INITIAL: Duration = Duration::from_millis(350);
const REPEAT_INTERVAL: Duration = Duration::from_millis(130);

pub(crate) fn nav_focus_id() -> Id {
    Id::new("ds4u::nav_focus")
}

#[derive(Clone, Default)]
pub(crate) struct NavFocus {
    pub(crate) enabled: bool,

    pub(crate) index: usize,

    pub(crate) count: usize,

    pub(crate) counter: usize,

    pub(crate) activate: bool,

    pub(crate) adjust: i32,

    pub(crate) rects: Vec<Rect>,

    pub(crate) last_section: Option<Section>,
}

pub(crate) struct Navigator {
    prev_buttons: u32,
    v_dir: i32,
    v_next: Instant,
    h_dir: i32,
    h_next: Instant,
}

impl Navigator {
    pub(crate) fn new() -> Self {
        let now = Instant::now();
        Self {
            prev_buttons: 0,
            v_dir: 0,
            v_next: now,
            h_dir: 0,
            h_next: now,
        }
    }

    fn reset_held(&mut self) {
        self.v_dir = 0;
        self.h_dir = 0;
    }
}

#[inline]
fn dpad_up(d: u8) -> bool {
    matches!(d, DPAD_N | DPAD_NE | DPAD_NW)
}
#[inline]
fn dpad_down(d: u8) -> bool {
    matches!(d, DPAD_S | DPAD_SE | DPAD_SW)
}
#[inline]
fn dpad_left(d: u8) -> bool {
    matches!(d, DPAD_W | DPAD_NW | DPAD_SW)
}
#[inline]
fn dpad_right(d: u8) -> bool {
    matches!(d, DPAD_E | DPAD_NE | DPAD_SE)
}

fn pulse(now: Instant, dir: i32, held: &mut i32, next: &mut Instant) -> i32 {
    if dir != *held {
        *held = dir;
        if dir != 0 {
            *next = now + REPEAT_INITIAL;
            return dir;
        }
        return 0;
    }
    if dir != 0 && now >= *next {
        *next = now + REPEAT_INTERVAL;
        return dir;
    }
    0
}

impl DS4UApp {
    pub(crate) fn handle_controller_nav(&mut self, ctx: &egui::Context) {
        let (buttons, dpad) = match self.input.controller_state.as_ref() {
            Some(s) => (s.buttons, s.dpad),
            None => {
                self.nav.reset_held();
                self.nav.prev_buttons = 0;
                return;
            }
        };

        let prev = self.nav.prev_buttons;
        let now = Instant::now();
        let edge = |m: u32| buttons & m != 0 && prev & m == 0;

        let v = if dpad_up(dpad) {
            -1
        } else if dpad_down(dpad) {
            1
        } else {
            0
        };
        let h = if dpad_left(dpad) {
            -1
        } else if dpad_right(dpad) {
            1
        } else {
            0
        };

        let move_focus = pulse(now, v, &mut self.nav.v_dir, &mut self.nav.v_next);
        let adjust = pulse(now, h, &mut self.nav.h_dir, &mut self.nav.h_next);

        let mut section_dir = 0;
        if edge(BTN_L1) {
            section_dir = -1;
        }
        if edge(BTN_R1) {
            section_dir = 1;
        }
        let activate = edge(BTN_CROSS);

        let any = move_focus != 0 || adjust != 0 || activate || section_dir != 0;

        if section_dir != 0 {
            self.cycle_section(section_dir);
        }
        if any {
            ctx.request_repaint();
        }

        ctx.data_mut(|d| {
            let f = d.get_temp_mut_or_default::<NavFocus>(nav_focus_id());
            if any {
                f.enabled = true;
            }
            if section_dir != 0 {
                f.index = 0;
            } else if move_focus != 0 && f.count > 0 {
                let max = (f.count - 1) as i64;
                let cur = f.index.min(f.count - 1) as i64;
                f.index = (cur + move_focus as i64).clamp(0, max) as usize;
            }
            if activate {
                f.activate = true;
            }
            f.adjust += adjust;
        });

        self.nav.prev_buttons = buttons;
    }

    pub(crate) fn focus_begin_frame(&self, ctx: &egui::Context) {
        let section = self.active_section;
        let mouse_moved = ctx.input(|i| i.pointer.delta() != Vec2::ZERO);
        ctx.data_mut(|d| {
            let f = d.get_temp_mut_or_default::<NavFocus>(nav_focus_id());
            f.counter = 0;
            f.rects.clear();
            if f.last_section != Some(section) {
                f.last_section = Some(section);
                f.index = 0;
            }
            if mouse_moved {
                f.enabled = false;
            }
        });
    }

    pub(crate) fn focus_end_frame(&self, ctx: &egui::Context) {
        ctx.data_mut(|d| {
            let f = d.get_temp_mut_or_default::<NavFocus>(nav_focus_id());
            f.count = f.counter;
            if f.count == 0 {
                f.index = 0;
            } else if f.index >= f.count {
                f.index = f.count - 1;
            }
            f.activate = false;
            f.adjust = 0;
        });
    }
}
