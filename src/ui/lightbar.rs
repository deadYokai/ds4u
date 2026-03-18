use egui::{Button, Color32, RichText, Slider, Ui, vec2};

use crate::{app::DS4UApp, common::LightbarEffect};

impl DS4UApp {
    pub(crate) fn render_lightbar_section(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Lightbar & Indicators").size(28.0));

        ui.add_space(10.0);

        let c = self.theme.colors.clone();

        ui.label(
            RichText::new("Customize your controller lights")
                .size(14.0)
                .color(c.text_dim()),
        );

        ui.add_space(20.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Color").size(16.0).strong());

            ui.add_space(20.0);

            let mut color = [self.lightbar.r, self.lightbar.g, self.lightbar.b];

            if ui.color_edit_button_rgb(&mut color).changed() {
                self.lightbar.r = color[0];
                self.lightbar.g = color[1];
                self.lightbar.b = color[2];
                self.apply_lightbar();
                self.sync_profile();
            }

            ui.add_space(20.0);

            ui.label("Presets:");

            for (name, r, g, b) in [
                ("Aesthetic", 0.0, 0.5, 1.0),
                ("DevColor", 1.0, 0.63, 0.0),
                ("Blue", 0.0, 0.0, 1.0),
                ("Red", 1.0, 0.0, 0.0),
                ("Green", 0.0, 1.0, 0.0),
                ("Purple", 0.8, 0.0, 1.0),
                ("White", 1.0, 1.0, 1.0),
            ] {
                let color_btn = Button::new(" ")
                    .fill(Color32::from_rgb(
                        (r * 255.0) as u8,
                        (g * 255.0) as u8,
                        (b * 255.0) as u8,
                    ))
                    .min_size(vec2(32.0, 32.0));

                if ui.add(color_btn).on_hover_text(name).clicked() {
                    self.lightbar.r = r;
                    self.lightbar.g = g;
                    self.lightbar.b = b;
                    self.apply_lightbar();
                    self.sync_profile();
                }
            }
        });

        ui.add_space(20.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Brightness").size(16.0).strong());

            ui.add_space(20.0);

            if ui
                .add(
                    Slider::new(&mut self.lightbar.brightness, 0.0..=255.0)
                        .text("")
                        .show_value(false),
                )
                .changed()
            {
                self.apply_lightbar();
                self.sync_profile();
            }

            ui.label(format!(
                "{}%",
                (self.lightbar.brightness / 255.0 * 100.0) as u8
            ));
        });

        if self.ipc.is_some() {
            ui.add_space(30.0);
            ui.separator();
            ui.add_space(30.0);

            ui.label(RichText::new("LED Effects").size(16.0).strong());
            ui.add_space(4.0);
            ui.label(
                RichText::new("Animated effects (daemon only)")
                    .size(12.0)
                    .color(c.text_dim()),
            );
            ui.add_space(15.0);

            ui.horizontal(|ui| {
                let none_active = matches!(self.lightbar_effect, LightbarEffect::None);
                let breathe_active = matches!(self.lightbar_effect, LightbarEffect::Breath { .. });
                let rainbow_active = matches!(self.lightbar_effect, LightbarEffect::Rainbow { .. });
                let strobe_active = matches!(self.lightbar_effect, LightbarEffect::Strobe { .. });

                let btn = |ui: &mut egui::Ui, label: &str, active: bool, accent: egui::Color32| {
                    ui.add(
                        Button::new(RichText::new(label).size(14.0))
                            .fill(if active { accent } else { c.widget_inactive() })
                            .min_size(vec2(90.0, 36.0)),
                    )
                };

                if btn(ui, "Static", none_active, c.accent()).clicked() && !none_active {
                    self.lightbar_effect = LightbarEffect::None;
                    self.apply_lightbar_effect();
                    self.apply_lightbar();
                }
                if btn(ui, "Breathe", breathe_active, c.accent()).clicked() && !breathe_active {
                    let speed = if let LightbarEffect::Breath { speed } = self.lightbar_effect {
                        speed
                    } else {
                        0.4
                    };
                    self.lightbar_effect = LightbarEffect::Breath { speed };
                    self.apply_lightbar_effect();
                }
                if btn(ui, "Rainbow", rainbow_active, c.accent()).clicked() && !rainbow_active {
                    let speed = if let LightbarEffect::Rainbow { speed } = self.lightbar_effect {
                        speed
                    } else {
                        0.15
                    };
                    self.lightbar_effect = LightbarEffect::Rainbow { speed };
                    self.apply_lightbar_effect();
                }
                if btn(ui, "Strobe", strobe_active, c.accent()).clicked() && !strobe_active {
                    let speed = if let LightbarEffect::Strobe { speed } = self.lightbar_effect {
                        speed
                    } else {
                        4.0
                    };
                    self.lightbar_effect = LightbarEffect::Strobe { speed };
                    self.apply_lightbar_effect();
                }
            });

            ui.add_space(14.0);
            let mut effect_clone = self.lightbar_effect.clone();
            let effect_changed = match &mut effect_clone {
                LightbarEffect::None => false,

                LightbarEffect::Breath { speed } => {
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        ui.label("Speed:");
                        changed = ui
                            .add(Slider::new(speed, 0.1..=3.0).text("").show_value(false))
                            .changed();
                        let label = if *speed < 0.5 {
                            "slow"
                        } else if *speed < 1.5 {
                            "medium"
                        } else {
                            "fast"
                        };
                        ui.label(label);
                    });
                    changed
                }

                LightbarEffect::Rainbow { speed } => {
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        ui.label("Speed:");
                        changed = ui
                            .add(Slider::new(speed, 0.05..=1.0).text("").show_value(false))
                            .changed();
                        let label = if *speed < 0.2 {
                            "slow"
                        } else if *speed < 0.5 {
                            "medium"
                        } else {
                            "fast"
                        };
                        ui.label(label);
                    });
                    changed
                }

                LightbarEffect::Strobe { speed } => {
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        ui.label("Speed:");
                        changed = ui
                            .add(Slider::new(speed, 1.0..=20.0).text("").show_value(false))
                            .changed();
                        ui.label(format!("{:.0} Hz", speed));
                    });
                    changed
                }
            };

            if effect_changed {
                self.lightbar_effect = effect_clone;
                self.apply_lightbar_effect();
            }
        }

        ui.add_space(30.0);
        ui.separator();
        ui.add_space(30.0);

        ui.label(RichText::new("Player Indicator").size(16.0).strong());

        ui.add_space(15.0);

        ui.horizontal(|ui| {
            for i in 0..=7 {
                let btn = Button::new(format!("{}", i + 1))
                    .fill(if self.player_leds == i {
                        c.accent()
                    } else {
                        c.widget_inactive()
                    })
                    .min_size(vec2(48.0, 48.0));

                if ui.add(btn).clicked() {
                    self.player_leds = i;
                    self.apply_player_leds();
                    self.sync_profile();
                }
            }
        });
    }
}
