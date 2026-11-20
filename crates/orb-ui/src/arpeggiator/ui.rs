use eframe::egui::{self, Align2, Color32, Context, FontId, Sense, Ui};

use super::{Chord, Order, Preview, Source, Undo, note_name, render};
use crate::app::{App, View};

const NAMES: [&str; 2] = ["TB-303 A", "TB-303 B"];
const KEYS: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

fn preview_grid(ui: &mut Ui, row: usize, preview: &Preview) {
    let pool = preview
        .notes
        .iter()
        .map(|&n| note_name(n))
        .collect::<Vec<_>>()
        .join(" / ");
    ui.label(format!("Note pool: {pool}"));
    ui.small("Octave marks are relative to the TB base register; Tune still transposes the sound.");
    egui::Grid::new(("arp-preview", row))
        .num_columns(8)
        .show(ui, |ui| {
            for (s, b) in preview.bytes[2..]
                .as_chunks::<6>()
                .0
                .iter()
                .take(usize::from(preview.bytes[1]))
                .enumerate()
            {
                let text = if b[5] != 0 {
                    "-".into()
                } else {
                    let pitch = i32::from(b[0]) + 12 * i32::from(b[3]) - 12 * i32::from(b[4]);
                    format!(
                        "{}{}{}",
                        note_name(pitch),
                        if b[2] != 0 { "*" } else { "" },
                        if b[1] != 0 { "~" } else { "" }
                    )
                };
                let color = if b[5] != 0 {
                    Color32::GRAY
                } else if b[2] != 0 {
                    Color32::from_rgb(235, 185, 80)
                } else {
                    Color32::WHITE
                };
                ui.label(
                    egui::RichText::new(format!("{:02}\n{text}", s + 1))
                        .monospace()
                        .color(color),
                );
                if s % 8 == 7 {
                    ui.end_row();
                }
            }
        });
    ui.small("* accent   ~ slide   - rest");
}

impl App {
    pub fn open_arpeggiator(&mut self, row: usize) {
        if row >= 2 {
            return;
        }
        let pattern = usize::from(self.panel.rows[row].pattern);
        self.arpeggiator.options[row].length = self.panel.rows[row].len[pattern].clamp(1, 16);
        self.arpeggiator.open[row] = true;
        self.arpeggiator.notice[row] = None;
    }

    fn flush_arpeggio_edits(&mut self) {
        self.note_edits();
        if let Some(link) = &mut self.link {
            link.forward(&mut self.panel);
        }
    }

    /// Render without changing notes, tuning, selection, transport or the document.
    pub fn arpeggiator_preview(&self, row: usize) -> Result<Preview, String> {
        if row >= 2 {
            return Err("Choose TB-303 A or B.".into());
        }
        render(
            &self.arpeggiator.options[row],
            &self.panel.pattern_bytes(row),
        )
    }

    pub fn apply_arpeggiator(&mut self, row: usize) -> Result<(), String> {
        let preview = self.arpeggiator_preview(row)?;
        self.flush_arpeggio_edits();
        let pattern = self.panel.rows[row].pattern;
        let before = if let Some(link) = &self.link {
            let start = usize::from(pattern) * 98;
            link.patterns()[row]
                .get(start..start + 98)
                .ok_or("The selected TB has no pattern storage.")?
                .to_vec()
        } else {
            self.panel.pattern_bytes(row)
        };
        self.panel
            .replace_pattern(row, pattern, preview.bytes.clone());
        self.arpeggiator.undo[row] = Some(Undo {
            pattern,
            before,
            after: preview.bytes,
        });
        self.arpeggiator.notice[row] = Some(format!(
            "Applied to {} / pattern {}.",
            NAMES[row],
            pattern + 1
        ));
        self.flush_arpeggio_edits();
        Ok(())
    }

    /// Rack actions apply the current settings immediately; Options opens the editor.
    pub(crate) fn apply_arpeggiator_from_rack(&mut self, row: usize) {
        if let Err(error) = self.apply_arpeggiator(row) {
            self.fail(error);
        }
    }

    pub fn can_undo_arpeggiator(&self, row: usize) -> bool {
        if row >= 2 {
            return false;
        }
        let Some(undo) = &self.arpeggiator.undo[row] else {
            return false;
        };
        if self.panel.rows[row].pattern == undo.pattern {
            self.panel.pattern_bytes(row) == undo.after
        } else if let Some(link) = &self.link {
            let start = usize::from(undo.pattern) * 98;
            link.patterns()[row].get(start..start + 98) == Some(undo.after.as_slice())
        } else {
            false
        }
    }

    pub fn undo_arpeggiator(&mut self, row: usize) -> bool {
        // Update the mirror before checking an offscreen slot, including pending edits.
        self.flush_arpeggio_edits();
        if !self.can_undo_arpeggiator(row) {
            return false;
        }
        let undo = self.arpeggiator.undo[row].take().unwrap();
        self.panel.replace_pattern(row, undo.pattern, undo.before);
        self.arpeggiator.notice[row] =
            Some(format!("Restored the previous {} pattern.", NAMES[row]));
        self.flush_arpeggio_edits();
        true
    }

