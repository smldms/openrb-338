//! MIDI transport bridge. Audio only accesses bounded queues and atomics; drivers live elsewhere.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

pub const OFF: u8 = 0;
pub const MASTER: u8 = 1;
pub const SLAVE: u8 = 2;

#[derive(Clone, Copy, Debug, Default)]
pub struct InputFrame {
    pub at: u64,
    pub tick: u64,
    pub period: u64,
    pub playing: bool,
    pub generation: u32,
}

pub struct Shared {
    epoch: Instant,
    pub mode: AtomicU8,
    pub inhibit: AtomicBool,
    pub lost: AtomicBool,
    pub overflow: AtomicU64,
    pub bpm: AtomicU32,
    pub input_offset_ms: AtomicI32,
    pub audio_at: AtomicU64,
    revision: AtomicU64,
    at: AtomicU64,
    tick: AtomicU64,
    period: AtomicU64,
    playing: AtomicBool,
    generation: AtomicU32,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            epoch: Instant::now(),
            mode: AtomicU8::new(OFF),
            inhibit: AtomicBool::new(false),
            lost: AtomicBool::new(false),
            overflow: AtomicU64::new(0),
            bpm: AtomicU32::new(0),
            input_offset_ms: AtomicI32::new(0),
            audio_at: AtomicU64::new(0),
            revision: AtomicU64::new(0),
            at: AtomicU64::new(0),
            tick: AtomicU64::new(0),
            period: AtomicU64::new(20_833_333),
            playing: AtomicBool::new(false),
            generation: AtomicU32::new(0),
        }
    }
}

impl Shared {
    pub fn now(&self) -> u64 {
        self.epoch.elapsed().as_nanos() as u64
    }

