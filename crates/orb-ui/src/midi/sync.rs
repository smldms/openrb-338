//! Clock driver worker, deliberately independent from egui and the audio callback.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

use orb_player::clock::{Connection, Follower, MASTER, OFF, SLAVE, Shared};
use serde::{Deserialize, Serialize};

use super::backend;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Off,
    Master,
    Slave,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub mode: Mode,
    pub input: Option<String>,
    pub output: Option<String>,
    /// Delay outgoing clock/transport to compensate the audio interface's output latency.
    pub output_delay_ms: u16,
    /// Phase correction: negative advances playback, positive delays it.
    pub input_offset_ms: i16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::Off,
            input: None,
            output: None,
            output_delay_ms: 30,
            input_offset_ms: 0,
        }
    }
}

#[derive(Clone)]
pub struct Sender {
    sender: SyncSender<(u32, u64)>,
    shared: Arc<Shared>,
}

impl Sender {
    pub fn receive(&self, word: u32) {
        if !matches!(word as u8, 0xf2 | 0xf8 | 0xfa | 0xfb | 0xfc) {
            return;
        }
        if self.sender.try_send((word, self.shared.now())).is_err() {
            self.shared.overflow.fetch_add(1, Ordering::Relaxed);
            self.shared.inhibit.store(true, Ordering::Release);
        }
    }
}

struct Worker {
    stop: Arc<AtomicBool>,
    join: JoinHandle<Connection>,
}

#[derive(Default)]
pub struct Runtime {
    connection: Option<Connection>,
    shared: Option<Arc<Shared>>,
    worker: Option<Worker>,
    pub sender: Option<Sender>,
    pub error: Option<String>,
    failed: Arc<AtomicBool>,
}

impl Runtime {
    pub fn attach(&mut self, connection: Option<Connection>) {
        if let Some(connection) = connection {
            self.shared = Some(connection.shared.clone());
            self.connection = Some(connection);
        }
    }

    pub fn stop(&mut self) {
        self.sender = None;
        if let Some(shared) = &self.shared {
            shared.mode.store(OFF, Ordering::Release);
        }
        if let Some(worker) = self.worker.take() {
            worker.stop.store(true, Ordering::Release);
            if let Ok(connection) = worker.join.join() {
                self.connection = Some(connection);
            }
        }
    }

