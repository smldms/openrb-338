//! Native, spacious Graphite rack. Controls retain the existing Panel event routes.

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::app::App;
use crate::layout;
use crate::midi::MidiMenu;

pub const WIDTH: f32 = 1760.0;
pub const HEIGHT: f32 = 1300.0;
const ACID: Color32 = Color32::from_rgb(190, 215, 126);
const INK: Color32 = Color32::from_rgb(232, 234, 231);
const DIM: Color32 = Color32::from_rgb(133, 138, 136);
const FACE: Color32 = Color32::from_rgb(29, 31, 32);
const BORDER: Color32 = Color32::from_rgb(54, 58, 57);
const ROWS: [(f32, f32); 4] = [
    (84.0, 168.0),
    (262.0, 168.0),
    (440.0, 250.0),
    (700.0, 250.0),
];
const MACHINES: [&str; 4] = ["TB-303 A", "TB-303 B", "TR-808", "TR-909"];
const NOTES: [&str; 13] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B", "C+",
];

type DrumParameter = (&'static str, &'static str);

// These are the actual controls of each engine voice, not a common synthetic channel strip.
// Hats use the separate closed-hat level supported by the controller router.
const VOICES_808: [&[DrumParameter]; 12] = [
    &[("ac_level", "LEVEL")],
    &[
        ("bd_level", "LEVEL"),
        ("bd_tone", "TONE"),
        ("bd_decay", "DECAY"),
    ],
    &[
        ("sd_level", "LEVEL"),
        ("sd_tone", "TONE"),
        ("sd_snappy", "SNAPPY"),
    ],
    &[("lt_level", "LEVEL"), ("lt_tuning", "TUNE")],
    &[("mt_level", "LEVEL"), ("mt_tuning", "TUNE")],
    &[("ht_level", "LEVEL"), ("ht_tuning", "TUNE")],
    &[("rs_level", "LEVEL")],
    &[("cp_level", "LEVEL")],
    &[("cb_level", "LEVEL")],
    &[
        ("cy_level", "LEVEL"),
        ("cy_tone", "TONE"),
        ("cy_decay", "DECAY"),
    ],
    &[("oh_level", "LEVEL"), ("oh_decay", "DECAY")],
    &[("ch_level", "LEVEL")],
];

const VOICES_909: [&[DrumParameter]; 12] = [
    &[("ac_level", "LEVEL"), ("flam", "FLAM")],
    &[
        ("bd_level", "LEVEL"),
        ("bd_tune", "TUNE"),
        ("bd_attack", "ATTACK"),
        ("bd_decay", "DECAY"),
    ],
    &[
        ("sd_level", "LEVEL"),
        ("sd_tune", "TUNE"),
        ("sd_tone", "TONE"),
        ("sd_snappy", "SNAPPY"),
    ],
    &[
        ("lt_level", "LEVEL"),
        ("lt_tune", "TUNE"),
        ("lt_decay", "DECAY"),
    ],
    &[
        ("mt_level", "LEVEL"),
        ("mt_tune", "TUNE"),
        ("mt_decay", "DECAY"),
    ],
    &[
        ("ht_level", "LEVEL"),
        ("ht_tune", "TUNE"),
        ("ht_decay", "DECAY"),
    ],
    &[("rs_level", "LEVEL")],
    &[("cp_level", "LEVEL")],
    &[("ch_level", "LEVEL"), ("ch_decay", "DECAY")],
    &[("hh_level", "LEVEL"), ("oh_decay", "DECAY")],
    &[("cc_level", "LEVEL"), ("cc_tune", "TUNE")],
    &[("rc_level", "LEVEL"), ("rc_tune", "TUNE")],
];

#[derive(Clone, Copy)]
struct Rack {
    origin: Pos2,
    scale: f32,
}

impl Rack {
    fn fit(avail: Rect) -> Self {
        let scale = (avail.width() / WIDTH)
            .min(avail.height() / HEIGHT)
            .max(0.1);
        Self {
            origin: avail.center() - Vec2::new(WIDTH, HEIGHT) * scale * 0.5,
            scale,
        }
    }

    fn rect(self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(
            self.origin + Vec2::new(x * WIDTH / 1280.0, y) * self.scale,
            Vec2::new(w * WIDTH / 1280.0, h) * self.scale,
        )
    }

    fn text(self, ui: &Ui, x: f32, y: f32, size: f32, text: &str, color: Color32) {
        ui.painter().text(
            self.origin + Vec2::new(x * WIDTH / 1280.0, y) * self.scale,
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(size * self.scale * 1.2),
            color,
        );
    }

    fn card(self, ui: &Ui, rect: Rect) {
        ui.painter().rect_filled(rect, 6.0 * self.scale, FACE);
        ui.painter().rect_stroke(
            rect,
            6.0 * self.scale,
            Stroke::new(self.scale, BORDER),
            egui::StrokeKind::Inside,
        );
    }
}

