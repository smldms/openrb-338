//! Permanent sampler and effect column for the Graphite rack.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, Ui, Vec2};
use orb_engine::studio::sampler::{Lane, Mode};
use orb_player::Command;

use super::editors::{trim_at, waveform};
use crate::app::App;
use crate::midi::MidiMenu;

const ACID: Color32 = Color32::from_rgb(190, 215, 126);
const FACE: Color32 = Color32::from_rgb(29, 31, 32);

fn surface(ui: &mut Ui, rect: Rect, id: &str, draw: impl FnOnce(&mut Ui)) {
    ui.painter().rect_filled(rect, 5.0, FACE);
    ui.painter().rect_stroke(
        rect,
        5.0,
        Stroke::new(1.0, Color32::from_gray(55)),
        egui::StrokeKind::Inside,
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect.shrink(8.0))
            .id_salt(id),
        |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(rect));
            let scale = (rect.width() / 300.0).clamp(0.8, 1.15);
            ui.style_mut().override_font_id = Some(FontId::proportional(12.0 * scale));
            ui.spacing_mut().item_spacing = Vec2::new(5.0 * scale, 3.0 * scale);
            ui.spacing_mut().interact_size.y = 18.0 * scale;
            ui.visuals_mut().selection.bg_fill = Color32::from_rgb(67, 78, 46);
            ui.visuals_mut().selection.stroke = Stroke::new(1.0, ACID);
            draw(ui);
        },
    );
}

fn float_knob(
    ui: &mut Ui,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    label: &str,
    id: &str,
) {
    let (lo, hi) = (*range.start(), *range.end());
    ui.vertical(|ui| {
        let width = (ui.available_width() / 3.0).clamp(32.0, 52.0);
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, width + 34.0), Sense::click_and_drag());
        let response = response.midi_menu(ui, id);
        if response.dragged_by(egui::PointerButton::Primary) {
            let delta = ui.input(|i| -i.pointer.delta().y);
            let fine = ui.input(|i| if i.modifiers.shift { 0.1 } else { 1.0 });
            *value = (*value + f64::from(delta * fine) * (hi - lo) / 160.0).clamp(lo, hi);
        }
        let center = rect.center_top() + Vec2::new(0.0, width * 0.5);
        let radius = width * 0.34;
        ui.painter()
            .circle_filled(center, radius, Color32::from_gray(23));
        ui.painter()
            .circle_stroke(center, radius, Stroke::new(2.0, Color32::from_gray(83)));
        let angle = -std::f32::consts::PI * 1.25
            + ((*value - lo) / (hi - lo)) as f32 * std::f32::consts::PI * 1.5;
        let tip = center + Vec2::angled(angle) * radius * 0.82;
        ui.painter()
            .line_segment([center, tip], Stroke::new(2.0, ACID));
        ui.painter().text(
            rect.center_bottom() - Vec2::new(0.0, 20.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(10.5),
            Color32::from_gray(200),
        );
        ui.painter().text(
            rect.center_bottom() - Vec2::new(0.0, 2.0),
            Align2::CENTER_BOTTOM,
            format!("{:.2}", *value),
            FontId::monospace(9.5),
            Color32::GRAY,
        );
        response.on_hover_text(format!(
            "{label}: drag vertically; Shift for fine adjustment"
        ));
    });
}

fn compact_knob(
    ui: &mut Ui,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    label: &str,
    id: &str,
    integer: bool,
) -> egui::Response {
    let (lo, hi) = (*range.start(), *range.end());
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(58.0, 52.0), Sense::click_and_drag());
    let drag = ui.make_persistent_id((id, "knob-origin"));
    let now = ui.input(|i| i.time);
    let state = ui.data(|d| d.get_temp::<(f64, f64)>(drag));
    if response.drag_started_by(egui::PointerButton::Primary)
        && state.is_none_or(|(_, time)| time != now)
    {
        ui.data_mut(|d| d.insert_temp(drag, (*value, f64::NEG_INFINITY)));
    }
    if response.dragged_by(egui::PointerButton::Primary) {
        let (base, consumed) = ui
            .data(|d| d.get_temp::<(f64, f64)>(drag))
            .unwrap_or((*value, f64::NEG_INFINITY));
        let fine = ui.input(|i| if i.modifiers.shift { 0.1 } else { 1.0 });
        let delta = response.drag_delta().y;
        let next = if consumed == now {
            base
        } else {
            (base - f64::from(delta) * (hi - lo) * fine / 160.0).clamp(lo, hi)
        };
        ui.data_mut(|d| d.insert_temp(drag, (next, now)));
        let next = if integer { next.round() } else { next };
        if next != *value {
            *value = next;
            response.mark_changed();
        }
    }
    let center = rect.center_top() + Vec2::new(0.0, 14.0);
    let angle = -std::f32::consts::PI * 1.25
        + ((*value - lo) / (hi - lo)) as f32 * std::f32::consts::PI * 1.5;
    ui.painter()
        .circle_filled(center, 11.0, Color32::from_gray(22));
    ui.painter()
        .circle_stroke(center, 11.0, Stroke::new(1.5, Color32::from_gray(80)));
    ui.painter().line_segment(
        [center, center + Vec2::angled(angle) * 9.0],
        Stroke::new(1.5, ACID),
    );
    ui.painter().text(
        rect.center_top() + Vec2::new(0.0, 33.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(9.0),
        Color32::LIGHT_GRAY,
    );
    let text = if integer {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    };
    ui.painter().text(
        rect.center_bottom(),
        Align2::CENTER_BOTTOM,
        text,
        FontId::monospace(9.0),
        Color32::LIGHT_GRAY,
    );
    response.midi_menu(ui, id).on_hover_text(format!(
        "{label}: drag vertically; Shift for fine adjustment"
    ))
}