    pub fn configure(&mut self, config: &Config) -> Result<(), String> {
        self.stop();
        self.error = None;
        self.failed.store(false, Ordering::Release);
        if config.mode == Mode::Off {
            return Ok(());
        }
        if !cfg!(windows) {
            return Err("MIDI Clock currently requires Windows.".into());
        }
        let output = if config.mode == Mode::Master {
            let name = config.output.as_deref().ok_or("Choose a Clock OUT port.")?;
            let ports: Vec<_> = backend::output_ports()?
                .into_iter()
                .filter(|p| p.name == name)
                .collect();
            if ports.len() != 1 {
                return Err("Clock OUT unavailable or name is ambiguous.".into());
            }
            Some(ports[0].id)
        } else {
            if config.input.is_none() {
                return Err("Choose a Clock IN port.".into());
            }
            None
        };
        let Some(mut connection) = self.connection.take() else {
            return Err("Clock needs a running audio player.".into());
        };
        let shared = connection.shared.clone();
        shared.inhibit.store(false, Ordering::Release);
        shared.lost.store(false, Ordering::Release);
        shared.overflow.store(0, Ordering::Relaxed);
        shared.bpm.store(0, Ordering::Relaxed);
        shared.input_offset_ms.store(
            i32::from(config.input_offset_ms.clamp(-500, 500)),
            Ordering::Release,
        );
        // Reset the previous source before opening a new one; never resume from stale transport.
        shared.write(orb_player::clock::InputFrame {
            period: 20_833_333,
            ..orb_player::clock::InputFrame::default()
        });
        while connection.output.pop().is_ok() {}
        let (sender, receiver) = sync_channel(1024);
        let clock_sender = Sender {
            sender,
            shared: shared.clone(),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let failed = self.failed.clone();
        let mode = if config.mode == Mode::Master {
            MASTER
        } else {
            SLAVE
        };
        let delay = u64::from(config.output_delay_ms.min(500)) * 1_000_000;
        let (ready, opened) = sync_channel(1);
        let join = std::thread::spawn(move || {
            let _timer = backend::Timer::new();
            let mut output = match output.map(backend::Output::open).transpose() {
                Ok(output) => {
                    let _ = ready.send(Ok(()));
                    output
                }
                Err(e) => {
                    let _ = ready.send(Err(e));
                    return connection;
                }
            };
            let mut follower = Follower::default();
            let mut pending = VecDeque::with_capacity(1024);
            let mut heartbeat = shared.now();
            let mut sent_clock = false;
            shared.mode.store(mode, Ordering::Release);
            while !stop_thread.load(Ordering::Acquire) {
                let now = shared.now();
                if now.saturating_sub(heartbeat) > 250_000_000 {
                    failed.store(true, Ordering::Release);
                    break; // Resume from sleep/stall must not burst old clocks into hardware.
                }
                heartbeat = now;
                for (word, at) in receiver.try_iter().take(1024) {
                    if mode == SLAVE {
                        follower.receive(word, at, &shared);
                    }
                }
                if mode == SLAVE {
                    follower.timeout(now, &shared);
                }
                while pending.len() < 1024 {
                    let Ok(event) = connection.output.pop() else {
                        break;
                    };
                    pending.push_back(event);
                }
                if let Some(output) = &mut output {
                    while pending.front().is_some_and(|e| e.at + delay <= now) {
                        let event = pending.pop_front().unwrap();
                        sent_clock |= event.word == 0xf8;
                        if now.saturating_sub(event.at + delay) > 100_000_000
                            || output.send(event.word).is_err()
                        {
                            failed.store(true, Ordering::Release);
                            break;
                        }
                    }
                } else {
                    pending.clear();
                }
                if failed.load(Ordering::Acquire) || shared.overflow.load(Ordering::Relaxed) != 0 {
                    failed.store(true, Ordering::Release);
                    break;
                }
                if sent_clock
                    && now.saturating_sub(shared.audio_at.load(Ordering::Acquire)) > 500_000_000
                {
                    failed.store(true, Ordering::Release);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            shared.mode.store(OFF, Ordering::Release);
            shared.inhibit.store(true, Ordering::Release);
            if let Some(output) = &mut output {
                let _ = output.send(0xfc);
            }
            connection
        });
        self.worker = Some(Worker { stop, join });
        match opened.recv() {
            Ok(Ok(())) => {
                self.sender = Some(clock_sender);
                Ok(())
            }
            Ok(Err(e)) => {
                self.stop();
                Err(e)
            }
            Err(_) => {
                self.stop();
                Err("Clock worker could not start.".into())
            }
        }
    }

    pub fn status(&self) -> String {
        if let Some(error) = &self.error {
            return error.clone();
        }
        if self.failed.load(Ordering::Acquire) {
            return "Clock stopped: driver error, queue overflow or scheduling stall. Reconnect."
                .into();
        }
        let Some(shared) = &self.shared else {
            return "Clock off".into();
        };
        match shared.mode.load(Ordering::Acquire) {
            MASTER => "MASTER / Clock OUT / 24 PPQN".into(),
            SLAVE if shared.lost.load(Ordering::Acquire) => {
                "CLOCK LOST / stopped / waiting for external Start or Continue".into()
            }
            SLAVE => {
                let bpm = shared.bpm.load(Ordering::Relaxed);
                if bpm == 0 {
                    "SLAVE / waiting for MIDI Clock".into()
                } else {
                    format!(
                        "SLAVE / {:.2} BPM / Start, Continue, Stop",
                        f64::from(bpm) / 1000.0
                    )
                }
            }
            _ => "Clock off".into(),
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop();
    }
}
