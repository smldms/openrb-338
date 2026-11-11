use eframe::egui::{self, Align2, Color32, Context, FontId, Rect, Sense, Ui};

use super::{Change, DrumStyle, Rng, Scale, Undo, drum_pattern, tb_pair};
use crate::app::{App, View};

const MACHINES: [&str; 4] = ["TB-303 A", "TB-303 B", "TR-808", "TR-909"];
const NOTES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

fn rack_button(
    ui: &mut Ui,
    rect: Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
    text: &str,
    on: bool,
    scale: f32,
) -> egui::Response {
    let response = ui.interact(rect, egui::Id::unique(id), Sense::click());
    let color = if on {
        Color32::from_rgb(0x26, 0x80, 0x78)
    } else if response.hovered() {
        Color32::from_rgb(0x65, 0x71, 0x82)
    } else {
        Color32::from_rgb(0x25, 0x2d, 0x37)
    };
    ui.painter().rect_filled(rect, 1.5 * scale, color);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(6.5 * scale),
        Color32::WHITE,
    );
    response
}

impl App {
    fn flush_generator_edits(&mut self) {
        self.note_edits();
        if let Some(link) = &mut self.link {
            link.forward(&mut self.panel);
        }
    }

    fn generator_before(&self, row: usize, pattern: u8) -> Vec<u8> {
        if let Some(link) = &self.link {
            let size = if row < 2 { 98 } else { 194 };
            let at = usize::from(pattern) * size;
            return link.patterns()[row][at..at + size].to_vec();
        }
        self.panel.pattern_bytes(row)
    }

    /// Generate the selected slot(s). Explicit seeds are also used by regression tests.
    pub fn random_pattern_with_seed(&mut self, row: usize, seed: u64) {
        if row >= 4 {
            return;
        }
        self.flush_generator_edits();
        let pattern_size = if row < 2 { 98 } else { 194 };
        if self
            .link
            .as_ref()
            .is_some_and(|l| l.patterns()[row].len() < 32 * pattern_size)
        {
            self.generator.notice = Some("Load a drum kit before generating its patterns.".into());
            return;
        }
        let options = &self.generator.options;
        let length = self.panel.rows[row].len[self.panel.rows[row].pattern as usize].clamp(1, 16);
        let linked = row < 2 && options.link_tb;
        let shuffle = self.panel.rows[row].shuffle[self.panel.rows[row].pattern as usize];
        let generated: Vec<(usize, Vec<u8>)> = if row < 2 {
            let [bass, answer] = tb_pair(options, length, seed);
            if linked {
                vec![(0, bass), (1, answer)]
            } else {
                vec![(row, bass)]
            }
        } else {
            vec![(row, drum_pattern(options, row == 2, length, seed))]
        };
        let tuning = if linked {
            let before = [
                self.panel.knob("tb303_1.tune"),
                self.panel.knob("tb303_2.tune"),
            ];
            let value = before[row];
            self.panel.set_knob("tb303_1.tune", value);
            self.panel.set_knob("tb303_2.tune", value);
            Some((before, [value; 2]))
        } else {
            None
        };
        let mut changes = Vec::new();
        for (r, mut bytes) in generated {
            let pattern = self.panel.rows[r].pattern;
            let before = self.generator_before(r, pattern);
            let pattern_shuffle = if linked {
                shuffle
            } else {
                self.panel.rows[r].shuffle[usize::from(pattern)]
            };
            bytes[0] = u8::from(pattern_shuffle);
            self.panel.replace_pattern(r, pattern, bytes.clone());
            changes.push(Change {
                row: r,
                pattern,
                before,
                after: bytes,
            });
        }
        self.generator.undo = Some(Undo { changes, tuning });
        self.generator.notice = Some(if linked {
            format!(
                "Generated linked TBs: {} {}, {length} steps.",
                NOTES[usize::from(self.generator.options.root % 12)],
                self.generator.options.scale.name()
            )
        } else {
            format!("Generated {}: {length} steps.", MACHINES[row])
        });
        self.flush_generator_edits();
    }

    pub fn random_pattern(&mut self, row: usize) {
        let mut rng = Rng(self.generator.seed);
        let seed = rng.next();
        self.generator.seed = seed;
        self.random_pattern_with_seed(row, seed);
    }

    /// Undo is offered only while the generated notes, length and linked tuning still match.
    /// Later manual/MIDI edits are never silently overwritten by this one-action undo.
    pub fn can_undo_random(&self) -> bool {
        let Some(undo) = &self.generator.undo else {
            return false;
        };
        if let Some((_, after)) = undo.tuning {
            let now = [
                self.panel.knob("tb303_1.tune"),
                self.panel.knob("tb303_2.tune"),
            ];
            if now != after {
                return false;
            }
        }
        undo.changes.iter().all(|c| {
            if self.panel.rows[c.row].pattern == c.pattern {
                self.panel.pattern_bytes(c.row) == c.after
            } else if let Some(link) = &self.link {
                let size = c.after.len();
                let at = usize::from(c.pattern) * size;
                link.patterns()[c.row][at..at + size] == c.after
            } else {
                false
            }
        })
    }