fn sample_knobs(ui: &mut Ui, lane: &mut Lane, prefix: &str) -> [Rect; 3] {
    let mut pitch = f64::from(lane.pitch);
    let p = compact_knob(
        ui,
        &mut pitch,
        -24.0..=24.0,
        "PITCH",
        &format!("{prefix}.pitch"),
        true,
    );
    if p.changed() {
        lane.pitch = pitch as i8;
    }
    let l = compact_knob(
        ui,
        &mut lane.gain,
        0.0..=2.0,
        "LEVEL",
        &format!("{prefix}.level"),
        false,
    );
    let r = compact_knob(
        ui,
        &mut lane.send,
        0.0..=1.0,
        "REVERB",
        &format!("{prefix}.send"),
        false,
    );
    [p.rect, l.rect, r.rect]
}

pub(super) fn gater_controls(ui: &mut Ui, lane: &mut Lane, prefix: &str) {
    ui.horizontal_wrapped(|ui| {
        let mut on = lane.gater_source != 0;
        if ui
            .checkbox(&mut on, "GATER")
            .midi_menu(ui, &format!("{prefix}.gater_source"))
            .changed()
        {
            lane.gater_source = if on { 4 } else { 0 };
        }
        egui::ComboBox::from_id_salt("gater-source")
            .width(66.0)
            .selected_text(match lane.gater_source {
                1 => "TB-A",
                2 => "TB-B",
                3 => "TR-808",
                _ => "TR-909",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut lane.gater_source, 3, "TR-808")
                    .midi_menu(ui, &format!("{prefix}.gater_source"));
                ui.selectable_value(&mut lane.gater_source, 4, "TR-909")
                    .midi_menu(ui, &format!("{prefix}.gater_source"));
            });
        let instruments = if lane.gater_source == 3 {
            [
                "BD", "SD", "LT", "MT", "HT", "RS", "CP", "CB", "CY", "OH", "CH",
            ]
        } else {
            [
                "BD", "SD", "LT", "MT", "HT", "RS", "CP", "CH", "OH", "CC", "RC",
            ]
        };
        let selected = usize::from(lane.gater_instrument.clamp(1, 11) - 1);
        egui::ComboBox::from_id_salt("gater-instrument")
            .width(42.0)
            .selected_text(instruments[selected])
            .show_ui(ui, |ui| {
                for (i, label) in instruments.iter().enumerate() {
                    ui.selectable_value(&mut lane.gater_instrument, i as u8 + 1, *label)
                        .midi_menu(ui, &format!("{prefix}.gater_instrument"));
                }
            });
        let mut depth = lane.gater_depth * 100.0;
        if ui
            .add(
                crate::midi::drag_value(ui, &mut depth)
                    .range(0.0..=100.0)
                    .prefix("Depth ")
                    .suffix("%"),
            )
            .changed()
        {
            lane.gater_depth = depth / 100.0;
        }
        let holds = [
            (0.125, "1/128"),
            (0.25, "1/64"),
            (0.5, "1/32"),
            (1.0, "1/16"),
            (2.0, "1/8"),
            (4.0, "1/4"),
            (8.0, "1/2"),
            (16.0, "1 bar"),
        ];
        let hold_text = holds
            .iter()
            .find(|(v, _)| (lane.gater_hold - v).abs() < 1e-6)
            .map_or_else(
                || format!("{:.2} steps", lane.gater_hold),
                |(_, text)| (*text).into(),
            );
        egui::ComboBox::from_id_salt("gater-hold")
            .width(73.0)
            .selected_text(format!("Hold {hold_text}"))
            .show_ui(ui, |ui| {
                for (value, text) in [
                    (0.125, "1/128"),
                    (0.25, "1/64"),
                    (0.5, "1/32"),
                    (1.0, "1/16"),
                    (2.0, "1/8"),
                    (4.0, "1/4"),
                    (8.0, "1/2"),
                    (16.0, "1 bar"),
                ] {
                    ui.selectable_value(&mut lane.gater_hold, value, text)
                        .midi_menu(ui, &format!("{prefix}.gater_hold"));
                }
            });
        ui.add(
            crate::midi::drag_value(ui, &mut lane.gater_release_ms)
                .range(1.0..=500.0)
                .prefix("Smooth ")
                .suffix("ms"),
        )
        .midi_menu(ui, &format!("{prefix}.gater_release_ms"));
        ui.checkbox(&mut lane.sidechain, "SIDECHAIN")
            .midi_menu(ui, &format!("{prefix}.sidechain"))
            .on_hover_text(
                "Sampler ducking follows the selected sidechain drum voice, source and release",
            );
    });
}

