//! Shared right-click menu. UI widgets enqueue edits; the app applies and saves them once.

use eframe::egui::{self, Context, Response, Ui};

use super::{Config, Kind, ValueRange, kind, label};

fn editing_allowed(ui: &Ui) -> bool {
    !ui.input(|i| {
        i.pointer.secondary_down() || i.pointer.button_released(egui::PointerButton::Secondary)
    })
}

/// egui's native slider accepts any pointer button. Reserve secondary input for MIDI menus.
pub(crate) fn slider<'a, N: egui::emath::Numeric>(
    ui: &Ui,
    value: &'a mut N,
    range: std::ops::RangeInclusive<N>,
) -> egui::Slider<'a> {
    let allow = editing_allowed(ui);
    let slider =
        egui::Slider::from_get_set(range.start().to_f64()..=range.end().to_f64(), move |new| {
            if allow && let Some(new) = new {
                *value = N::from_f64(new);
            }
            value.to_f64()
        });
    if N::INTEGRAL {
        slider.integer()
    } else {
        slider
    }
}

pub(crate) fn drag_value<'a, N: egui::emath::Numeric>(
    ui: &Ui,
    value: &'a mut N,
) -> egui::DragValue<'a> {
    let allow = editing_allowed(ui);
    let drag = egui::DragValue::from_get_set(move |new| {
        if allow && let Some(new) = new {
            *value = N::from_f64(new);
        }
        value.to_f64()
    });
    if N::INTEGRAL {
        drag.max_decimals(0).range(N::MIN..=N::MAX).speed(0.25)
    } else {
        drag
    }
}

#[derive(Clone, Debug)]
pub(super) enum Action {
    Learn(String),
    Remove(String),
    Range(String, ValueRange),
}

fn key() -> egui::Id {
    egui::Id::unique("midi-context-actions")
}

pub(super) fn take(ctx: &Context) -> Vec<Action> {
    ctx.data_mut(|d| d.remove_temp::<Vec<Action>>(key()))
        .unwrap_or_default()
}

pub(super) fn queue(ui: &Ui, action: Action) {
    ui.ctx().data_mut(|d| {
        let mut actions = d.get_temp::<Vec<Action>>(key()).unwrap_or_default();
        actions.push(action);
        d.insert_temp(key(), actions);
    });
    ui.ctx().request_repaint();
}

pub(super) fn snapshot(ctx: &Context, config: &Config) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("midi-context-config"), config.clone()));
}

pub(crate) trait MidiMenu {
    fn midi_menu(self, ui: &mut Ui, id: &str) -> Self;
}

impl MidiMenu for Response {
    fn midi_menu(self, _ui: &mut Ui, id: &str) -> Self {
        let Some(kind) = kind(id) else { return self };
        if !self.enabled() {
            return self;
        }
        self.container_context_menu(|menu| {
            menu.strong(label(id));
            if menu.button("MIDI LEARN...").clicked() {
                queue(menu, Action::Learn(id.into()));
                menu.close();
            }
            let config = menu
                .ctx()
                .data(|d| d.get_temp::<Config>(egui::Id::unique("midi-context-config")))
                .unwrap_or_default();
            if let Some(binding) = config.bindings.iter().find(|b| b.target == id) {
                menu.small(format!(
                    "Input {} / Ch {} / CC {}",
                    binding.input + 1,
                    binding.channel,
                    binding.controller
                ));
                if menu.button("Remove MIDI assignment").clicked() {
                    queue(menu, Action::Remove(id.into()));
                    menu.close();
                }
                if matches!(kind, Kind::Knob | Kind::Fader | Kind::Number) {
                    menu.separator();
                    menu.label("Maximum parameter value (0–127)");
                    let mut range = config.ranges.get(id).copied().unwrap_or_default();
                    let old = range;
                    menu.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut range.max)
                                .range(0..=127)
                                .prefix("MAX "),
                        );
                    });
                    if menu.button("Reset MAX to 127").clicked() {
                        range = ValueRange::default();
                    }
                    if range != old {
                        queue(menu, Action::Range(id.into(), range));
                    }
                    menu.small("Fader fully up = MAX. Fader fully down = 0. MIDI only.");
                }
            } else {
                menu.small("Move a controller after choosing MIDI LEARN.");
            }
        });
        // Keep the original response: painting and primary-button gestures are unchanged.
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn right_click_opens_learn_and_the_chosen_max_without_editing_the_control() {
        let ctx = Context::default();
        let mut config = Config::default();
        config.assign(super::super::Binding {
            input: 0,
            channel: 1,
            controller: 7,
            target: "mixer1.level".into(),
        });
        config
            .ranges
            .insert("mixer1.level".into(), ValueRange { max: 50 });
        snapshot(&ctx, &config);
        let mut value = 0.4_f32;
        let mut position = egui::Pos2::ZERO;
        let mut text = String::new();
        fn collect(shape: &egui::Shape, out: &mut String) {
            match shape {
                egui::Shape::Text(t) => {
                    out.push_str(&t.galley.job.text);
                    out.push('\n');
                }
                egui::Shape::Vec(v) => {
                    for s in v {
                        collect(s, out);
                    }
                }
                _ => {}
            }
        }
        for pressed in [None, Some(true), Some(false), None, None] {
            let events = pressed.map_or_else(Vec::new, |pressed| {
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: egui::Modifiers::default(),
                    },
                ]
            });
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600.0, 500.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    let response = ui
                        .add(slider(ui, &mut value, 0.0..=1.0))
                        .midi_menu(ui, "mixer1.level");
                    position = response.rect.center();
                },
            );
            for shape in &output.shapes {
                collect(&shape.shape, &mut text);
            }
            output.textures_delta.clear();
        }
        assert_eq!(value, 0.4);
        assert!(text.contains("MIDI LEARN..."));
        assert!(text.contains("Remove MIDI assignment"));
        assert!(text.contains("Reset MAX to 127"));
        assert!(!text.contains("Min "));
    }
}
