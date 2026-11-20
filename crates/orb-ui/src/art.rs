//! The default Graphite SVG parts, generated and embedded by `build.rs`, and knob geometry.

include!(concat!(env!("OUT_DIR"), "/parts_gen.rs"));

/// A part: its URI for egui's loaders and its SVG bytes.
#[derive(Clone, Copy)]
pub struct Part {
    pub name: &'static str,
    pub uri: &'static str,
    pub svg: &'static [u8],
}

/// The part called `name` (`PARTS` is sorted by name).
pub fn part(name: &str) -> Option<Part> {
    let i = PARTS.binary_search_by(|p| p.0.cmp(name)).ok()?;
    let (name, uri, svg) = PARTS[i];
    Some(Part { name, uri, svg })
}

/// The part names asked for that no art has, in this process.
static MISSING: std::sync::Mutex<std::collections::BTreeSet<String>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());

/// Records that the part `name` is missing; true the first time it is.
pub fn report_missing(name: &str) -> bool {
    let mut m = MISSING.lock().unwrap_or_else(|e| e.into_inner());
    m.insert(name.to_owned())
}

/// The part names drawn as nothing because no art has them, in this process (task-156).
pub fn missing() -> Vec<String> {
    let m = MISSING.lock().unwrap_or_else(|e| e.into_inner());
    m.iter().cloned().collect()
}

/// Every part.
pub fn all() -> impl Iterator<Item = Part> {
    PARTS
        .iter()
        .map(|&(name, uri, svg)| Part { name, uri, svg })
}

/// A knob design: the layers are `<name>_body`, the arc painted by the app and
/// `<name>_pointer` rotated about `(cx, cy)` (assets/ui/README.md).
pub struct KnobDesign {
    pub name: &'static str,
    pub w: f32,
    pub h: f32,
    pub cx: f32,
    pub cy: f32,
    /// The value arc's radius and width, 0 for none.
    pub ring_r: f32,
    pub ring_w: f32,
    /// The arc color as RGB.
    pub arc: [u8; 3],
}

pub const KNOBS: [KnobDesign; 5] = [
    KnobDesign {
        name: "knob_303",
        w: 30.0,
        h: 30.0,
        cx: 15.0,
        cy: 15.0,
        ring_r: 13.6,
        ring_w: 1.6,
        arc: crate::palette::KNOB_ARC,
    },
    KnobDesign {
        name: "knob_808_red",
        w: 13.0,
        h: 13.0,
        cx: 6.5,
        cy: 6.5,
        ring_r: 5.9,
        ring_w: 0.9,
        arc: crate::palette::KNOB_ARC,
    },
    KnobDesign {
        name: "knob_808_white",
        w: 13.0,
        h: 13.0,
        cx: 6.5,
        cy: 6.5,
        ring_r: 5.9,
        ring_w: 0.9,
        arc: crate::palette::KNOB_ARC,
    },
    KnobDesign {
        name: "knob_909",
        w: 12.0,
        h: 12.0,
        cx: 6.0,
        cy: 6.0,
        ring_r: 5.4,
        ring_w: 0.9,
        arc: crate::palette::KNOB_ARC,
    },
    KnobDesign {
        name: "knob_22",
        w: 22.0,
        h: 22.0,
        cx: 11.0,
        cy: 11.0,
        ring_r: 10.1,
        ring_w: 1.4,
        arc: crate::palette::KNOB_ARC,
    },
];

/// The design of a knob control, by its layout part name.
pub fn knob_design(part: &str) -> &'static KnobDesign {
    KNOBS
        .iter()
        .find(|k| k.name == part)
        .unwrap_or_else(|| panic!("no knob design {part}"))
}

/// The window's icon: the `app_icon` part rendered at `size` x `size` pixels.
pub fn app_icon(size: u32) -> eframe::egui::IconData {
    let image = image::load_from_memory_with_format(
        include_bytes!("../../../assets/brand/acid-recall.png"),
        image::ImageFormat::Png,
    )
    .expect("ACID RECALL icon decodes")
    .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
    .into_rgba8();
    eframe::egui::IconData {
        rgba: image.into_raw(),
        width: size,
        height: size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_default_is_graphite() {
        let part = part("bg_tb303").unwrap();
        let svg = std::str::from_utf8(part.svg).unwrap();
        assert!(!svg.contains(orb_ui_art::svg::pal::PAPER));
        assert!(svg.contains(orb_ui_art::graphite::INK));
    }

    #[test]
    fn knob_designs_match_the_art_generator() {
        for (a, b) in KNOBS.iter().zip(orb_ui_art::parts::KNOBS.iter()) {
            assert_eq!(a.name, b.name);
            assert_eq!((a.w, a.h, a.cx, a.cy), (b.w, b.h, b.cx, b.cy));
            assert_eq!((a.ring_r, a.ring_w), (b.ring_r, b.ring_w));
            assert_eq!(a.arc, orb_ui_art::graphite::ARC, "{}", a.name);
        }
    }

    #[test]
    fn all_generated_parts_are_embedded_and_sorted() {
        assert_eq!(PARTS.len(), orb_ui_art::parts::all_parts().len());
        assert!(PARTS.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(part("btn_play_lit").is_some());
        assert!(part("panel").is_none());
        assert!(part("nope").is_none());
    }
}
