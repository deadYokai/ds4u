use egui::{ComboBox, RichText, Slider, Ui};

use crate::app::DS4UApp;
use crate::common::TriggerMode;
use crate::profiles::TriggerConfig;

const ALL_MODES: &[TriggerMode] = &[
    TriggerMode::Off,
    TriggerMode::Feedback,
    TriggerMode::Weapon,
    TriggerMode::Bow,
    TriggerMode::Galloping,
    TriggerMode::Vibration,
    TriggerMode::Machine,
];

fn mode_label(m: &TriggerMode) -> &'static str {
    match m {
        TriggerMode::Off => "Off",
        TriggerMode::Feedback => "Feedback",
        TriggerMode::Weapon => "Weapon",
        TriggerMode::Bow => "Bow",
        TriggerMode::Galloping => "Galloping",
        TriggerMode::Vibration => "Vibration",
        TriggerMode::Machine => "Machine",
    }
}

impl DS4UApp {
    fn render_trigger_panel(
        ui: &mut Ui,
        label: &str,
        side_id: &str,
        cfg: &mut TriggerConfig,
    ) -> bool {
        let mut changed = false;

        ui.label(RichText::new(label).size(16.0).strong());
        ui.add_space(8.0);

        ComboBox::from_id_salt(format!("{}_mode", side_id))
            .selected_text(mode_label(&cfg.mode))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for m in ALL_MODES {
                    if ui
                        .selectable_value(&mut cfg.mode, m.clone(), mode_label(m))
                        .changed()
                    {
                        changed = true;
                    }
                }
            });

        ui.add_space(10.0);

        match cfg.mode {
            TriggerMode::Off => {
                ui.label(RichText::new("Trigger effect disabled.").italics());
            }
            TriggerMode::Feedback => {
                ui.label("Start position (0–9)");
                if ui.add(Slider::new(&mut cfg.start, 0..=9)).changed() {
                    changed = true;
                }
                ui.label("End position (0–9)");
                if ui.add(Slider::new(&mut cfg.end, 0..=9)).changed() {
                    changed = true;
                }
                ui.label("Strength (1–8)");
                if ui.add(Slider::new(&mut cfg.strength, 1..=8)).changed() {
                    changed = true;
                }
            }
            TriggerMode::Weapon => {
                ui.label("Pre-pull resistance start (2–7)");
                if ui.add(Slider::new(&mut cfg.start, 2..=7)).changed() {
                    changed = true;
                }
                ui.label("Break point (start+1..=8)");
                if ui
                    .add(Slider::new(&mut cfg.end, (cfg.start + 1)..=8))
                    .changed()
                {
                    changed = true;
                }
                ui.label("Strength (1–8)");
                if ui.add(Slider::new(&mut cfg.strength, 1..=8)).changed() {
                    changed = true;
                }
            }
            TriggerMode::Bow => {
                ui.label("Start position (0–8)");
                if ui.add(Slider::new(&mut cfg.start, 0..=8)).changed() {
                    changed = true;
                }
                ui.label("Snap position (start+1..=8)");
                if ui
                    .add(Slider::new(&mut cfg.end, (cfg.start + 1)..=8))
                    .changed()
                {
                    changed = true;
                }
                ui.label("Force / snap strength (1–8)");
                if ui.add(Slider::new(&mut cfg.strength, 1..=8)).changed() {
                    changed = true;
                }
            }
            TriggerMode::Galloping => {
                ui.label("Start position (0–8)");
                if ui.add(Slider::new(&mut cfg.start, 0..=8)).changed() {
                    changed = true;
                }
                ui.label("End position (start+1..=9)");
                if ui
                    .add(Slider::new(&mut cfg.end, (cfg.start + 1)..=9))
                    .changed()
                {
                    changed = true;
                }
                ui.label("Foot offset (1–8)");
                if ui.add(Slider::new(&mut cfg.strength, 1..=8)).changed() {
                    changed = true;
                }
                ui.label("Frequency (Hz)");
                if ui.add(Slider::new(&mut cfg.frequency, 1..=255)).changed() {
                    changed = true;
                }
            }
            TriggerMode::Vibration => {
                ui.label("Start position (0–9)");
                if ui.add(Slider::new(&mut cfg.start, 0..=9)).changed() {
                    changed = true;
                }
                ui.label("End position");
                if ui.add(Slider::new(&mut cfg.end, cfg.start..=9)).changed() {
                    changed = true;
                }
                ui.label("Amplitude (1–8)");
                if ui.add(Slider::new(&mut cfg.strength, 1..=8)).changed() {
                    changed = true;
                }
                ui.label("Frequency (Hz)");
                if ui.add(Slider::new(&mut cfg.frequency, 1..=255)).changed() {
                    changed = true;
                }
            }
            TriggerMode::Machine => {
                ui.label("Start position (0–8)");
                if ui.add(Slider::new(&mut cfg.start, 0..=8)).changed() {
                    changed = true;
                }
                ui.label("End position (start+1..=9)");
                if ui
                    .add(Slider::new(&mut cfg.end, (cfg.start + 1)..=9))
                    .changed()
                {
                    changed = true;
                }
                ui.label("Amplitude (1–8)");
                if ui.add(Slider::new(&mut cfg.strength, 1..=8)).changed() {
                    changed = true;
                }
                ui.label("Frequency (Hz)");
                if ui.add(Slider::new(&mut cfg.frequency, 1..=255)).changed() {
                    changed = true;
                }
            }
        }

        ui.add_space(14.0);
        ui.label(RichText::new("Deadband").size(14.0).strong());
        ui.label("Release (raw 0–255)");
        if ui
            .add(Slider::new(&mut cfg.deadband.release, 0..=254))
            .changed()
        {
            changed = true;
        }
        ui.label("Full stroke (raw 0–255)");
        if ui
            .add(Slider::new(
                &mut cfg.deadband.full_stroke,
                (cfg.deadband.release + 1)..=255,
            ))
            .changed()
        {
            changed = true;
        }

        changed
    }

    pub(crate) fn render_triggers_section(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Adaptive Triggers").size(28.0));
        ui.add_space(10.0);

        let c = self.theme.colors.clone();
        ui.label(
            RichText::new("Configure trigger resistance and feedback")
                .size(14.0)
                .color(c.text_dim()),
        );

        ui.add_space(20.0);

        let mut left_changed = false;
        let mut right_changed = false;

        let (mut l, mut r) = (self.triggers.left.clone(), self.triggers.right.clone());

        ui.columns(2, |cols| {
            left_changed =
                Self::render_trigger_panel(&mut cols[0], "L2 (Left Trigger)", "left", &mut l);
            right_changed =
                Self::render_trigger_panel(&mut cols[1], "R2 (Right Trigger)", "right", &mut r);
        });

        self.triggers.left = l;
        self.triggers.right = r;

        if left_changed || right_changed {
            self.apply_triggers();
            self.apply_input_transform();
            self.sync_profile();
        }

        ui.add_space(20.0);
        if ui.button("Reset both triggers to Off").clicked() {
            self.triggers.left = TriggerConfig::default();
            self.triggers.right = TriggerConfig::default();
            self.apply_triggers();
            self.apply_input_transform();
            self.sync_profile();
        }
    }
}