pub(super) fn delay_timing(ui: &mut Ui, panel: &mut crate::model::Panel) -> [Rect; 3] {
    let mut rects = [Rect::NOTHING; 3];
    ui.horizontal_wrapped(|ui| {
        let mut steps = f64::from(panel.display("delay.steps").unwrap_or(3));
        let response = compact_knob(ui, &mut steps, 1.0..=32.0, "STEPS", "delay.steps", true);
        rects[0] = response.rect;
        if response.changed() {
            panel.set_delay_steps(steps as i32);
        }
    });
    ui.horizontal_wrapped(|ui| {
        let mut mode = panel.switch_position("delay.triplet");
        for (i, (value, label)) in [(1, "STRAIGHT"), (0, "TRIPLET")].into_iter().enumerate() {
            let response = ui
                .selectable_value(&mut mode, value, label)
                .midi_menu(ui, "delay.triplet");
            rects[i + 1] = response.rect;
            if response.changed() {
                panel.set_switch("delay.triplet", mode);
            }
        }
    });
    rects
}

pub(super) fn warp_controls(
    ui: &mut Ui,
    lane: &mut Lane,
    frames: Option<usize>,
    prefix: &str,
) -> bool {
    let mut edit = false;
    ui.horizontal_wrapped(|ui| {
        if ui
            .checkbox(&mut lane.warp, "WARP")
            .midi_menu(ui, &format!("{prefix}.warp"))
            .on_hover_text("Enable and fit the selected region to Beats at the song tempo")
            .changed()
            && lane.warp
        {
            lane.fit_warp(frames);
        }
        if lane.warp_grid || !lane.warp_markers.points().is_empty() {
            ui.small("Beat fit");
            if lane.warp_markers.points().is_empty() && ui.small_button("BPM mode").clicked() {
                lane.warp_grid = false;
            }
        } else {
            ui.add(
                crate::midi::drag_value(ui, &mut lane.warp_bpm)
                    .range(20.0..=400.0)
                    .speed(0.1)
                    .prefix("Src BPM "),
            )
            .midi_menu(ui, &format!("{prefix}.warp_bpm"))
            .on_hover_text("The original tempo of this sample, not the song tempo");
        }
        if ui
            .add(
                crate::midi::drag_value(ui, &mut lane.warp_beats)
                    .range(1..=256)
                    .prefix("Beats "),
            )
            .midi_menu(ui, &format!("{prefix}.warp_beats"))
            .on_hover_text("Number of quarter-note beats in the selected IN/OUT region")
            .changed()
        {
            lane.warp_grid = true;
        }
        if ui
            .add_enabled(frames.is_some(), egui::Button::new("FIT"))
            .midi_menu(ui, &format!("{prefix}.warp_fit"))
            .on_hover_text("Calculate source BPM from Beats and the selected audio; enable Warp")
            .clicked()
        {
            lane.fit_warp(frames);
        }
        edit = ui
            .add_enabled(frames.is_some(), egui::Button::new("MARKERS / GRID"))
            .on_hover_text("Edit warp markers on the beat grid; stretch individual regions")
            .clicked();
        ui.small(if lane.warp {
            "Tempo sync"
        } else {
            "Free playback"
        });
    });
    edit
}

struct LaneRowOptions {
    active: bool,
    can_unload: bool,
}

fn lane_row(
    ui: &mut Ui,
    index: usize,
    name: &str,
    options: LaneRowOptions,
    lane: &mut Lane,
    selected: &mut usize,
    row_width: f32,
) -> [Rect; 3] {
    let mut regions = [Rect::NOTHING; 3];
    ui.push_id(("sampler-lane", index), |ui| {
        ui.spacing_mut().interact_size.x = 20.0;
        ui.spacing_mut().button_padding.x = 3.0;
        ui.horizontal(|ui| {
            let width = (row_width - 40.0 - 2.0 * ui.spacing().item_spacing.x).max(40.0);
            let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 20.0), Sense::click());
            regions[0] = rect;
            ui.painter().rect_filled(
                rect,
                3.0,
                if options.active {
                    Color32::from_rgb(67, 78, 46)
                } else {
                    Color32::from_gray(43)
                },
            );
            ui.painter().with_clip_rect(rect.shrink(3.0)).text(
                rect.left_center() + Vec2::new(4.0, 0.0),
                Align2::LEFT_CENTER,
                format!("{}  {name}", index + 1),
                FontId::proportional(11.0),
                if lane.muted {
                    Color32::GRAY
                } else {
                    Color32::WHITE
                },
            );
            let response = response.on_hover_text(name);
            if response.clicked() {
                *selected = index;
            }
            response.context_menu(|ui| {
                if ui
                    .add_enabled(options.can_unload, egui::Button::new("Unload sample"))
                    .clicked()
                {
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(egui::Id::unique("sampler-unload"), index));
                    ui.close();
                }
            });
            let solo = ui.add_sized([20.0, 20.0], egui::Button::new("S").selected(lane.solo));
            regions[1] = solo.rect;
            if solo
                .midi_menu(ui, &format!("sampler{}.solo", index + 1))
                .on_hover_text("Solo this sample")
                .clicked()
            {
                lane.solo = !lane.solo;
            }
            let mute = ui.add_sized([20.0, 20.0], egui::Button::new("M").selected(lane.muted));
            regions[2] = mute.rect;
            if mute
                .midi_menu(ui, &format!("sampler{}.mute", index + 1))
                .on_hover_text("Mute this sample")
                .clicked()
            {
                lane.muted = !lane.muted;
            }
        });
    });
    regions
}