fn button(
    ui: &mut Ui,
    rack: Rack,
    rect: Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
    text: &str,
    selected: bool,
) -> egui::Response {
    let response = ui.interact(
        rect,
        egui::Id::unique(("graphite-button", id)),
        Sense::click(),
    );
    let fill = if selected {
        ACID
    } else if response.hovered() {
        Color32::from_rgb(62, 67, 63)
    } else {
        Color32::from_rgb(40, 44, 43)
    };
    ui.painter().rect_filled(rect, 3.0 * rack.scale, fill);
    ui.painter().rect_stroke(
        rect,
        3.0 * rack.scale,
        Stroke::new(rack.scale, if selected { ACID } else { BORDER }),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(11.0 * rack.scale),
        if selected { Color32::BLACK } else { INK },
    );
    response
}

/// Slots under each TB's routing buttons, reserved for its independent sidechain controls.
pub(crate) fn sidechain_areas(avail: Rect) -> [Rect; 2] {
    let rack = Rack::fit(avail);
    [0, 1].map(|row| rack.rect(858.0, ROWS[row].0 + 119.0, 176.0, 39.0))
}

pub(crate) fn machine_power_button(
    ui: &mut Ui,
    rect: Rect,
    scale: f32,
    id: &str,
    enabled: bool,
) -> egui::Response {
    button(
        ui,
        Rack {
            origin: Pos2::ZERO,
            scale,
        },
        rect,
        id,
        "ON",
        enabled,
    )
    .midi_menu(ui, id)
}

pub(crate) fn mix_knob(
    ui: &mut Ui,
    scale: f32,
    id: &str,
    label: &str,
    rect: Rect,
    position: &mut i32,
) -> egui::Response {
    let mut response = ui
        .interact(rect, egui::Id::unique(("graphite-knob", id)), Sense::drag())
        .midi_menu(ui, id);
    let mut value = *position;
    if response.dragged_by(egui::PointerButton::Primary) {
        let delta = ui.input(|i| -i.pointer.delta().y * if i.modifiers.shift { 1.0 } else { 5.0 });
        value = (value + delta.round() as i32).clamp(0, 1000);
    }
    let r = rect.width() * 0.43;
    let center = Pos2::new(rect.center().x, rect.top() + rect.width() * 0.5);
    ui.painter()
        .circle_filled(center, r * 0.72, Color32::from_rgb(43, 47, 46));
    ui.painter()
        .circle_stroke(center, r * 0.72, Stroke::new(scale, BORDER));
    let point = |a: f32, radius: f32| center + Vec2::new(a.sin(), -a.cos()) * radius;
    let start = -std::f32::consts::PI * 0.75;
    let sweep = std::f32::consts::PI * 1.5;
    let arc = |fraction: f32| {
        (0..=36)
            .map(|n| point(start + sweep * fraction * n as f32 / 36.0, r))
            .collect()
    };
    ui.painter().add(egui::Shape::line(
        arc(1.0),
        Stroke::new(1.7 * scale, BORDER),
    ));
    ui.painter().add(egui::Shape::line(
        arc(value as f32 / 1000.0),
        Stroke::new(1.7 * scale, ACID),
    ));
    let angle = start + sweep * value as f32 / 1000.0;
    ui.painter().line_segment(
        [point(angle, r * 0.25), point(angle, r * 0.60)],
        Stroke::new(2.0 * scale, INK),
    );
    if !label.is_empty() {
        ui.painter().text(
            Pos2::new(rect.center().x, rect.bottom() - 3.0 * scale),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(10.5 * scale),
            DIM,
        );
    }
    if value != *position {
        *position = value;
        response.mark_changed();
    }
    response.on_hover_text(format!(
        "{id}: {value} / 1000\nDrag vertically; Shift for fine control."
    ))
}

pub(crate) fn mix_fader(
    ui: &mut Ui,
    scale: f32,
    id: &str,
    rect: Rect,
    vertical: bool,
    position: &mut i32,
) -> egui::Response {
    let mut response = ui
        .interact(
            rect.expand(5.0 * scale),
            egui::Id::unique(("graphite-fader", id)),
            Sense::click_and_drag(),
        )
        .midi_menu(ui, id);
    if (response.dragged_by(egui::PointerButton::Primary) || response.clicked())
        && let Some(pos) = response.interact_pointer_pos()
    {
        let fraction = if vertical {
            (rect.bottom() - pos.y) / rect.height()
        } else {
            (pos.x - rect.left()) / rect.width()
        };
        let value = (fraction.clamp(0.0, 1.0) * 999.0).round() as i32;
        if *position != value {
            *position = value;
            response.mark_changed();
        }
    }
    ui.painter()
        .rect_filled(rect, 3.0 * scale, Color32::from_rgb(13, 16, 15));
    let fraction = *position as f32 / 999.0;
    let handle = if vertical {
        egui::pos2(rect.center().x, rect.bottom() - rect.height() * fraction)
    } else {
        egui::pos2(rect.left() + rect.width() * fraction, rect.center().y)
    };
    let size = if vertical {
        Vec2::new(22.0, 7.0)
    } else {
        Vec2::new(7.0, 20.0)
    } * scale;
    ui.painter()
        .rect_filled(Rect::from_center_size(handle, size), scale, INK);
    response.on_hover_text(format!("{id}: {:.0}%", fraction * 100.0))
}