    pub fn undo_random(&mut self) -> bool {
        if !self.can_undo_random() {
            return false;
        }
        self.flush_generator_edits();
        let undo = self.generator.undo.take().unwrap();
        for c in undo.changes {
            self.panel.replace_pattern(c.row, c.pattern, c.before);
        }
        if let Some((before, _)) = undo.tuning {
            self.panel.set_knob("tb303_1.tune", before[0]);
            self.panel.set_knob("tb303_2.tune", before[1]);
        }
        self.generator.notice = Some("Restored the patterns before the last Random.".into());
        self.flush_generator_edits();
        true
    }

    pub(crate) fn generator_buttons(&mut self, ui: &mut Ui, view: &View) {
        if self.browsing() || self.prompting() || self.error_open() {
            return;
        }
        let mut generate = None;
        let mut undo = false;
        for (row, name) in MACHINES.iter().enumerate() {
            let y = 131.0 + row as f32 * 115.0;
            let response = rack_button(
                ui,
                view.rect(20.0, y, 30.0, 12.0),
                ("random-pattern", row),
                "RND",
                false,
                view.scale,
            )
            .on_hover_text(format!(
                "Random {name}. Right-click for generator settings and Undo Random."
            ));
            if response.clicked() {
                generate = Some(row);
            }
            response.context_menu(|ui| {
                if ui.button("Generator settings...").clicked() {
                    self.generator.open = true;
                    ui.close();
                }
                if ui
                    .add_enabled(self.can_undo_random(), egui::Button::new("Undo Random"))
                    .clicked()
                {
                    undo = true;
                    ui.close();
                }
            });
            if row < 2 {
                let response = rack_button(
                    ui,
                    view.rect(54.0, y - 1.0, 28.0, 8.0),
                    ("link-tb", row),
                    "LINK",
                    self.generator.options.link_tb,
                    view.scale,
                )
                .on_hover_text(
                    "LINK TB: generate both bass lines in one key, with shared length and tuning.",
                );
                if response.clicked() {
                    self.generator.options.link_tb = !self.generator.options.link_tb;
                }
            }
        }
        if let Some(row) = generate {
            self.random_pattern(row);
            ui.ctx().request_repaint();
        }
        if undo {
            self.undo_random();
            ui.ctx().request_repaint();
        }
    }

    pub(crate) fn generator_ui(&mut self, ctx: &Context) {
        if !self.generator.open {
            return;
        }
        let mut open = true;
        let mut generate = None;
        let mut undo = false;
        let can_undo = self.can_undo_random();
        egui::Window::new("Pattern Generator")
            .open(&mut open)
            .default_width(430.0)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.heading("Musical Random");
                ui.label("Generate the selected pattern of each machine. Length: 1 to 16 steps.");
                let o = &mut self.generator.options;
                ui.horizontal(|ui| {
                    ui.label("Key");
                    egui::ComboBox::from_id_salt("generator-key")
                        .selected_text(NOTES[usize::from(o.root % 12)])
                        .show_ui(ui, |ui| {
                            for (root, name) in NOTES.iter().enumerate() {
                                ui.selectable_value(&mut o.root, root as u8, *name);
                            }
                        });
                    egui::ComboBox::from_id_salt("generator-scale")
                        .selected_text(o.scale.name())
                        .show_ui(ui, |ui| {
                            for scale in Scale::ALL {
                                ui.selectable_value(&mut o.scale, scale, scale.name());
                            }
                        });
                });
                ui.checkbox(&mut o.link_tb, "LINK TB - two complementary lines");
                ui.small(
                    "Linked TBs use the clicked TB's length and tuning, in the selected key/scale.",
                );
                ui.add(egui::Slider::new(&mut o.density, 0..=100).text("Notes (%)"));
                ui.add(egui::Slider::new(&mut o.accents, 0..=100).text("Accents (%)"));
                ui.add(egui::Slider::new(&mut o.slides, 0..=100).text("Slides (%)"));
                ui.add(egui::Slider::new(&mut o.variation, 0..=100).text("Variation (%)"));
                ui.separator();
                ui.label("Drum grooves");
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut o.drum_style,
                        DrumStyle::Techno,
                        "Techno / four on the floor",
                    );
                    ui.selectable_value(&mut o.drum_style, DrumStyle::Broken, "Broken");
                });
                ui.add(egui::Slider::new(&mut o.drum_density, 0..=100).text("Drum density (%)"));
                ui.separator();
                for (row, name) in MACHINES.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(*name);
                        let pattern = usize::from(self.panel.rows[row].pattern);
                        let mut length = self.panel.rows[row].len[pattern].clamp(1, 16);
                        if ui
                            .add(
                                egui::DragValue::new(&mut length)
                                    .range(1..=16)
                                    .suffix(" steps"),
                            )
                            .changed()
                        {
                            self.panel.set_pattern_length(row, length);
                        }
                        if ui.button("Random").clicked() {
                            generate = Some(row);
                        }
                    });
                }
                ui.separator();
                if ui
                    .add_enabled(can_undo, egui::Button::new("Undo Random"))
                    .clicked()
                {
                    undo = true;
                }
                if let Some(notice) = &self.generator.notice {
                    ui.label(notice);
                }
            });
        self.generator.open = open;
        if let Some(row) = generate {
            self.random_pattern(row);
            ctx.request_repaint();
        }
        if undo {
            self.undo_random();
            ctx.request_repaint();
        }
    }
}