    pub(crate) fn arpeggiator_buttons(&mut self, ui: &mut Ui, view: &View) {
        if self.browsing() || self.prompting() || self.error_open() {
            return;
        }
        for (row, name) in NAMES.iter().enumerate() {
            let rect = view.rect(106.0, 86.0 + 115.0 * row as f32, 36.0, 9.0);
            let response = ui.interact(rect, egui::Id::unique(("arp-open", row)), Sense::click());
            let fill = if response.hovered() {
                Color32::from_rgb(0x50, 0x50, 0x56)
            } else {
                Color32::from_rgb(0x30, 0x30, 0x34)
            };
            ui.painter().rect_filled(rect, view.scale, fill);
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "ARP",
                FontId::proportional(6.5 * view.scale),
                Color32::WHITE,
            );
            if response
                .on_hover_text(format!(
                    "Apply {name} arpeggio now. Configure it in Options > Arpeggiator."
                ))
                .clicked()
            {
                self.apply_arpeggiator_from_rack(row);
                ui.ctx().request_repaint();
            }
        }
    }

    pub(crate) fn arpeggiator_ui(&mut self, ctx: &Context) {
        for (row, name) in NAMES.iter().enumerate() {
            if !self.arpeggiator.open[row] {
                continue;
            }
            let mut open = true;
            let mut apply = false;
            let mut undo = false;
            let can_undo = self.can_undo_arpeggiator(row);
            let pattern = self.panel.pattern_bytes(row);
            egui::Window::new(format!("Arpeggiator / {name}"))
                .open(&mut open).default_width(490.0).vscroll(true).show(ctx, |ui| {
                    ui.label("Preview an arpeggio, then apply it to this TB's selected pattern.");
                    let o = &mut self.arpeggiator.options[row];
                    ui.horizontal(|ui| {
                        ui.label("Notes from");
                        ui.selectable_value(&mut o.source, Source::Chord, "Chord");
                        ui.selectable_value(&mut o.source, Source::Pattern, "Current pattern");
                    });
                    if o.source == Source::Chord {
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt(("arp-root", row))
                                .selected_text(KEYS[usize::from(o.root % 12)]).show_ui(ui, |ui| {
                                    for (root, key) in KEYS.iter().enumerate() {
                                        ui.selectable_value(&mut o.root, root as u8, *key);
                                    }
                                });
                            egui::ComboBox::from_id_salt(("arp-chord", row))
                                .selected_text(o.chord.name()).show_ui(ui, |ui| {
                                    for chord in Chord::ALL {
                                        ui.selectable_value(&mut o.chord, chord, chord.name());
                                    }
                                });
                        });
                    } else {
                        ui.small("Uses distinct active notes within the source pattern's length.");
                    }
                    ui.horizontal(|ui| {
                        ui.label("Direction");
                        egui::ComboBox::from_id_salt(("arp-order", row))
                            .selected_text(o.order.name()).show_ui(ui, |ui| {
                                for order in Order::ALL {
                                    ui.selectable_value(&mut o.order, order, order.name());
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Octaves");
                        ui.add(egui::DragValue::new(&mut o.octaves).range(1..=3));
                        ui.label("Length");
                        ui.add(egui::DragValue::new(&mut o.length).range(1..=16).suffix(" steps"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Rhythm");
                        for (every, label) in [(1, "1/16"), (2, "1/8"), (4, "1/4"), (8, "1/2")] {
                            ui.selectable_value(&mut o.every, every, label);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Start offset");
                        ui.add_enabled(o.order != Order::Random,
                            egui::DragValue::new(&mut o.offset).range(0..=32));
                        ui.label("Accent every (0 = off)");
                        ui.add(egui::DragValue::new(&mut o.accent_every).range(0..=8));
                    });
                    ui.checkbox(&mut o.slides, "Slides between adjacent notes");
                    if o.order == Order::Random && ui.button("New random variation").clicked() {
                        o.seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                            .map_or(o.seed.wrapping_add(1), |d| d.as_nanos() as u64);
                    }
                    ui.separator();
                    let preview = render(o, &pattern);
                    match &preview {
                        Ok(p) => preview_grid(ui, row, p),
                        Err(e) => { ui.colored_label(Color32::LIGHT_RED, e); }
                    }
                    ui.small("Octave expansion fits the TB register; the preview shows the actual notes.");
                    ui.horizontal(|ui| {
                        apply = ui.add_enabled(preview.is_ok(), egui::Button::new("Apply to pattern")).clicked();
                        undo = ui.add_enabled(can_undo, egui::Button::new("Undo arpeggio")).clicked();
                    });
                    if let Some(notice) = &self.arpeggiator.notice[row] {
                        ui.label(notice);
                    }
                });
            self.arpeggiator.open[row] = open;
            if apply {
                if let Err(e) = self.apply_arpeggiator(row) {
                    self.arpeggiator.notice[row] = Some(e);
                }
                ctx.request_repaint();
            }
            if undo {
                self.undo_arpeggiator(row);
                ctx.request_repaint();
            }
        }
    }
}
