//! Dark, compact rack launchers and spacious instrument/effect editors.

use eframe::egui::{self, Align2, Color32, Context, FontId, Sense, Stroke, Ui, Vec2};
use orb_engine::studio::sampler::Mode;
use orb_player::Command;

use super::IR_NAMES;
use crate::app::{App, View};
use crate::midi::MidiMenu;

const ACCENT: Color32 = Color32::from_rgb(190, 215, 126);
const CARD: Color32 = Color32::from_rgb(30, 31, 34);
const DIM: Color32 = Color32::from_rgb(145, 148, 151);

fn style(ui: &mut Ui) {
    ui.visuals_mut().selection.bg_fill = Color32::from_rgb(69, 80, 48);
    ui.visuals_mut().selection.stroke = Stroke::new(1.0, ACCENT);
}

fn slider(ui: &mut Ui, value: &mut f64, lo: f64, hi: f64, label: &str, id: &str) {
    ui.add(
        crate::midi::slider(ui, value, lo..=hi)
            .logarithmic(lo > 0.0 && hi / lo >= 50.0)
            .text(label),
    )
    .midi_menu(ui, id);
}

impl App {
    pub(crate) fn studio_rack(&mut self, ui: &mut Ui, view: &View) {
        if !self.studio.modern {
            return;
        }
        let active = !self.browsing() && !self.prompting() && !self.error_open();
        for (i, (name, subtitle)) in [
            ("PCF", "PATTERN FILTER"),
            ("DELAY", "TEMPO ECHO"),
            ("DIST", "SATURATION"),
            ("COMP", "DYNAMICS"),
            ("SPACE", "CONVOLUTION"),
            ("SAMPLER", "8 LANES / 64 BARS"),
        ]
        .into_iter()
        .enumerate()
        {
            let rect = view.rect(528.0, 150.0 + i as f32 * 59.0, 78.0, 55.0);
            let response = ui.interact(rect, egui::Id::unique(("studio-card", i)), Sense::click());
            ui.painter().rect_filled(
                rect,
                3.0 * view.scale,
                if response.hovered() {
                    Color32::from_gray(47)
                } else {
                    CARD
                },
            );
            ui.painter().line_segment(
                [rect.left_top(), rect.right_top()],
                Stroke::new(
                    view.scale,
                    if i == 4 {
                        ACCENT
                    } else {
                        Color32::from_gray(67)
                    },
                ),
            );
            ui.painter().text(
                rect.center_top() + Vec2::new(0.0, 17.0 * view.scale),
                Align2::CENTER_CENTER,
                name,
                FontId::proportional(10.0 * view.scale),
                Color32::WHITE,
            );
            ui.painter().text(
                rect.center_top() + Vec2::new(0.0, 33.0 * view.scale),
                Align2::CENTER_CENTER,
                subtitle,
                FontId::proportional(5.3 * view.scale),
                DIM,
            );
            if active && response.clicked() {
                match i {
                    4 => self.studio.reverb_open = true,
                    5 => self.studio.sampler_open = true,
                    _ => self.studio.fx = Some(name.to_ascii_lowercase()),
                }
            }
        }
        for row in 0..4 {
            let rect = view.rect(492.0, 150.0 + row as f32 * 115.0, 30.0, 11.0);
            let response =
                ui.interact(rect, egui::Id::unique(("studio-send", row)), Sense::click());
            let on = self.studio.settings.sends[row] > 0.0;
            ui.painter()
                .rect_filled(rect, 2.0 * view.scale, if on { ACCENT } else { CARD });
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "REV",
                FontId::proportional(6.0 * view.scale),
                if on { Color32::BLACK } else { DIM },
            );
            if active
                && response
                    .on_hover_text("Reverb send. Open SPACE to set the amount.")
                    .clicked()
            {
                self.studio.settings.sends[row] = if on { 0.0 } else { 0.3 };
                self.studio.changed();
                self.doc.modified = true;
            }
        }
    }

    pub(crate) fn studio_ui(&mut self, ctx: &Context) {
        self.sampler_history_keys(ctx);
        if self.studio.reverb_open {
            self.reverb_editor(ctx);
        }
        if self.studio.sampler_open {
            self.sampler_editor(ctx);
        }
        self.warp_editor(ctx);
        self.legacy_fx_editor(ctx);
        self.studio_mirror();
        self.studio_poll(ctx);
        if self.studio.worker.is_some() && !self.studio.modern {
            egui::Window::new("Preparing Studio")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Preparing audio in the background...");
                    });
                });
        }
        if let Some(notice) = self.studio.notice.clone() {
            egui::Window::new("Studio message")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(notice);
                    if ui.button("OK").clicked() {
                        self.studio.notice = None;
                    }
                });
        }
    }

    fn reverb_editor(&mut self, ctx: &Context) {
        let before = self.studio.settings.clone();
        let mut open = true;
        let busy = self.studio.worker.is_some() || !self.studio.ready;
        let mut build = None;
        egui::Window::new("SPACE / Convolution Reverb")
            .open(&mut open)
            .default_width(620.0)
            .resizable(true)
            .show(ctx, |ui| {
                style(ui);
                ui.spacing_mut().item_spacing = Vec2::new(12.0, 10.0);
                ui.horizontal(|ui| {
                    ui.heading(egui::RichText::new("SPACE").color(ACCENT));
                    ui.label("STEREO CONVOLUTION");
                    ui.checkbox(&mut self.studio.settings.enabled, "On");
                });
                ui.label(&self.studio.ir_name);
                if let Some(ir) = &self.studio.ir {
                    waveform(
                        ui,
                        &ir.frames,
                        70.0,
                        0.0,
                        1.0,
                        self.studio.waveform_epoch,
                        &mut self.studio.wave_views[8],
                    );
                }
                ui.add_enabled_ui(!busy, |ui| {
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("space-preset")
                            .selected_text(IR_NAMES[self.studio.preset])
                            .show_ui(ui, |ui| {
                                for (i, name) in IR_NAMES.iter().enumerate() {
                                    ui.selectable_value(&mut self.studio.preset, i, *name);
                                }
                            });
                        ui.add(
                            crate::midi::drag_value(ui, &mut self.studio.decay)
                                .range(0.1..=6.0)
                                .speed(0.05)
                                .suffix(" s"),
                        );
                        if ui.button("Build space").clicked() {
                            build = Some(false);
                        }
                        if ui.button("Load IR...").clicked() {
                            build = Some(true);
                        }
                    });
                });
                ui.small(
                    "Built-in spaces use original synthetic impulse responses. \
            Import a room/plate IR for its sound.",
                );
                ui.separator();
                ui.columns(2, |columns| {
                    let s = &mut self.studio.settings;
                    slider(
                        &mut columns[0],
                        &mut s.return_gain,
                        0.0,
                        1.0,
                        "Return",
                        "studio.return",
                    );
                    slider(
                        &mut columns[0],
                        &mut s.predelay_ms,
                        0.0,
                        200.0,
                        "Pre-delay / ms",
                        "studio.predelay",
                    );
                    slider(
                        &mut columns[0],
                        &mut s.width,
                        0.0,
                        2.0,
                        "Stereo width",
                        "studio.width",
                    );
                    slider(
                        &mut columns[1],
                        &mut s.low_cut,
                        20.0,
                        2000.0,
                        "Low cut / Hz",
                        "studio.low_cut",
                    );
                    slider(
                        &mut columns[1],
                        &mut s.high_cut,
                        2000.0,
                        20000.0,
                        "High cut / Hz",
                        "studio.high_cut",
                    );
                });
                ui.separator();
                ui.label("POST-FADER SENDS");
                for (i, name) in ["TB-303 A", "TB-303 B", "TR-808", "TR-909"]
                    .iter()
                    .enumerate()
                {
                    slider(
                        ui,
                        &mut self.studio.settings.sends[i],
                        0.0,
                        1.0,
                        name,
                        &format!("studio.send{}", i + 1),
                    );
                }
                slider(
                    ui,
                    &mut self.studio.settings.sampler_bus.reverb_send,
                    0.0,
                    1.0,
                    "Sampler / global send",
                    "sampler.send",
                );
                ui.small(
                    "Sampler lanes have individual sends. Turning a send down \
            leaves the reverb tail.",
                );
            });
        self.studio.reverb_open = open;
        if self.studio.settings != before {
            self.studio.changed();
            self.doc.modified = true;
        }
        if let Some(imported) = build {
            self.studio_build_ir(ctx, imported);
        }
    }

    fn sampler_editor(&mut self, ctx: &Context) {
        let before = self.studio.settings.clone();
        let mut open = true;
        let mut load = None;
        let mut audition = None;
        let mut unload = None;
        let tick = self.panel.transport.position.max(0) as usize;
        let playing = self.panel.transport.playing;
        let busy = self.studio.worker.is_some() || !self.studio.ready;
        egui::Window::new("SAMPLER / Eight-lane sequencer")
            .open(&mut open)
            .default_size(Vec2::new(980.0, 680.0))
            .min_width(720.0)
            .vscroll(true)
            .show(ctx, |ui| {
                if self.studio.history_restoring {
                    ui.disable();
                }
                style(ui);
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                ui.horizontal(|ui| {
                    ui.heading(egui::RichText::new("SAMPLER").color(ACCENT));
                    ui.label("8 STEREO LANES / 64 BARS");
                    self.sampler_history_buttons(ui);
                    if ui.button(if playing { "Stop" } else { "Play" }).clicked() {
                        self.panel.press(
                            if playing {
                                "transport.stop"
                            } else {
                                "transport.play"
                            },
                            0.0,
                        );
                    }
                    ui.checkbox(&mut self.studio.follow, "Follow");
                });
                let selected = self.studio.selected;
                let bars = usize::from(self.studio.settings.lanes[selected].bars);
                if self.studio.follow && playing {
                    self.studio.bar = tick / 768 % bars;
                }
                self.studio.bar = self.studio.bar.min(bars - 1);
                let bar = self.studio.bar;
                ui.separator();
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    egui::Grid::new("sampler-lanes")
                        .min_col_width(0.0)
                        .spacing(Vec2::new(7.0, 7.0))
                        .show(ui, |ui| {
                            ui.label("LANE / SAMPLE");
                            ui.label("M");
                            ui.label("S");
                            ui.label("AUDIO");
                            for step in 0..16 {
                                ui.label(format!("{:02}", step + 1));
                            }
                            ui.end_row();
                            for i in 0..8 {
                                let active = i == selected;
                                let label = format!("{:02}  {}", i + 1, self.studio.names[i]);
                                let name_response = ui.add_sized(
                                    [190.0, 26.0],
                                    egui::Button::new(label).selected(active),
                                );
                                if name_response.clicked() {
                                    self.studio.selected = i;
                                }
                                name_response.context_menu(|ui| {
                                    if ui
                                        .add_enabled(
                                            !busy && self.studio.waves[i].is_some(),
                                            egui::Button::new("Unload sample"),
                                        )
                                        .clicked()
                                    {
                                        unload = Some(i);
                                        ui.close();
                                    }
                                });
                                let lane = &mut self.studio.settings.lanes[i];
                                ui.checkbox(&mut lane.muted, "")
                                    .midi_menu(ui, &format!("sampler{}.mute", i + 1));
                                ui.checkbox(&mut lane.solo, "")
                                    .midi_menu(ui, &format!("sampler{}.solo", i + 1));
                                if ui.add_enabled(!busy, egui::Button::new("Load")).clicked() {
                                    load = Some(i);
                                }
                                for step in 0..16 {
                                    let index = bar * 16 + step;
                                    let available = bar < usize::from(lane.bars);
                                    let value = lane.steps[index];
                                    let head = playing
                                        && tick / 48 % (usize::from(lane.bars) * 16) == index;
                                    let mut button = egui::Button::new(if value == 0 {
                                        "".into()
                                    } else {
                                        format!("{value}")
                                    })
                                    .fill(if value > 0 {
                                        Color32::from_rgb(84, 99, 56)
                                    } else if step % 4 == 0 {
                                        Color32::from_gray(49)
                                    } else {
                                        CARD
                                    });
                                    if head {
                                        button = button.stroke(Stroke::new(2.0, ACCENT));
                                    }
                                    let response = ui.add_enabled(
                                        available,
                                        button.min_size(Vec2::new(25.0, 26.0)),
                                    );
                                    if response.clicked() {
                                        lane.steps[index] = if value > 0 { 0 } else { 100 };
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
                                ui.end_row();
                            }
                        });
                });
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.label("BAR");
                    let lane = &mut self.studio.settings.lanes[self.studio.selected];
                    for b in 0..usize::from(lane.bars) {
                        let contains = lane.steps[b * 16..b * 16 + 16].iter().any(|&v| v > 0);
                        if ui
                            .selectable_label(
                                self.studio.bar == b,
                                format!("{}{}", b + 1, if contains { " •" } else { "" }),
                            )
                            .clicked()
                        {
                            self.studio.bar = b;
                            self.studio.follow = false;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    let lane = &mut self.studio.settings.lanes[self.studio.selected];
                    ui.add(
                        crate::midi::drag_value(ui, &mut lane.bars)
                            .range(1..=64)
                            .prefix("Length ")
                            .suffix(" bars"),
                    );
                    if ui.button("Copy bar").clicked() {
                        self.studio.clipboard =
                            Some(lane.steps[bar * 16..bar * 16 + 16].try_into().unwrap());
                    }
                    if ui
                        .add_enabled(
                            self.studio.clipboard.is_some(),
                            egui::Button::new("Paste bar"),
                        )
                        .clicked()
                        && let Some(copy) = self.studio.clipboard
                    {
                        lane.steps[bar * 16..bar * 16 + 16].copy_from_slice(&copy);
                    }
                    if ui.button("Clear bar").clicked() {
                        lane.steps[bar * 16..bar * 16 + 16].fill(0);
                    }
                    if ui.button("Repeat bar to length").clicked() {
                        let copy: [u8; 16] =
                            lane.steps[bar * 16..bar * 16 + 16].try_into().unwrap();
                        for b in 0..usize::from(lane.bars) {
                            lane.steps[b * 16..b * 16 + 16].copy_from_slice(&copy);
                        }
                    }
                });
                ui.separator();
                let selected = self.studio.selected;
                ui.horizontal(|ui| {
                    ui.strong(format!(
                        "{:02} / {}",
                        selected + 1,
                        self.studio.names[selected]
                    ));
                    if ui
                        .add_enabled(
                            self.studio.waves[selected].is_some() && !busy,
                            egui::Button::new("Audition"),
                        )
                        .clicked()
                    {
                        audition = Some(selected);
                    }
                    if ui
                        .add_enabled(
                            self.studio.waves[selected].is_some() && !busy,
                            egui::Button::new("Remove sample"),
                        )
                        .clicked()
                    {
                        unload = Some(selected);
                    }
                });
                let lane = &mut self.studio.settings.lanes[selected];
                if let Some(wave) = &self.studio.waves[selected] {
                    let response = waveform(
                        ui,
                        &wave.frames,
                        100.0,
                        lane.start,
                        lane.end,
                        self.studio.waveform_epoch,
                        &mut self.studio.wave_views[selected],
                    );
                    if let Some(head) = self.panel.sampler_playheads[selected] {
                        draw_playhead(ui, response.rect, head.source);
                    }
                    let minimum = 2.0 / wave.frames.len().max(2) as f64;
                    if response.double_clicked() {
                        lane.start = 0.0;
                        lane.end = 1.0;
                    } else if let Some(pointer) = response.interact_pointer_pos() {
                        let at =
                            f64::from((pointer.x - response.rect.left()) / response.rect.width())
                                .clamp(0.0, 1.0);
                        let id = response.id.with("trim-handle");
                        if response.drag_started() || response.clicked() {
                            let entry = (at - lane.start).abs() <= (at - lane.end).abs();
                            ui.data_mut(|d| d.insert_temp(id, entry));
                        }
                        if response.dragged_by(egui::PointerButton::Primary) || response.clicked() {
                            let entry = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(true);
                            trim_at(lane, entry, at, minimum);
                        }
                    }
                    response.on_hover_text(
                        "Drag IN or OUT. Click to move the nearest marker. \
                         Double-click to select all.",
                    );
                    let duration = wave.frames.len() as f64 / 44100.0;
                    let mut entry = lane.start * duration;
                    let mut exit = lane.end * duration;
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                crate::midi::drag_value(ui, &mut entry)
                                    .range(0.0..=duration)
                                    .speed(0.001)
                                    .fixed_decimals(3)
                                    .prefix("IN ")
                                    .suffix(" s"),
                            )
                            .changed()
                        {
                            trim_at(lane, true, entry / duration, minimum);
                        }
                        if ui
                            .add(
                                crate::midi::drag_value(ui, &mut exit)
                                    .range(0.0..=duration)
                                    .speed(0.001)
                                    .fixed_decimals(3)
                                    .prefix("OUT ")
                                    .suffix(" s"),
                            )
                            .changed()
                        {
                            trim_at(lane, false, exit / duration, minimum);
                        }
                    });
                    ui.small(format!(
                        "{:.2} seconds / stereo / 44.1 kHz",
                        wave.frames.len() as f64 / 44100.0
                    ));
                } else {
                    ui.label(
                        "Load WAV, AIFF or MP3. Click steps to sequence; \
                         right-click for velocity.",
                    );
                }
                ui.horizontal(|ui| {
                    for (mode, label) in [
                        (Mode::OneShot, "One Shot"),
                        (Mode::AutoCut, "Auto Cut"),
                        (Mode::Gate, "Gate"),
                        (Mode::Loop, "Loop"),
                    ] {
                        ui.selectable_value(&mut lane.mode, mode, label)
                            .midi_menu(ui, &format!("sampler{}.mode", selected + 1));
                    }
                    ui.checkbox(&mut lane.reverse, "Reverse")
                        .midi_menu(ui, &format!("sampler{}.reverse", selected + 1));
                    ui.add(
                        crate::midi::drag_value(ui, &mut lane.choke)
                            .range(0..=8)
                            .prefix("Choke "),
                    )
                    .midi_menu(ui, &format!("sampler{}.choke", selected + 1));
                });
                ui.small(match lane.mode {
                    Mode::OneShot => {
                        "One Shot: plays IN to OUT across pattern wraps. Four overlaps; \
                         excess triggers are ignored. Stop ends all voices."
                    }
                    Mode::AutoCut => {
                        "Auto Cut: each new trigger cuts the previous voice with a short fade."
                    }
                    Mode::Gate => "Gate: releases after the selected gate duration.",
                    Mode::Loop => {
                        "Loop: repeats the selected region until retriggered, choked \
            or transport stops."
                    }
                });
                ui.columns(3, |columns| {
                    slider(
                        &mut columns[0],
                        &mut lane.gain,
                        0.0,
                        2.0,
                        "Level",
                        &format!("sampler{}.{}", selected + 1, "level"),
                    );
                    slider(
                        &mut columns[0],
                        &mut lane.pan,
                        -1.0,
                        1.0,
                        "Pan",
                        &format!("sampler{}.{}", selected + 1, "pan"),
                    );
                    slider(
                        &mut columns[0],
                        &mut lane.send,
                        0.0,
                        1.0,
                        "Reverb send",
                        &format!("sampler{}.{}", selected + 1, "send"),
                    );
                    columns[1]
                        .add(
                            crate::midi::slider(&columns[1], &mut lane.pitch, -24..=24)
                                .text("Pitch / st"),
                        )
                        .midi_menu(&mut columns[1], &format!("sampler{}.pitch", selected + 1));
                    slider(
                        &mut columns[1],
                        &mut lane.attack_ms,
                        0.1,
                        2000.0,
                        "Attack / ms",
                        &format!("sampler{}.{}", selected + 1, "attack"),
                    );
                    slider(
                        &mut columns[1],
                        &mut lane.release_ms,
                        1.0,
                        3000.0,
                        "Release / ms",
                        &format!("sampler{}.{}", selected + 1, "release"),
                    );
                    slider(
                        &mut columns[2],
                        &mut lane.start,
                        0.0,
                        1.0 - 1e-9,
                        "Start",
                        &format!("sampler{}.{}", selected + 1, "start"),
                    );
                    slider(
                        &mut columns[2],
                        &mut lane.end,
                        lane.start + 1e-9,
                        1.0,
                        "End",
                        &format!("sampler{}.{}", selected + 1, "end"),
                    );
                    slider(
                        &mut columns[2],
                        &mut lane.gate,
                        0.01,
                        16.0,
                        "Gate / steps",
                        &format!("sampler{}.{}", selected + 1, "gate"),
                    );
                });
                ui.separator();
                let frames = self.studio.waves[selected]
                    .as_ref()
                    .map(|wave| wave.frames.len());
                if super::rack::warp_controls(ui, lane, frames, &format!("sampler{}", selected + 1))
                {
                    self.studio.warp_editor = Some(selected);
                }
                super::rack::gater_controls(ui, lane, &format!("sampler{}", selected + 1));
                ui.columns(3, |columns| {
                    slider(
                        &mut columns[0],
                        &mut lane.gater_depth,
                        0.0,
                        1.0,
                        "Gater depth",
                        &format!("sampler{}.{}", selected + 1, "gater_depth"),
                    );
                    slider(
                        &mut columns[1],
                        &mut lane.gater_attack_ms,
                        0.1,
                        100.0,
                        "Open / ms",
                        &format!("sampler{}.{}", selected + 1, "gater_attack_ms"),
                    );
                    slider(
                        &mut columns[2],
                        &mut lane.gater_release_ms,
                        1.0,
                        500.0,
                        "Close / ms",
                        &format!("sampler{}.{}", selected + 1, "gater_release_ms"),
                    );
                });
                ui.small(
                    "Gater follows the selected TR instrument, even with its mixer volume down. \
                     Sidechain follows the global sidechain \
                     On, TR source, voice and release (65% depth).",
                );
                ui.small(
                    "Pitch changes playback speed. Choke 0 is independent; lanes \
            sharing 1–8 cut each other.",
                );
                ui.small(
                    "File > Save stores samples, IR and all 64 bars together in \
            a portable .orb338 project.",
                );
            });
        self.studio.sampler_open = open;
        if self.studio.settings != before {
            self.studio.changed();
            self.doc.modified = true;
        }
        if let Some(lane) = load {
            self.studio_load_wave(ctx, lane);
        }
        if let Some(lane) = audition {
            self.studio_command(Command::StudioAudition(lane));
        }
        if let Some(lane) = unload {
            self.sampler_unload(lane);
        }
        if playing {
            ctx.request_repaint_after(crate::app::METER_FRAME);
        }
    }

    fn legacy_fx_editor(&mut self, ctx: &Context) {
        let Some(section) = self.studio.fx.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new(format!("{} / Effect", section.to_uppercase()))
            .open(&mut open)
            .default_width(430.0)
            .show(ctx, |ui| {
                if section == "delay" {
                    super::rack::delay_timing(ui, &mut self.panel);
                }
                for control in crate::layout::controls().into_iter().filter(|c| {
                    c.section == section
                        && !(section == "delay"
                            && (c.id.starts_with("delay.steps") || c.id == "delay.triplet"))
                }) {
                    let label = control.name().replace('_', " ");
                    match control.kind.as_str() {
                        "knob" => {
                            let mut value = self.panel.knob(&control.id);
                            if ui
                                .add(crate::midi::slider(ui, &mut value, 0..=1000).text(label))
                                .midi_menu(ui, &control.id)
                                .changed()
                            {
                                self.panel.set_knob(&control.id, value);
                            }
                        }
                        "fader" => {
                            let mut value = self.panel.fader(&control.id);
                            if ui
                                .add(crate::midi::slider(ui, &mut value, 0..=999).text(label))
                                .midi_menu(ui, &control.id)
                                .changed()
                            {
                                self.panel.set_fader(&control.id, value);
                            }
                        }
                        "vswitch" | "hswitch" => {
                            let mut value = self.panel.switch(&control.id);
                            ui.horizontal(|ui| {
                                ui.label(&label);
                                let options = if control.id == "pcf.mode" {
                                    [(0, "Low-pass"), (1, "Band-pass")]
                                } else {
                                    [(0, "Triplet"), (1, "Straight")]
                                };
                                for (v, text) in options {
                                    if ui
                                        .selectable_value(&mut value, v, text)
                                        .midi_menu(ui, &control.id)
                                        .changed()
                                    {
                                        self.panel.set_switch(&control.id, value);
                                    }
                                }
                            });
                        }
                        "display" => {
                            ui.horizontal(|ui| {
                                ui.label(format!(
                                    "{}: {}",
                                    label,
                                    self.panel.display(&control.id).unwrap_or(0)
                                ));
                                if ui.button("−").clicked() {
                                    self.panel.arrow(&format!("{}_down", control.id));
                                }
                                if ui.button("+").clicked() {
                                    self.panel.arrow(&format!("{}_up", control.id));
                                }
                            });
                        }
                        "toggle" => {
                            let mut on = self.panel.toggle_on(&control.id);
                            if ui
                                .checkbox(&mut on, label)
                                .midi_menu(ui, &control.id)
                                .changed()
                            {
                                self.panel.press(&control.id, 0.0);
                            }
                        }
                        _ => {}
                    }
                }
                ui.small(
                    "Route this effect using the machine's mixer buttons. \
            Existing MIDI assignments stay active.",
                );
            });
        if !open {
            self.studio.fx = None;
        }
    }
}