    /// Exactly one writer: the dedicated MIDI worker. Sequential atomics make the snapshot
    /// coherent.
    pub fn write(&self, s: InputFrame) {
        self.revision.fetch_add(1, Ordering::SeqCst);
        self.at.store(s.at, Ordering::SeqCst);
        self.tick.store(s.tick, Ordering::SeqCst);
        self.period.store(s.period, Ordering::SeqCst);
        self.playing.store(s.playing, Ordering::SeqCst);
        self.generation.store(s.generation, Ordering::SeqCst);
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    /// One attempt, never a wait on the audio thread. Keep the previous state on contention.
    pub fn read(&self) -> Option<InputFrame> {
        let revision = self.revision.load(Ordering::SeqCst);
        if revision & 1 != 0 {
            return None;
        }
        let s = InputFrame {
            at: self.at.load(Ordering::SeqCst),
            tick: self.tick.load(Ordering::SeqCst),
            period: self.period.load(Ordering::SeqCst),
            playing: self.playing.load(Ordering::SeqCst),
            generation: self.generation.load(Ordering::SeqCst),
        };
        (revision == self.revision.load(Ordering::SeqCst)).then_some(s)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub at: u64,
    pub word: u32,
}

pub struct Connection {
    pub shared: Arc<Shared>,
    pub output: rtrb::Consumer<Event>,
}

pub(crate) struct AudioClock {
    pub shared: Arc<Shared>,
    output: rtrb::Producer<Event>,
    phase: f64,
    playing: bool,
    mode: u8,
    base_frame: u64,
    base_time: u64,
    pub input: InputFrame,
    pub generation: u32,
    pub resync: bool,
}

pub(crate) fn bridge() -> (Connection, AudioClock) {
    let shared = Arc::new(Shared::default());
    let (output, receiver) = rtrb::RingBuffer::new(1024);
    (
        Connection {
            shared: shared.clone(),
            output: receiver,
        },
        AudioClock {
            shared,
            output,
            phase: 0.0,
            playing: false,
            mode: OFF,
            base_frame: 0,
            base_time: 0,
            input: InputFrame::default(),
            generation: u32::MAX,
            resync: false,
        },
    )
}

impl AudioClock {
    pub fn was_slave(&self) -> bool {
        self.mode == SLAVE
    }
    pub fn begin_pull(&mut self, frame: u64) {
        self.base_frame = frame;
        self.base_time = self.shared.now();
        self.shared
            .audio_at
            .store(self.base_time, Ordering::Release);
    }

    fn send(&mut self, frame: u64, word: u32) {
        let at = self.base_time + frame.saturating_sub(self.base_frame) * 1_000_000_000 / 44100;
        if self.output.push(Event { at, word }).is_err() {
            self.shared.overflow.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn master(&mut self, frame: u64, count: usize, playing: bool, tempo: u32, tick: i32) {
        let mode = self.shared.mode.load(Ordering::Acquire);
        if self.mode != mode {
            self.playing = false;
            self.phase = 0.0;
            self.generation = u32::MAX;
            self.mode = mode;
        }
        if mode != MASTER {
            return;
        }
        if self.playing && (!playing || self.resync) {
            self.send(frame, 0xfc);
        }
        if playing && (!self.playing || self.resync) {
            if tick <= 0 {
                self.send(frame, 0xfa);
            } else {
                let position = (tick as u32 / 48).min(16383);
                self.send(
                    frame,
                    0xf2 | ((position & 127) << 8) | ((position >> 7) << 16),
                );
                self.send(frame, 0xfb);
            }
            self.phase = 0.0;
            self.send(frame, 0xf8);
        }
        self.resync = false;
        self.playing = playing;
        if !playing {
            return;
        }
        let step = f64::from(tempo) * 24.0 / (60_000.0 * 44100.0);
        for offset in 0..count {
            // Test before increment: the pulse at the exact block end belongs to the next block.
            if self.phase >= 1.0 - 1e-12 {
                self.phase -= 1.0;
                self.send(frame + offset as u64, 0xf8);
            }
            self.phase += step;
        }
    }
}

#[cfg(test)]
mod tests;

/// Input state machine, called with timestamps captured at the MIDI callback, not GUI poll time.
pub struct Follower {
    pub state: InputFrame,
    next_tick: u64,
    armed: bool,
    armed_at: u64,
    previous: Option<u64>,
    intervals: [u64; 6],
    count: usize,
    cursor: usize,
}

impl Default for Follower {
    fn default() -> Self {
        Self {
            state: InputFrame {
                period: 20_833_333,
                ..InputFrame::default()
            },
            next_tick: 0,
            armed: false,
            armed_at: 0,
            previous: None,
            intervals: [0; 6],
            count: 0,
            cursor: 0,
        }
    }
}

impl Follower {
    pub fn receive(&mut self, word: u32, at: u64, shared: &Shared) {
        match word as u8 {
            0xfa | 0xfb => {
                if word as u8 == 0xfa {
                    self.next_tick = 0;
                }
                self.state.playing = false;
                self.armed = true;
                self.armed_at = at;
                self.state.generation = self.state.generation.wrapping_add(1);
                shared.inhibit.store(false, Ordering::Release);
                shared.lost.store(false, Ordering::Release);
            }
            0xfc => {
                self.state.playing = false;
                self.armed = false;
            }
            0xf2 if word & 0x808000 == 0 => {
                self.next_tick = u64::from(((word >> 8) & 127) | (((word >> 16) & 127) << 7)) * 48;
                self.state.tick = self.next_tick;
                self.state.at = at;
                self.state.generation = self.state.generation.wrapping_add(1);
            }
            0xf8 => {
                if let Some(previous) = self.previous {
                    let interval = at.saturating_sub(previous);
                    // Supported 20..400 BPM; reject duplicate and wildly late packets.
                    if (6_000_000..=130_000_000).contains(&interval) {
                        self.intervals[self.cursor] = interval;
                        self.cursor = (self.cursor + 1) % self.intervals.len();
                        self.count = (self.count + 1).min(self.intervals.len());
                        self.state.period = self.intervals.iter().sum::<u64>() / self.count as u64;
                        shared.bpm.store(
                            (2_500_000_000_000u64 / self.state.period) as u32,
                            Ordering::Relaxed,
                        );
                    } else {
                        self.intervals = [0; 6];
                        self.count = 0;
                        self.cursor = 0;
                    }
                }
                self.previous = Some(at);
                if self.armed || self.state.playing {
                    self.armed = false;
                    self.state.playing = true;
                    self.state.tick = self.next_tick;
                    self.state.at = at;
                    self.next_tick += 8;
                }
            }
            _ => return,
        }
        shared.write(self.state);
    }

    pub fn timeout(&mut self, now: u64, shared: &Shared) {
        if (self.state.playing || self.armed)
            && now.saturating_sub(self.previous.unwrap_or(0).max(self.armed_at)) > 500_000_000
        {
            self.state.playing = false;
            self.armed = false;
            self.state.generation = self.state.generation.wrapping_add(1);
            shared.lost.store(true, Ordering::Release);
            shared.write(self.state);
        }
    }
}
