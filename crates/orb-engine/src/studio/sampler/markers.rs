//! A bounded, monotonic map from musical time to positions in the original sample.

pub const MAX_WARP_MARKERS: usize = 16;
const GAP: f64 = 0.00001;

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WarpMarker {
    /// Position in the whole original sample, normalized to 0..1.
    pub source: f64,
    /// Position in the selected musical region, normalized to 0..1.
    pub target: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct WarpMarkers {
    pub markers: [WarpMarker; MAX_WARP_MARKERS],
    pub count: u8,
}

impl WarpMarkers {
    pub fn points(&self) -> &[WarpMarker] {
        &self.markers[..usize::from(self.count).min(MAX_WARP_MARKERS)]
    }

    /// Reject invalid/crossed anchors without allocating, including in the audio callback.
    pub fn sanitize(&mut self, start: f64, end: f64) {
        let old = *self;
        *self = Self::default();
        let (mut source, mut target) = (start, 0.0);
        for point in old.points() {
            if point.source.is_finite()
                && point.target.is_finite()
                && point.source > source + GAP
                && point.source < end - GAP
                && point.target > target + GAP
                && point.target < 1.0 - GAP
            {
                self.markers[usize::from(self.count)] = *point;
                self.count += 1;
                source = point.source;
                target = point.target;
            }
        }
    }

    pub fn insert(&mut self, source: f64, target: f64, start: f64, end: f64) -> Option<usize> {
        self.sanitize(start, end);
        let n = self.points().len();
        if n == MAX_WARP_MARKERS || !source.is_finite() || !target.is_finite() {
            return None;
        }
        let index = self.points().partition_point(|p| p.target < target);
        let lo = if index == 0 {
            WarpMarker {
                source: start,
                target: 0.0,
            }
        } else {
            self.markers[index - 1]
        };
        let hi = if index == n {
            WarpMarker {
                source: end,
                target: 1.0,
            }
        } else {
            self.markers[index]
        };
        if source <= lo.source + GAP
            || source >= hi.source - GAP
            || target <= lo.target + GAP
            || target >= hi.target - GAP
        {
            return None;
        }
        self.markers.copy_within(index..n, index + 1);
        self.markers[index] = WarpMarker { source, target };
        self.count += 1;
        Some(index)
    }

    pub fn move_target(&mut self, index: usize, target: f64) {
        if index >= self.points().len() || !target.is_finite() {
            return;
        }
        let lo = if index == 0 {
            0.0
        } else {
            self.markers[index - 1].target
        };
        let hi = self.points().get(index + 1).map_or(1.0, |p| p.target);
        let margin = GAP * 1.01;
        if hi - lo <= margin * 2.0 {
            return;
        }
        self.markers[index].target = target.clamp(lo + margin, hi - margin);
    }

    pub fn remove(&mut self, index: usize) {
        let n = self.points().len();
        if index < n {
            self.markers.copy_within(index + 1..n, index);
            self.markers[n - 1] = WarpMarker::default();
            self.count = (n - 1) as u8;
        }
    }

    /// Convert source position to its displayed grid position, including when WARP is bypassed.
    pub fn target_for_source(&self, source: f64, start: f64, end: f64) -> f64 {
        let points = self.points();
        let index = points.partition_point(|p| p.source <= source);
        let lo = if index == 0 {
            WarpMarker {
                source: start,
                target: 0.0,
            }
        } else {
            points[index - 1]
        };
        let hi = points.get(index).copied().unwrap_or(WarpMarker {
            source: end,
            target: 1.0,
        });
        let span = hi.source - lo.source;
        if span <= 0.0 {
            return 0.0;
        }
        (lo.target + (source - lo.source) / span * (hi.target - lo.target)).clamp(0.0, 1.0)
    }

    /// Returns absolute source position and source advance per normalized musical unit.
    pub fn map(&self, target: f64, start: f64, end: f64) -> (f64, f64) {
        let target = if target.is_finite() {
            target.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let points = self.points();
        let index = points.partition_point(|p| p.target <= target);
        let lo = if index == 0 {
            WarpMarker {
                source: start,
                target: 0.0,
            }
        } else {
            points[index - 1]
        };
        let hi = points.get(index).copied().unwrap_or(WarpMarker {
            source: end,
            target: 1.0,
        });
        let slope = (hi.source - lo.source) / (hi.target - lo.target).max(GAP);
        (lo.source + (target - lo.target) * slope, slope)
    }
}