pub(super) fn draw_playhead(ui: &Ui, rect: egui::Rect, position: f32) {
    if !position.is_finite() {
        return;
    }
    let x = rect.left() + position.clamp(0.0, 1.0) * rect.width();
    ui.painter().line_segment(
        [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
        Stroke::new(2.0, Color32::WHITE),
    );
    ui.painter()
        .circle_filled(egui::pos2(x, rect.top() + 3.0), 3.0, Color32::WHITE);
}

pub(super) fn waveform(
    ui: &mut Ui,
    frames: &[[f32; 2]],
    height: f32,
    start: f64,
    end: f64,
    epoch: u64,
    cache: &mut super::WaveformCache,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        Sense::click_and_drag(),
    );
    ui.painter().rect_filled(rect, 4.0, Color32::from_gray(17));
    if frames.is_empty() {
        return response;
    }
    let peaks = cache.get(epoch, frames);
    let columns = rect.width().floor().max(1.0) as usize;
    for x in 0..columns {
        let a = x * peaks.len() / columns;
        let b = ((x + 1) * peaks.len() / columns)
            .max(a + 1)
            .min(peaks.len());
        let peak = peaks[a.min(peaks.len())..b]
            .iter()
            .copied()
            .fold(0.0f32, f32::max)
            .min(1.0);
        let px = rect.left() + x as f32;
        let selected = x as f64 / columns as f64 >= start && x as f64 / columns as f64 <= end;
        ui.painter().line_segment(
            [
                egui::pos2(px, rect.center().y - peak * height * 0.45),
                egui::pos2(px, rect.center().y + peak * height * 0.45),
            ],
            Stroke::new(1.0, if selected { ACCENT } else { DIM }),
        );
    }
    for (value, label) in [(start, "IN"), (end, "OUT")] {
        let x = (rect.left() + value as f32 * rect.width())
            .clamp(rect.left() + 1.0, rect.right() - 1.0);
        ui.painter().line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            Stroke::new(2.0, ACCENT),
        );
        let align = if label == "IN" {
            egui::Align2::LEFT_TOP
        } else {
            egui::Align2::RIGHT_TOP
        };
        ui.painter().text(
            egui::pos2(x, rect.top() + 3.0),
            align,
            label,
            egui::FontId::monospace(11.0),
            ACCENT,
        );
    }
    response
}

pub(super) fn trim_at(
    lane: &mut orb_engine::studio::sampler::Lane,
    entry: bool,
    at: f64,
    minimum: f64,
) {
    if entry {
        lane.start = at.clamp(0.0, (lane.end - minimum).max(0.0));
    } else {
        lane.end = at.clamp((lane.start + minimum).min(1.0), 1.0);
    }
}

#[cfg(test)]
mod trim_tests {
    use super::*;

    #[test]
    fn markers_do_not_cross_and_long_samples_allow_precise_regions() {
        let mut lane = orb_engine::studio::sampler::Lane::default();
        let minimum = 2.0 / (600.0 * 44100.0);
        trim_at(&mut lane, true, 0.9, minimum);
        trim_at(&mut lane, false, 0.0, minimum);
        assert!((lane.end - lane.start - minimum).abs() < 1e-12);
        trim_at(&mut lane, true, 1.0, minimum);
        assert!(lane.start < lane.end);
        trim_at(&mut lane, false, 1.5, minimum);
        assert_eq!(lane.end, 1.0);
    }
}