impl App {
    fn graphite_sampler_mixer(&mut self, ui: &mut Ui, rect: Rect) {
        let active = !self.browsing() && !self.prompting() && !self.error_open();
        let meter = self
            .link
            .as_mut()
            .map(|link| link.player().snapshot().sampler_meter)
            .unwrap_or([0; 2]);
        if meter.iter().any(|&v| v > 0) {
            ui.ctx().request_repaint_after(crate::app::METER_FRAME);
        }
        // Reuse the four native mixer controls and their exact relative geometry.
        let scale = rect.height() / 320.0;
        let x_scale = crate::graphite_rack::WIDTH / 1280.0 * scale;
        let at = |x: f32, y: f32, w: f32, h: f32| {
            Rect::from_min_size(
                rect.left_top() + Vec2::new(x * x_scale, y * scale),
                Vec2::new(w * x_scale, h * scale),
            )
        };
        ui.painter().rect_filled(rect, 5.0, FACE);
        ui.painter().text(
            at(13.0, 19.0, 0.0, 0.0).min,
            Align2::LEFT_CENTER,
            "MIX",
            FontId::proportional(12.0 * scale),
            Color32::from_rgb(133, 138, 136),
        );
        ui.add_enabled_ui(active && !self.studio.history_restoring, |ui| {
            let bus = &mut self.studio.settings.sampler_bus;
            if crate::graphite_rack::machine_power_button(
                ui,
                at(145.0, 9.0, 42.0, 19.0),
                scale,
                "sampler.on",
                bus.enabled,
            )
            .clicked()
            {
                bus.enabled = !bus.enabled;
            }
            let mut level = (bus.gain / 2.0 * 999.0).round() as i32;
            if crate::graphite_rack::mix_fader(
                ui,
                scale,
                "sampler.level",
                at(16.0, 36.0, 12.0, 43.0),
                true,
                &mut level,
            )
            .changed()
            {
                bus.gain = f64::from(level) / 999.0 * 2.0;
            }
            crate::graphite_rack::mix_meter(
                ui,
                at(42.0, 38.0, 5.0, 40.0),
                crate::link::meter_state(meter[0].max(meter[1])),
            );
            let mut pan = ((bus.pan + 1.0) * 500.0).round() as i32;
            if crate::graphite_rack::mix_knob(
                ui,
                scale,
                "sampler.pan",
                "PAN",
                at(66.0, 33.0, 32.0, 47.0),
                &mut pan,
            )
            .changed()
            {
                bus.pan = f64::from(pan) / 500.0 - 1.0;
            }
            let mut delay = (bus.delay * 1000.0).round() as i32;
            if crate::graphite_rack::mix_knob(
                ui,
                scale,
                "sampler.delay",
                "DELAY",
                at(128.0, 33.0, 32.0, 47.0),
                &mut delay,
            )
            .changed()
            {
                bus.delay = f64::from(delay) / 1000.0;
            }
            if crate::graphite_rack::mixer_button(
                ui,
                at(12.0, 89.0, 82.0, 21.0),
                scale,
                "sampler.dist",
                "DIST",
                bus.dist,
            )
            .clicked()
            {
                bus.dist = !bus.dist;
            }
            let on = bus.reverb_send > 0.0;
            if crate::graphite_rack::mixer_button(
                ui,
                at(106.0, 89.0, 82.0, 21.0),
                scale,
                "sampler.send",
                "REV",
                on,
            )
            .clicked()
            {
                bus.reverb_send = if on { 0.0 } else { 0.3 };
            }
            let mut reverb = (bus.reverb_send * 1000.0).round() as i32;
            if crate::graphite_rack::mix_knob(
                ui,
                scale,
                "sampler.send",
                "REV SEND",
                at(20.0, 130.0, 32.0, 47.0),
                &mut reverb,
            )
            .changed()
            {
                bus.reverb_send = f64::from(reverb) / 1000.0;
            }
        });
    }