pub(crate) fn mix_meter(ui: &Ui, rect: Rect, value: u8) {
    for level in 0..10 {
        let led = Rect::from_min_size(
            egui::pos2(
                rect.left(),
                rect.bottom() - (level + 1) as f32 * rect.height() / 10.0,
            ),
            egui::vec2(rect.width(), rect.height() / 20.0),
        );
        ui.painter()
            .rect_filled(led, 0.0, if level < value { ACID } else { BORDER });
    }
}

pub(crate) fn mixer_button(
    ui: &mut Ui,
    rect: Rect,
    scale: f32,
    id: &str,
    text: &str,
    selected: bool,
) -> egui::Response {
    button(
        ui,
        Rack {
            origin: Pos2::ZERO,
            scale,
        },
        rect,
        id,
        text,
        selected,
    )
    .midi_menu(ui, id)
}

impl App {
    /// Draw transport, four instruments and their mixer strips. The caller embeds the sampler
    /// and effects in the returned rectangles, in screen coordinates.
    pub(crate) fn graphite_rack(&mut self, ui: &mut Ui, avail: Rect) -> (Rect, Rect) {
        let rack = Rack::fit(avail);
        ui.painter()
            .rect_filled(avail, 0.0, Color32::from_rgb(12, 14, 14));
        let active = !self.browsing() && !self.prompting() && !self.error_open();
        ui.add_enabled_ui(active, |ui| {
            self.rack_transport(ui, rack);
            for (row, &(y, h)) in ROWS.iter().enumerate() {
                rack.card(ui, rack.rect(14.0, y, 1030.0, h));
                ui.painter().rect_filled(
                    rack.rect(14.0, y + 10.0, 3.0, h - 20.0),
                    rack.scale,
                    if self.panel.focus == row {
                        ACID
                    } else {
                        BORDER
                    },
                );
                rack.text(ui, 30.0, y + 19.0, 16.0, MACHINES[row], INK);
                rack.text(
                    ui,
                    206.0,
                    y + 19.0,
                    8.0,
                    if row < 2 {
                        "BASS SYNTHESIZER"
                    } else {
                        "RHYTHM COMPOSER"
                    },
                    DIM,
                );
                self.rack_patterns(ui, rack, row, y);
                if row < 2 {
                    self.rack_tb(ui, rack, row, y);
                } else {
                    self.rack_tr(ui, rack, row, y);
                }
                self.rack_mixer(ui, rack, row, y);
                if button(
                    ui,
                    rack,
                    rack.rect(740.0, y + 9.0, 43.0, 19.0),
                    (row, "random"),
                    "RND",
                    false,
                )
                .clicked()
                {
                    self.random_pattern(row);
                }
                if button(
                    ui,
                    rack,
                    rack.rect(789.0, y + 9.0, 43.0, 19.0),
                    (row, "edit"),
                    "EDIT",
                    false,
                )
                .clicked()
                {
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(egui::Id::unique("graphite-edit"), Some(row)));
                }
                if row < 2 {
                    if button(
                        ui,
                        rack,
                        rack.rect(685.0, y + 9.0, 49.0, 19.0),
                        (row, "arp"),
                        "ARP",
                        false,
                    )
                    .on_hover_text("Apply arpeggio now. Configure it in Options > Arpeggiator.")
                    .clicked()
                    {
                        self.apply_arpeggiator_from_rack(row);
                    }
                    if button(
                        ui,
                        rack,
                        rack.rect(617.0, y + 9.0, 62.0, 19.0),
                        (row, "link"),
                        "LINK TB",
                        self.generator.options.link_tb,
                    )
                    .clicked()
                    {
                        self.generator.options.link_tb = !self.generator.options.link_tb;
                    }
                }
            }
        });
        self.rack_advanced(ui);
        (
            rack.rect(14.0, 962.0, 1030.0, 320.0),
            rack.rect(1058.0, 84.0, 208.0, 1202.0),
        )
    }

    fn rack_transport(&mut self, ui: &mut Ui, rack: Rack) {
        rack.card(ui, rack.rect(14.0, 10.0, 1252.0, 60.0));
        rack.text(ui, 30.0, 32.0, 22.0, "ACID RECALL", INK);
        rack.text(ui, 31.0, 53.0, 8.0, "GRAPHITE / STUDIO", DIM);
        for (i, (id, label, on)) in [
            ("transport.play", "PLAY", self.panel.transport.playing),
            ("transport.stop", "STOP", false),
            ("transport.record", "REC", self.panel.transport.recording),
            ("transport.loop", "LOOP", self.panel.transport.looping),
        ]
        .into_iter()
        .enumerate()
        {
            if button(
                ui,
                rack,
                rack.rect(231.0 + i as f32 * 56.0, 25.0, 50.0, 29.0),
                id,
                label,
                on,
            )
            .midi_menu(ui, id)
            .clicked()
            {
                self.panel.press(id, 0.0);
            }
        }
        let tempo = self.panel.transport.tempo_display();
        rack.text(ui, 477.0, 28.0, 9.0, "TEMPO", DIM);
        rack.text(ui, 477.0, 47.0, 18.0, &format!("{tempo} BPM"), ACID);
        for (up, x, label) in [(false, 566.0, "-"), (true, 591.0, "+")] {
            if button(
                ui,
                rack,
                rack.rect(x, 28.0, 21.0, 24.0),
                ("tempo", up),
                label,
                false,
            )
            .clicked()
            {
                self.panel.arrow(if up {
                    "transport.tempo_up"
                } else {
                    "transport.tempo_down"
                });
            }
        }
        let song = self.panel.transport.song_mode;
        if button(
            ui,
            rack,
            rack.rect(637.0, 25.0, 70.0, 29.0),
            "song-mode",
            if song { "SONG" } else { "PATTERN" },
            song,
        )
        .midi_menu(ui, "song-mode")
        .clicked()
        {
            self.panel.set_switch("transport.mode", u8::from(!song));
        }
        for (i, label) in ["MIDI", "TOOLS", "MORE"].into_iter().enumerate() {
            if button(
                ui,
                rack,
                rack.rect(724.0 + i as f32 * 63.0, 25.0, 57.0, 29.0),
                ("transport-tool", i),
                label,
                false,
            )
            .clicked()
            {
                match i {
                    0 => self.midi.open = true,
                    1 => self.generator.open = true,
                    _ => {
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(egui::Id::unique("graphite-edit"), Some(4usize));
                        });
                    }
                }
            }
        }
        rack.text(ui, 934.0, 29.0, 9.0, "MASTER", DIM);
        self.rack_fader(
            ui,
            rack,
            "master.fader",
            rack.rect(935.0, 40.0, 185.0, 12.0),
            false,
        );
        let comp = self.panel.toggle_on("master.comp");
        if button(
            ui,
            rack,
            rack.rect(1140.0, 25.0, 104.0, 29.0),
            "master.comp",
            "COMP MASTER",
            comp,
        )
        .midi_menu(ui, "master.comp")
        .clicked()
        {
            self.panel.press("master.comp", 0.0);
        }
    }

    fn rack_patterns(&mut self, ui: &mut Ui, rack: Rack, row: usize, y: f32) {
        let section = format!("pattern{}", row + 1);
        for i in 0..8u8 {
            let id = format!("{section}.pattern{}", i + 1);
            if button(
                ui,
                rack,
                rack.rect(28.0 + i as f32 * 22.5, y + 39.0, 20.0, 22.0),
                &id,
                &(i + 1).to_string(),
                self.panel.rows[row].lit_button() == Some(i),
            )
            .midi_menu(ui, &id)
            .clicked()
            {
                self.panel.press(&id, 0.0);
            }
        }
        for (i, name) in ["A", "B", "C", "D"].into_iter().enumerate() {
            let id = format!("{section}.bank{name}");
            if button(
                ui,
                rack,
                rack.rect(28.0 + i as f32 * 45.0, y + 67.0, 42.0, 20.0),
                &id,
                name,
                self.panel.rows[row].bank as usize == i,
            )
            .midi_menu(ui, &id)
            .clicked()
            {
                self.panel.press(&id, 0.0);
            }
        }
        let p = self.panel.rows[row].pattern as usize;
        rack.text(
            ui,
            30.0,
            y + 103.0,
            9.0,
            &format!("LENGTH {:02}", self.panel.rows[row].len[p]),
            DIM,
        );
        for (up, x, label) in [(false, 102.0, "-"), (true, 125.0, "+")] {
            if button(
                ui,
                rack,
                rack.rect(x, y + 94.0, 20.0, 19.0),
                (row, "length", up),
                label,
                false,
            )
            .clicked()
            {
                let len = i32::from(self.panel.rows[row].len[p]) + if up { 1 } else { -1 };
                self.panel.set_pattern_length(row, len.clamp(1, 16) as u8);
            }
        }
        let id = format!("{section}.shuffle");
        if button(
            ui,
            rack,
            rack.rect(152.0, y + 94.0, 54.0, 19.0),
            &id,
            "SWING",
            self.panel.toggle_on(&id),
        )
        .midi_menu(ui, &id)
        .clicked()
        {
            self.panel.press(&id, 0.0);
        }
        if row >= 2 {
            if button(
                ui,
                rack,
                rack.rect(28.0, y + 123.0, 178.0, 19.0),
                (row, "groove"),
                "GROOVE GENERATOR",
                false,
            )
            .clicked()
            {
                self.generator.open = true;
            }
        } else {
            let id = format!("tb303_{}.wave", row + 1);
            let wave = self.panel.switch(&id);
            if button(
                ui,
                rack,
                rack.rect(28.0, y + 123.0, 82.0, 21.0),
                &id,
                if wave == 0 { "SAW" } else { "SQUARE" },
                false,
            )
            .midi_menu(ui, &id)
            .clicked()
            {
                self.panel.set_switch(&id, u8::from(wave == 0));
            }
            if button(
                ui,
                rack,
                rack.rect(117.0, y + 123.0, 89.0, 21.0),
                (row, "enable"),
                "ENABLED",
                self.panel.rows[row].enabled,
            )
            .clicked()
            {
                self.panel.press(&format!("{section}.on"), 0.0);
            }
        }
    }

    fn rack_tb(&mut self, ui: &mut Ui, rack: Rack, row: usize, y: f32) {
        for (i, (id, name)) in [
            ("tune", "TUNE"),
            ("cutoff", "CUTOFF"),
            ("reso", "RESONANCE"),
            ("envmod", "ENV MOD"),
            ("decay", "DECAY"),
            ("accent", "ACCENT"),
        ]
        .into_iter()
        .enumerate()
        {
            self.rack_knob(
                ui,
                rack,
                &format!("tb303_{}.{id}", row + 1),
                name,
                rack.rect(241.0 + i as f32 * 96.0, y + 40.0, 55.0, 64.0),
            );
        }
        let pattern = self.panel.rows[row].pattern;
        let length = usize::from(self.panel.rows[row].len[pattern as usize]);
        for step in 0..16 {
            let note = self.panel.synth[row].patterns[pattern as usize][step];
            let selected = self.panel.synth[row].edit as usize == step;
            let label = if note.rest {
                "-"
            } else {
                NOTES[usize::from(note.note.min(12))]
            };
            let rect = rack.rect(224.0 + step as f32 * 38.0, y + 126.0, 34.0, 27.0);
            let response = button(ui, rack, rect, (row, "tb-step", step), label, selected);
            if step >= length {
                ui.painter()
                    .rect_filled(rect, 3.0 * rack.scale, Color32::from_black_alpha(155));
            }
            if self.panel.playing_step303[row] == Some(step) {
                ui.painter().rect_filled(
                    rack.rect(225.0 + step as f32 * 38.0, y + 120.0, 32.0, 2.0),
                    0.0,
                    ACID,
                );
            }
            if note.accent {
                ui.painter().circle_filled(
                    rect.right_top() + Vec2::new(-4.0, 4.0) * rack.scale,
                    1.7 * rack.scale,
                    if selected { Color32::BLACK } else { ACID },
                );
            }
            if note.slide {
                ui.painter().line_segment(
                    [
                        rect.left_bottom() + Vec2::new(5.0, -3.0) * rack.scale,
                        rect.right_bottom() + Vec2::new(-5.0, -3.0) * rack.scale,
                    ],
                    Stroke::new(rack.scale, DIM),
                );
            }
            if response.clicked() || response.secondary_clicked() {
                self.panel.synth[row].edit = step as u8;
                self.panel.focus = row;
            }
            if response.double_clicked() {
                self.panel
                    .press(&format!("tb303_{}.note_pause", row + 1), 0.0);
            }
            response.context_menu(|ui| {
                ui.label(format!("STEP {:02}", step + 1));
                let mut pitch = note.note.min(12);
                if ui
                    .add(crate::midi::slider(ui, &mut pitch, 0..=12).text("Note"))
                    .changed()
                {
                    self.panel.synth[row].edit = step as u8;
                    self.panel
                        .press(&format!("tb303_{}.key{pitch}", row + 1), 0.0);
                }
                for (name, label, on) in [
                    ("note_pause", "Rest", note.rest),
                    ("accent", "Accent", note.accent),
                    ("slide", "Slide", note.slide),
                    ("up", "Octave up", note.up),
                    ("down", "Octave down", note.down),
                ] {
                    let mut value = on;
                    if ui.checkbox(&mut value, label).changed() {
                        self.panel.synth[row].edit = step as u8;
                        self.panel.press(&format!("tb303_{}.{name}", row + 1), 0.0);
                    }
                }
            });
        }
    }

    fn rack_tr(&mut self, ui: &mut Ui, rack: Rack, row: usize, y: f32) {
        let names = if row == 2 {
            [
                "AC", "BD", "SD", "LT", "MT", "HT", "RS", "CP", "CB", "CY", "OH", "CH",
            ]
        } else {
            [
                "AC", "BD", "SD", "LT", "MT", "HT", "RS", "CP", "CH", "OH", "CC", "RC",
            ]
        };
        for (i, name) in names.into_iter().enumerate() {
            let section = if row == 2 { "tr808" } else { "tr909" };
            let voices = if row == 2 { &VOICES_808 } else { &VOICES_909 };
            let x = 224.0 + i as f32 * 50.0;
            // Subtle vertical voice groups make unequal parameter counts intentional and
            // leave nonexistent controls blank rather than inventing silent knobs.
            ui.painter().rect_filled(
                rack.rect(x, y + 33.0, 46.0, 165.0),
                3.0 * rack.scale,
                Color32::from_rgb(25, 28, 28),
            );
            for (parameter, &(id, label)) in voices[i].iter().enumerate() {
                self.rack_knob(
                    ui,
                    rack,
                    &format!("{section}.{id}"),
                    label,
                    rack.rect(x + 12.0, y + 35.0 + parameter as f32 * 40.0, 22.0, 40.0),
                );
            }
            if row == 2 && (3..=7).contains(&i) {
                let id = format!("tr808.slot{i}_switch");
                let original = self.panel.switch_position(&id) != 0;
                let label = match (i, original) {
                    (3..=5, true) => "TOM",
                    (3..=5, false) => "CONGA",
                    (6, true) => "RIM",
                    (6, false) => "CLAVE",
                    (_, true) => "CLAP",
                    (_, false) => "MARACA",
                };
                if button(
                    ui,
                    rack,
                    rack.rect(x + 2.0, y + 174.0, 42.0, 18.0),
                    &id,
                    label,
                    original,
                )
                .midi_menu(ui, &id)
                .clicked()
                {
                    self.panel.set_switch(&id, u8::from(!original));
                }
            }
            if row == 3 && i == 0 {
                let on = self.panel.toggle_on("tr909.flam_button");
                if button(
                    ui,
                    rack,
                    rack.rect(x + 2.0, y + 174.0, 42.0, 18.0),
                    "tr909.flam_button",
                    "FLAM",
                    on,
                )
                .midi_menu(ui, "tr909.flam_button")
                .clicked()
                {
                    self.panel.press("tr909.flam_button", 0.0);
                }
            }
            let selected = if row == 2 {
                self.panel.inst808
            } else {
                self.panel.inst909
            };
            if button(
                ui,
                rack,
                rack.rect(224.0 + i as f32 * 50.0, y + 202.0, 46.0, 18.0),
                (row, "instrument", i),
                name,
                selected == i,
            )
            .clicked()
            {
                self.panel.select_instrument(row == 2, i);
                self.panel.focus = row;
            }
        }
        let length = usize::from(self.panel.rows[row].len[self.panel.rows[row].pattern as usize]);
        for step in 0..16 {
            let value = if row == 2 {
                u8::from(self.panel.steps808[self.panel.inst808][step])
            } else {
                self.panel.steps909[self.panel.inst909][step]
            };
            let rect = rack.rect(224.0 + step as f32 * 38.0, y + 226.0, 34.0, 19.0);
            let response = button(
                ui,
                rack,
                rect,
                (row, "drum-step", step),
                &(step + 1).to_string(),
                value > 0,
            );
            let head = if row == 2 {
                self.panel.playing_step808
            } else {
                self.panel.playing_step909
            };
            if head == Some(step) {
                ui.painter().rect_stroke(
                    rect.expand(2.0 * rack.scale),
                    3.0 * rack.scale,
                    Stroke::new(rack.scale, ACID),
                    egui::StrokeKind::Inside,
                );
            }
            if step >= length {
                ui.painter()
                    .rect_filled(rect, 3.0 * rack.scale, Color32::from_black_alpha(155));
            }
            if response.clicked() {
                self.panel.focus = row;
                if row == 2 {
                    self.panel.set_step808(step, value == 0);
                } else {
                    let next = self.panel.step909_new_value(
                        step,
                        crate::behavior::Mods {
                            ctrl: ui.input(|i| i.modifiers.ctrl),
                            alt: ui.input(|i| i.modifiers.alt),
                        },
                    );
                    self.panel.set_step909(step, next);
                }
            }
        }
    }

    fn rack_mixer(&mut self, ui: &mut Ui, rack: Rack, row: usize, y: f32) {
        if row >= 2 {
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(rack.rect(858.0, y + 129.0, 176.0, 23.0))
                    .id_salt(("drum-trim", row)),
                |ui| {
                    ui.horizontal(|ui| {
                        let response = ui
                            .add(
                                crate::midi::drag_value(
                                    ui,
                                    &mut self.studio.settings.drum_gain_db[row - 2],
                                )
                                .range(-12.0..=12.0)
                                .speed(0.1)
                                .fixed_decimals(1)
                                .prefix("TRIM ")
                                .suffix(" dB"),
                            )
                            .midi_menu(
                                ui,
                                if row == 2 {
                                    "studio.trim808"
                                } else {
                                    "studio.trim909"
                                },
                            );
                        let reset = ui
                            .small_button("0 dB")
                            .on_hover_text("Reset drum output trim");
                        if reset.clicked() {
                            self.studio.settings.drum_gain_db[row - 2] = 0.0;
                        }
                        if response.changed() || reset.clicked() {
                            self.studio.changed();
                            self.doc.modified = true;
                        }
                    });
                },
            );
        }
        ui.painter().line_segment(
            [
                rack.rect(846.0, y + 10.0, 0.0, 0.0).min,
                rack.rect(846.0, y + ROWS[row].1 - 10.0, 0.0, 0.0).min,
            ],
            Stroke::new(rack.scale, BORDER),
        );
        let mixer = format!("mixer{}", row + 1);
        rack.text(ui, 859.0, y + 19.0, 10.0, "MIX", DIM);
        let mute = self.panel.toggle_on(&format!("{mixer}.mute"));
        if machine_power_button(
            ui,
            rack.rect(991.0, y + 9.0, 42.0, 19.0),
            rack.scale,
            &format!("{mixer}.mute"),
            mute,
        )
        .clicked()
        {
            self.panel.press(&format!("{mixer}.mute"), 0.0);
        }
        self.rack_fader(
            ui,
            rack,
            &format!("{mixer}.level"),
            rack.rect(862.0, y + 36.0, 12.0, 43.0),
            true,
        );
        self.rack_knob(
            ui,
            rack,
            &format!("{mixer}.pan"),
            "PAN",
            rack.rect(912.0, y + 33.0, 32.0, 47.0),
        );
        self.rack_knob(
            ui,
            rack,
            &format!("{mixer}.delay"),
            "DELAY",
            rack.rect(974.0, y + 33.0, 32.0, 47.0),
        );
        let meter = self
            .panel
            .meters
            .get(&format!("{mixer}.meter"))
            .copied()
            .unwrap_or(0);
        mix_meter(ui, rack.rect(888.0, y + 38.0, 5.0, 40.0), meter);
        for (i, name) in ["DIST", "PCF", "COMP", "REV"].into_iter().enumerate() {
            let id = format!("{mixer}.{}", name.to_ascii_lowercase());
            let on = if i == 3 {
                self.studio.settings.sends[row] > 0.0
            } else {
                self.panel.toggle_on(&id)
            };
            if button(
                ui,
                rack,
                rack.rect(858.0 + i as f32 * 44.0, y + 89.0, 41.0, 21.0),
                (row, "route", i),
                name,
                on,
            )
            .midi_menu(
                ui,
                &if i == 3 {
                    format!("studio.send{}", row + 1)
                } else {
                    id.clone()
                },
            )
            .clicked()
            {
                if i == 3 {
                    self.studio.settings.sends[row] = if on { 0.0 } else { 0.3 };
                    self.studio.changed();
                    self.doc.modified = true;
                } else {
                    self.panel.press(&id, 0.0);
                }
            }
        }
    }

    fn rack_knob(&mut self, ui: &mut Ui, rack: Rack, id: &str, label: &str, rect: Rect) {
        let mut value = self.panel.knob(id);
        let response = mix_knob(ui, rack.scale, id, label, rect, &mut value);
        if response.dragged_by(egui::PointerButton::Primary) {
            self.panel.held = Some(id.to_string());
        }
        if response.changed() {
            self.panel.set_knob(id, value);
        }
    }

    fn rack_fader(&mut self, ui: &mut Ui, rack: Rack, id: &str, rect: Rect, vertical: bool) {
        let mut value = self.panel.fader(id);
        let response = mix_fader(ui, rack.scale, id, rect, vertical, &mut value);
        if response.dragged_by(egui::PointerButton::Primary) || response.clicked() {
            self.panel.held = Some(id.to_string());
        }
        if response.changed() {
            self.panel.set_fader(id, value);
        }
    }

    fn rack_advanced(&mut self, ui: &mut Ui) {
        let id = egui::Id::unique("graphite-edit");
        let Some(row) = ui.ctx().data(|d| d.get_temp::<Option<usize>>(id)).flatten() else {
            return;
        };
        let title = if row < 4 {
            format!("{} / Detailed editor", MACHINES[row])
        } else {
            "Transport / Detailed editor".into()
        };
        let mut open = true;
        let section = if row < 2 {
            format!("tb303_{}", row + 1)
        } else if row == 2 {
            "tr808".into()
        } else if row == 3 {
            "tr909".into()
        } else {
            "transport".into()
        };
        egui::Window::new(title)
            .open(&mut open)
            .default_width(520.0)
            .vscroll(true)
            .show(ui.ctx(), |ui| {
                ui.label("All original controls, connected to the same MIDI assignments.");
                for c in layout::controls().into_iter().filter(|c| {
                    c.section == section
                        || (row < 4
                            && (c.section == format!("pattern{}", row + 1)
                                || c.section == format!("mixer{}", row + 1)))
                }) {
                    let label = c.name().replace('_', " ");
                    match c.kind.as_str() {
                        "knob" => {
                            let mut v = self.panel.knob(&c.id);
                            if ui
                                .add(crate::midi::slider(ui, &mut v, 0..=1000).text(label))
                                .midi_menu(ui, &c.id)
                                .changed()
                            {
                                self.panel.set_knob(&c.id, v);
                            }
                        }
                        "fader" => {
                            let mut v = self.panel.fader(&c.id);
                            if ui
                                .add(crate::midi::slider(ui, &mut v, 0..=999).text(label))
                                .midi_menu(ui, &c.id)
                                .changed()
                            {
                                self.panel.set_fader(&c.id, v);
                            }
                        }
                        "toggle" | "radio" => {
                            let mut on = self.panel.toggle_on(&c.id);
                            if ui.checkbox(&mut on, label).midi_menu(ui, &c.id).changed() {
                                self.panel.press(&c.id, 0.0);
                            }
                        }
                        "hswitch" | "vswitch" => {
                            let mut v = self.panel.switch_position(&c.id);
                            ui.horizontal(|ui| {
                                ui.label(label);
                                for value in 0..=1 {
                                    if ui
                                        .selectable_value(&mut v, value, value.to_string())
                                        .midi_menu(ui, &c.id)
                                        .changed()
                                    {
                                        self.panel.set_switch(&c.id, value);
                                    }
                                }
                            });
                        }
                        "trigger" => {
                            if ui.button(label).clicked() {
                                self.panel.press(&c.id, 0.0);
                            }
                        }
                        "repeat" => {
                            if ui.button(label).clicked() {
                                self.panel.arrow(&c.id);
                            }
                        }
                        "display" => {
                            ui.label(format!(
                                "{label}: {}",
                                self.panel.display(&c.id).unwrap_or(0)
                            ));
                        }
                        _ => {}
                    }
                }
            });
        if !open {
            ui.ctx().data_mut(|d| d.insert_temp(id, None::<usize>));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Panel;

    #[test]
    fn native_arp_buttons_apply_each_tbs_settings_without_opening_the_editor() {
        for row in 0..2 {
            let ctx = egui::Context::default();
            let mut app = App::new(Panel::new(), None);
            app.arpeggiator.options[row].root = if row == 0 { 0 } else { 9 };
            app.arpeggiator.options[row].length = 7;
            app.panel.press(&format!("pattern{}.bankD", row + 1), 0.0);
            app.panel
                .press(&format!("pattern{}.pattern8", row + 1), 0.0);
            let before = [app.panel.pattern_bytes(0), app.panel.pattern_bytes(1)];
            let expected = app.arpeggiator_preview(row).unwrap().bytes;
            let selected = app.panel.rows[row].pattern;
            let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(WIDTH, HEIGHT));
            let position = Rack::fit(screen)
                .rect(685.0, ROWS[row].0 + 9.0, 49.0, 19.0)
                .center();
            for pressed in [None, Some(true), Some(false)] {
                let events = pressed.map_or_else(Vec::new, |pressed| {
                    vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::default(),
                        },
                    ]
                });
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(screen),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        app.graphite_rack(ui, screen);
                    },
                );
                output.textures_delta.clear();
            }
            assert_eq!(app.panel.pattern_bytes(row), expected);
            assert_eq!(app.panel.pattern_bytes(1 - row), before[1 - row]);
            assert_eq!(app.panel.rows[row].pattern, selected);
            assert_eq!(app.arpeggiator.open, [false, false]);
            assert!(app.doc.modified);
            assert!(app.undo_arpeggiator(row));
            assert_eq!(app.panel.pattern_bytes(row), before[row]);
        }
    }

    #[test]
    fn every_real_drum_parameter_is_directly_available_with_no_invented_controls() {
        use std::collections::BTreeSet;
        for (section, voices) in [("tr808", VOICES_808), ("tr909", VOICES_909)] {
            let actual: BTreeSet<_> = layout::controls()
                .into_iter()
                .filter(|c| c.section == section && c.kind == "knob")
                .map(|c| c.id)
                .collect();
            let exposed: BTreeSet<_> = voices
                .iter()
                .flat_map(|voice| voice.iter())
                .map(|(name, _)| format!("{section}.{name}"))
                .collect();
            assert_eq!(
                exposed, actual,
                "The {section} face must expose its real voice controls"
            );
        }
    }

    #[test]
    fn drum_attack_decay_and_snappy_are_directly_draggable_on_the_rack() {
        for (id, row, instrument, parameter) in [
            ("tr808.bd_decay", 2, 1, 2),
            ("tr808.sd_snappy", 2, 2, 2),
            ("tr909.bd_attack", 3, 1, 2),
            ("tr909.sd_snappy", 3, 2, 3),
        ] {
            let ctx = egui::Context::default();
            let mut app = App::new(Panel::new(), None);
            let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(WIDTH, HEIGHT));
            let rack = Rack::fit(screen);
            let pos = rack
                .rect(
                    236.0 + instrument as f32 * 50.0,
                    ROWS[row].0 + 35.0 + parameter as f32 * 40.0,
                    22.0,
                    40.0,
                )
                .center();
            let before = app.panel.knob(id);
            let moved = pos - Vec2::new(0.0, 20.0);
            for events in [
                vec![],
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::default(),
                    },
                ],
                vec![egui::Event::PointerMoved(moved)],
                vec![egui::Event::PointerButton {
                    pos: moved,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                }],
            ] {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(screen),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        app.graphite_rack(ui, screen);
                    },
                );
                output.textures_delta.clear();
            }
            assert!(
                app.panel.knob(id) > before,
                "{id} must be editable without opening EDIT"
            );
            assert!(app.panel.events.iter().any(|event|
                matches!(event, crate::model::Event::Set { id: changed, .. } if changed == id)));
        }
    }

    #[test]
    fn native_rack_does_not_push_the_menu_to_the_bottom() {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let mut app = App::new(Panel::new(), None);
        app.studio.modern = true;
        app.menu = true;
        let mut menu_bottom = 0.0;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(WIDTH, HEIGHT))),
                ..Default::default()
            },
            |ui| {
                app.show(ui);
                menu_bottom = ui.cursor().top();
            },
        );
        assert!(
            menu_bottom < 70.0,
            "The menu must remain at the top of its parent UI"
        );
        output.textures_delta.clear();
    }

    #[test]
    fn master_compressor_is_visible_and_clickable_in_the_native_rack() {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let mut app = App::new(Panel::new(), None);
        app.studio.modern = true;
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(WIDTH, HEIGHT));
        let position = Rack::fit(rect).rect(1140.0, 25.0, 104.0, 29.0).center();
        assert!(!app.panel.toggle_on("master.comp"));
        for pressed in [None, Some(true), Some(false)] {
            let events = pressed.map_or_else(Vec::new, |pressed| {
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::default(),
                    },
                ]
            });
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(rect),
                    events,
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
        }
        assert!(app.panel.toggle_on("master.comp"));
        for area in sidechain_areas(rect) {
            assert!(rect.contains_rect(area));
            assert!(area.left() > rect.center().x);
        }
    }
}