    pub(crate) fn graphite_sampler(&mut self, ui: &mut Ui, rect: Rect) {
        let before = self.studio.settings.clone();
        let mut load = None;
        let mut audition = false;
        let selected = self.studio.selected.min(7);
        let busy = self.studio.worker.is_some() || !self.studio.ready;
        let context = ui.ctx().clone();
        let tick = self.panel.transport.position.max(0) as usize;
        let mixer_width = rect.width() * (198.0 / 1030.0);
        let mixer = Rect::from_min_max(
            egui::pos2(rect.right() - mixer_width, rect.top()),
            rect.right_bottom(),
        );
        let rect = Rect::from_min_max(
            rect.left_top(),
            egui::pos2(mixer.left() - 6.0, rect.bottom()),
        );
        surface(ui, rect, "graphite-sampler", |ui| {
            if self.studio.history_restoring {
                ui.disable();
            }
            egui::ScrollArea::vertical()
                .id_salt("sampler-rack-scroll")
                .max_height(ui.available_height())
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(ACID, egui::RichText::new("SAMPLER").strong());
                        ui.small("8 LANES / 64 BARS");
                        self.sampler_history_buttons(ui);
                        if ui.add_enabled(!busy, egui::Button::new("LOAD")).clicked() {
                            load = Some(selected);
                        }
                        if ui
                            .add_enabled(
                                self.studio.waves[selected].is_some(),
                                egui::Button::new("AUDITION"),
                            )
                            .clicked()
                        {
                            audition = true;
                        }
                        if ui.button("EDIT / ALL LANES").clicked() {
                            self.studio.sampler_open = true;
                        }
                        ui.checkbox(&mut self.studio.follow, "Follow");
                        if busy {
                            ui.spinner();
                        }
                    });
                    ui.separator();
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(rect.width() * 0.22);
                            for i in 0..8 {
                                let lane = &mut self.studio.settings.lanes[i];
                                lane_row(
                                    ui,
                                    i,
                                    &self.studio.names[i],
                                    LaneRowOptions {
                                        can_unload: self.studio.waves[i].is_some() && !busy,
                                        active: i == selected,
                                    },
                                    lane,
                                    &mut self.studio.selected,
                                    rect.width() * 0.22,
                                );
                            }
                        });
                        ui.vertical(|ui| {
                            ui.set_width((rect.width() * 0.75 - 16.0).max(100.0));
                            let lane = &mut self.studio.settings.lanes[selected];
                            if let Some(wave) = &self.studio.waves[selected] {
                                let response = waveform(
                                    ui,
                                    &wave.frames,
                                    (rect.height() * 0.25).max(32.0),
                                    lane.start,
                                    lane.end,
                                    self.studio.waveform_epoch,
                                    &mut self.studio.wave_views[selected],
                                );
                                if let Some(head) = self.panel.sampler_playheads[selected] {
                                    super::editors::draw_playhead(ui, response.rect, head.source);
                                }
                                let minimum = 2.0 / wave.frames.len().max(2) as f64;
                                if response.double_clicked() {
                                    lane.start = 0.0;
                                    lane.end = 1.0;
                                } else if let Some(p) = response.interact_pointer_pos() {
                                    let at = f64::from(
                                        (p.x - response.rect.left()) / response.rect.width(),
                                    )
                                    .clamp(0.0, 1.0);
                                    let id = response.id.with("handle");
                                    if response.drag_started() || response.clicked() {
                                        let entry =
                                            (at - lane.start).abs() <= (at - lane.end).abs();
                                        ui.data_mut(|d| d.insert_temp(id, entry));
                                    }
                                    if response.dragged_by(egui::PointerButton::Primary)
                                        || response.clicked()
                                    {
                                        let entry =
                                            ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(true);
                                        trim_at(lane, entry, at, minimum);
                                    }
                                }
                                let duration = wave.frames.len() as f64 / 44100.0;
                                let mut entry = lane.start * duration;
                                let mut exit = lane.end * duration;
                                ui.horizontal(|ui| {
                                    if ui
                                        .add(
                                            crate::midi::drag_value(ui, &mut entry)
                                                .speed(0.001)
                                                .fixed_decimals(3)
                                                .prefix("IN ")
                                                .suffix("s"),
                                        )
                                        .changed()
                                    {
                                        trim_at(lane, true, entry / duration, minimum);
                                    }
                                    if ui
                                        .add(
                                            crate::midi::drag_value(ui, &mut exit)
                                                .speed(0.001)
                                                .fixed_decimals(3)
                                                .prefix("OUT ")
                                                .suffix("s"),
                                        )
                                        .changed()
                                    {
                                        trim_at(lane, false, exit / duration, minimum);
                                    }
                                });
                            } else {
                                ui.allocate_ui(
                                    Vec2::new(ui.available_width(), rect.height() * 0.28),
                                    |ui| {
                                        ui.centered_and_justified(|ui| {
                                            ui.label("LOAD WAV / AIFF / MP3");
                                        });
                                    },
                                );
                            }
                            ui.horizontal_wrapped(|ui| {
                                for (mode, label) in [
                                    (Mode::OneShot, "ONE SHOT"),
                                    (Mode::AutoCut, "AUTO CUT"),
                                    (Mode::Gate, "GATE"),
                                    (Mode::Loop, "LOOP"),
                                ] {
                                    ui.selectable_value(&mut lane.mode, mode, label)
                                        .midi_menu(ui, &format!("sampler{}.mode", selected + 1));
                                }
                                if ui
                                    .selectable_label(lane.reverse, "REVERSE")
                                    .midi_menu(ui, &format!("sampler{}.reverse", selected + 1))
                                    .clicked()
                                {
                                    lane.reverse = !lane.reverse;
                                }
                                sample_knobs(ui, lane, &format!("sampler{}", selected + 1));
                            });
                            let frames =
                                self.studio.waves[selected].as_ref().map(|w| w.frames.len());
                            if warp_controls(ui, lane, frames, &format!("sampler{}", selected + 1))
                            {
                                self.studio.warp_editor = Some(selected);
                            }
                            gater_controls(ui, lane, &format!("sampler{}", selected + 1));
                            let bars = usize::from(lane.bars.clamp(1, 64));
                            if self.studio.follow && self.panel.transport.playing {
                                self.studio.bar = tick / 768 % bars;
                            }
                            self.studio.bar = self.studio.bar.min(bars - 1);
                            ui.horizontal(|ui| {
                                ui.add(
                                    crate::midi::drag_value(ui, &mut lane.bars)
                                        .range(1..=64)
                                        .prefix("Length ")
                                        .suffix(" bars"),
                                );
                                let mut bar = self.studio.bar + 1;
                                if ui
                                    .add(
                                        crate::midi::drag_value(ui, &mut bar)
                                            .range(1..=bars)
                                            .prefix("Bar "),
                                    )
                                    .changed()
                                {
                                    self.studio.bar = bar - 1;
                                    self.studio.follow = false;
                                }
                            });
                            ui.horizontal(|ui| {
                                let width = ((ui.available_width() - 15.0 * 3.0) / 16.0).max(8.0);
                                ui.spacing_mut().item_spacing.x = 3.0;
                                for step in 0..16 {
                                    let index = self.studio.bar * 16 + step;
                                    let on = lane.steps[index] != 0;
                                    let head = self.panel.transport.playing
                                        && tick / 48 % (usize::from(lane.bars) * 16) == index;
                                    let button = egui::Button::new(format!("{}", step + 1))
                                        .fill(if on {
                                            Color32::from_rgb(94, 112, 50)
                                        } else {
                                            FACE
                                        })
                                        .stroke(Stroke::new(
                                            if head { 2.0 } else { 1.0 },
                                            if head { ACID } else { Color32::from_gray(70) },
                                        ));
                                    let response = ui.add_sized([width, 19.0], button);
                                    if response.clicked() {
                                        lane.steps[index] = if on { 0 } else { 100 };
                                    }
                                    response.context_menu(|ui| {
                                        ui.add(
                                            crate::midi::slider(
                                                ui,
                                                &mut lane.steps[index],
                                                0..=127,
                                            )
                                            .text("Velocity"),
                                        );
                                    });
                                }
                            });
                        });
                    });
                });
        });
        self.graphite_sampler_mixer(ui, mixer);
        if self.studio.settings != before {
            self.studio.changed();
            self.doc.modified = true;
        }
        if let Some(lane) = load {
            self.studio_load_wave(&context, lane);
        }
        if let Some(lane) =
            context.data_mut(|d| d.remove_temp::<usize>(egui::Id::unique("sampler-unload")))
        {
            self.sampler_unload(lane);
        }
        if audition {
            self.studio_command(Command::StudioAudition(selected));
        }
    }

    pub(crate) fn graphite_fx(&mut self, ui: &mut Ui, rect: Rect) {
        let before = self.studio.settings.clone();
        let width = rect.width();
        let gap = 7.0;
        let heights = [0.23, 0.18, 0.14, 0.14, 0.14, 0.17];
        let mut top = rect.top();
        for (i, fraction) in heights.into_iter().enumerate() {
            let card = Rect::from_min_size(
                egui::pos2(rect.left(), top),
                Vec2::new(width, rect.height() * fraction - gap),
            );
            top = card.bottom() + gap;
            surface(ui, card, &format!("graphite-fx-{i}"), |ui| match i {
                0 => {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(self.studio.settings.enabled, "SPACE")
                            .midi_menu(ui, "studio.enabled")
                            .clicked()
                        {
                            self.studio.settings.enabled = !self.studio.settings.enabled;
                        }
                        ui.small("CONVOLUTION");
                        if ui.small_button("...").clicked() {
                            self.studio.reverb_open = true;
                        }
                    });
                    ui.label(&self.studio.ir_name);
                    ui.horizontal(|ui| {
                        let s = &mut self.studio.settings;
                        float_knob(ui, &mut s.return_gain, 0.0..=1.0, "RETURN", "studio.return");
                        float_knob(
                            ui,
                            &mut s.predelay_ms,
                            0.0..=200.0,
                            "PREDELAY",
                            "studio.predelay",
                        );
                        float_knob(ui, &mut s.width, 0.0..=2.0, "WIDTH", "studio.width");
                    });
                    ui.horizontal(|ui| {
                        float_knob(
                            ui,
                            &mut self.studio.settings.low_cut,
                            20.0..=2000.0,
                            "LOW CUT",
                            "studio.low_cut",
                        );
                        float_knob(
                            ui,
                            &mut self.studio.settings.high_cut,
                            2000.0..=20000.0,
                            "HIGH CUT",
                            "studio.high_cut",
                        );
                        if ui.small_button("IR / SENDS").clicked() {
                            self.studio.reverb_open = true;
                        }
                    });
                }
                1..=4 => {
                    let (section, title, parameters): (&str, &str, &[(&str, &str)]) = match i {
                        1 => ("delay", "DELAY", &[("pan", "PAN"), ("feedback", "FDBK")]),
                        2 => ("dist", "DIST", &[("amount", "AMOUNT"), ("shape", "SHAPE")]),
                        3 => (
                            "pcf",
                            "PCF",
                            &[("freq", "FREQ"), ("q", "Q"), ("amt", "AMOUNT")],
                        ),
                        _ => (
                            "comp",
                            "COMP",
                            &[("threshold", "THRESH"), ("ratio", "RATIO")],
                        ),
                    };
                    ui.horizontal(|ui| {
                        let id = format!("{section}.on");
                        if ui
                            .selectable_label(self.panel.toggle_on(&id), title)
                            .midi_menu(ui, &id)
                            .clicked()
                        {
                            self.panel.press(&id, 0.0);
                        }
                        if ui.small_button("EDIT").clicked() {
                            self.studio.fx = Some(section.into());
                        }
                    });
                    if section == "delay" {
                        delay_timing(ui, &mut self.panel);
                    }
                    ui.horizontal(|ui| {
                        for &(name, label) in parameters {
                            let id = format!("{section}.{name}");
                            if self.panel.value(&id).is_none() {
                                continue;
                            }
                            let old = self.panel.knob(&id);
                            let mut value = f64::from(old) / 1000.0;
                            ui.push_id(&id, |ui| {
                                if section == "delay" {
                                    compact_knob(ui, &mut value, 0.0..=1.0, label, &id, false);
                                } else {
                                    float_knob(ui, &mut value, 0.0..=1.0, label, &id);
                                }
                            });
                            let next = (value * 1000.0).round() as i32;
                            if next != old {
                                self.panel.set_knob(&id, next);
                            }
                        }
                    });
                }
                _ => {
                    ui.horizontal(|ui| {
                        ui.colored_label(ACID, "MASTER");
                        let on = self.panel.toggle_on("master.comp");
                        if ui
                            .selectable_label(on, "COMP")
                            .midi_menu(ui, "master.comp")
                            .clicked()
                        {
                            self.panel.press("master.comp", 0.0);
                        }
                    });
                    let mut level = self.panel.fader("master.fader");
                    if ui
                        .add(crate::midi::slider(ui, &mut level, 0..=999).text("LEVEL"))
                        .midi_menu(ui, "master.fader")
                        .changed()
                    {
                        self.panel.set_fader("master.fader", level);
                    }
                    for key in ["master.meter_l", "master.meter_r"] {
                        let level = f32::from(*self.panel.meters.get(key).unwrap_or(&0)) / 31.0;
                        ui.add(egui::ProgressBar::new(level.clamp(0.0, 1.0)).desired_height(8.0));
                    }
                    ui.small("Stereo master / compressor before output");
                }
            });
        }
        if self.studio.settings != before {
            self.studio.changed();
            self.doc.modified = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn right_click_sample_name_opens_unload_for_that_lane_without_selecting_it() {
        fn text_position(shape: &egui::Shape) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == "Unload sample" => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(text_position),
                _ => None,
            }
        }
        let ctx = egui::Context::default();
        let mut lane = Lane::default();
        let mut selected = 0;
        let mut frame = |events| {
            let mut name = Rect::NOTHING;
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(500.0, 400.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    name = lane_row(
                        ui,
                        4,
                        "Loaded.wav",
                        LaneRowOptions {
                            can_unload: true,
                            active: false,
                        },
                        &mut lane,
                        &mut selected,
                        240.0,
                    )[0];
                },
            );
            out.textures_delta.clear();
            (
                name,
                out.shapes.iter().find_map(|s| text_position(&s.shape)),
            )
        };
        let pos = frame(vec![]).0.center();
        let click = |pos, button, pressed| egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Default::default(),
        };
        frame(vec![
            egui::Event::PointerMoved(pos),
            click(pos, egui::PointerButton::Secondary, true),
        ]);
        frame(vec![click(pos, egui::PointerButton::Secondary, false)]);
        let option = frame(vec![]).1.expect("Unload menu must be visible");
        frame(vec![
            egui::Event::PointerMoved(option),
            click(option, egui::PointerButton::Primary, true),
        ]);
        frame(vec![click(option, egui::PointerButton::Primary, false)]);
        assert_eq!(selected, 0);
        assert_eq!(
            ctx.data(|d| d.get_temp::<usize>(egui::Id::unique("sampler-unload"))),
            Some(4)
        );
    }

    #[test]
    fn sample_knobs_keep_pitch_discrete_and_shift_does_not_jump_during_drag() {
        let ctx = egui::Context::default();
        let mut lane = Lane::default();
        let frame = |lane: &mut Lane, events| {
            let mut rects = [Rect::NOTHING; 3];
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(500.0, 400.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.horizontal(|ui| {
                        rects = sample_knobs(ui, lane, "sampler1");
                    });
                },
            );
            out.textures_delta.clear();
            rects
        };
        let rects = frame(&mut lane, vec![]);
        let event = |pos, button, pressed| egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Default::default(),
        };
        let start = rects[1].center();
        let end = start - Vec2::new(0.0, 80.0);
        frame(
            &mut lane,
            vec![
                egui::Event::PointerMoved(start),
                event(start, egui::PointerButton::Primary, true),
            ],
        );
        frame(&mut lane, vec![egui::Event::PointerMoved(end)]);
        let value = lane.gain;
        assert!((value - 1.7).abs() < 1e-9);
        let shift = egui::Modifiers {
            shift: true,
            ..Default::default()
        };
        frame(&mut lane, vec![egui::Event::ModifiersChanged(shift)]);
        assert_eq!(lane.gain, value);
        let fine = end - Vec2::new(0.0, 16.0);
        frame(&mut lane, vec![egui::Event::PointerMoved(fine)]);
        assert!((lane.gain - value - 0.02).abs() < 1e-9);
        frame(
            &mut lane,
            vec![
                event(fine, egui::PointerButton::Primary, false),
                egui::Event::ModifiersChanged(Default::default()),
            ],
        );
        for index in [0, 2] {
            let pos = rects[index].center();
            let end = pos - Vec2::new(0.0, 400.0);
            frame(
                &mut lane,
                vec![
                    egui::Event::PointerMoved(pos),
                    event(pos, egui::PointerButton::Primary, true),
                ],
            );
            frame(&mut lane, vec![egui::Event::PointerMoved(end)]);
            frame(
                &mut lane,
                vec![event(end, egui::PointerButton::Primary, false)],
            );
        }
        assert_eq!(lane.pitch, 24);
        assert_eq!(lane.send, 1.0);
        let before = lane.clone();
        let pos = rects[1].center();
        frame(
            &mut lane,
            vec![
                egui::Event::PointerMoved(pos),
                event(pos, egui::PointerButton::Secondary, true),
            ],
        );
        frame(
            &mut lane,
            vec![event(pos, egui::PointerButton::Secondary, false)],
        );
        assert_eq!(lane, before);
    }

    #[test]
    fn delay_steps_and_mode_controls_emit_the_real_delay_messages() {
        let ctx = egui::Context::default();
        let mut panel = crate::model::Panel::new();
        panel.events.clear();
        let frame = |panel: &mut crate::model::Panel, events| {
            let mut rects = [Rect::NOTHING; 3];
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(300.0, 200.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    rects = delay_timing(ui, panel);
                },
            );
            out.textures_delta.clear();
            rects
        };
        let rects = frame(&mut panel, vec![]);
        let event = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        for (index, value) in [(1, 1), (2, 0)] {
            let pos = rects[index].center();
            frame(
                &mut panel,
                vec![egui::Event::PointerMoved(pos), event(pos, true)],
            );
            frame(&mut panel, vec![event(pos, false)]);
            assert_eq!(panel.switch("delay.triplet"), value);
        }
        let start = rects[0].center();
        let end = start + Vec2::new(0.0, -20.0);
        frame(
            &mut panel,
            vec![egui::Event::PointerMoved(start), event(start, true)],
        );
        frame(&mut panel, vec![egui::Event::PointerMoved(end)]);
        frame(&mut panel, vec![event(end, false)]);
        let steps = panel.display("delay.steps").unwrap();
        assert!(steps > 3 && steps <= 32);
        let (mut link, _) =
            crate::link::Link::new(std::sync::Arc::new(orb_engine::tables::Tables::new()), None);
        let messages: Vec<_> = panel.events.iter().flat_map(|e| link.messages(e)).collect();
        for (ctrl, value) in [(1, steps as u8), (2, 1), (2, 0)] {
            assert!(messages.contains(&crate::link::Msg::Device {
                dev: orb_engine::song::Device::Delay.id(),
                sub: 84,
                data: vec![ctrl, value],
            }));
        }
    }

    #[test]
    fn sampler_mix_fader_reaches_both_endpoints_and_right_click_does_not_move_it() {
        let ctx = egui::Context::default();
        let mut app = App::new(crate::model::Panel::new(), None);
        let x_scale = crate::graphite_rack::WIDTH / 1280.0;
        let rect = Rect::from_min_size(egui::pos2(40.0, 40.0), Vec2::new(198.0 * x_scale, 320.0));
        let frame = |app: &mut App, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(600.0, 450.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.graphite_sampler_mixer(ui, rect),
            );
            output.textures_delta.clear();
        };
        frame(&mut app, vec![]);
        for (y, expected) in [(36.0, 2.0), (79.0, 0.0)] {
            let position = rect.left_top() + Vec2::new(22.0 * x_scale, y);
            frame(&mut app, vec![egui::Event::PointerMoved(position)]);
            for pressed in [true, false] {
                frame(
                    &mut app,
                    vec![egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    }],
                );
            }
            assert_eq!(app.studio.settings.sampler_bus.gain, expected);
        }
        let position = rect.left_top() + Vec2::new(22.0 * x_scale, 40.0);
        for pressed in [true, false] {
            frame(
                &mut app,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        assert_eq!(app.studio.settings.sampler_bus.gain, 0.0);
    }

    #[test]
    fn solo_and_mute_are_independent_clicks_next_to_the_sample_name() {
        let ctx = egui::Context::default();
        let mut lane = Lane::default();
        let mut selected = 0;
        let mut regions = [Rect::NOTHING; 3];
        let mut draw = |events: Vec<egui::Event>| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(300.0, 100.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.set_width(240.0);
                    regions = lane_row(
                        ui,
                        4,
                        "A very long sample name to truncate.wav",
                        LaneRowOptions {
                            can_unload: true,
                            active: false,
                        },
                        &mut lane,
                        &mut selected,
                        240.0,
                    );
                },
            );
            output.textures_delta.clear();
            regions
        };
        let regions = draw(Vec::new());
        assert!(regions[0].right() < regions[1].left());
        assert!(regions[1].right() < regions[2].left());
        for index in [1, 2] {
            let pos = regions[index].center();
            for pressed in [true, false] {
                draw(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ]);
            }
        }
        assert!(lane.solo && lane.muted);
        assert_eq!(
            selected, 0,
            "Quick mix actions must not change the editing lane"
        );
    }

    #[test]
    fn quick_mix_columns_stay_aligned_across_all_eight_sample_names() {
        let ctx = egui::Context::default();
        let mut selected = 0;
        let mut lanes: [Lane; 8] = std::array::from_fn(|_| Lane::default());
        let mut columns = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, Vec2::splat(400.0))),
                ..Default::default()
            },
            |ui| {
                ui.set_width(240.0);
                for (index, lane) in lanes.iter_mut().enumerate() {
                    columns.push(lane_row(
                        ui,
                        index,
                        "A long imported sample filename.wav",
                        LaneRowOptions {
                            can_unload: true,
                            active: false,
                        },
                        lane,
                        &mut selected,
                        240.0,
                    ));
                }
            },
        );
        output.textures_delta.clear();
        for row in &columns {
            assert_eq!(row[1].left(), columns[0][1].left());
            assert_eq!(row[2].left(), columns[0][2].left());
            assert!(row[2].right() <= row[0].left() + 240.1);
        }
    }

    #[test]
    fn cards_keep_the_scroll_viewport_clip() {
        let ctx = egui::Context::default();
        let viewport = Rect::from_min_size(egui::pos2(20.0, 20.0), Vec2::splat(60.0));
        let outer = Rect::from_min_size(egui::Pos2::ZERO, Vec2::splat(100.0));
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(outer),
                ..Default::default()
            },
            |ui| {
                ui.set_clip_rect(viewport);
                surface(ui, outer, "clipped-card", |ui| {
                    assert_eq!(ui.clip_rect(), viewport);
                });
            },
        );
        output.textures_delta.clear();
    }
}
