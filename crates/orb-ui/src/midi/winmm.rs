//! Small WinMM input adapter. Only short messages are accepted; no SysEx buffers are queued.
//!
//! API signatures: Microsoft Learn, midiInOpen, MIDIINCAPSW and midiInClose (mmeapi.h).
//! The callback never calls WinMM, egui or the audio player. Its context stays allocated until
//! midiInClose succeeds, including callbacks during stop/reset/close.

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

use super::Port;

type Handle = *mut c_void;

#[repr(C)]
struct Caps {
    manufacturer: u16,
    product: u16,
    version: u32,
    name: [u16; 32],
    support: u32,
}

#[link(name = "winmm")]
unsafe extern "system" {
    fn midiInGetNumDevs() -> u32;
    fn midiInGetDevCapsW(id: usize, caps: *mut Caps, size: u32) -> u32;
    fn midiInGetErrorTextW(error: u32, text: *mut u16, len: u32) -> u32;
    fn midiInOpen(handle: *mut Handle, id: u32, callback: usize, data: usize, flags: u32) -> u32;
    fn midiInStart(handle: Handle) -> u32;
    fn midiInStop(handle: Handle) -> u32;
    fn midiInReset(handle: Handle) -> u32;
    fn midiInClose(handle: Handle) -> u32;
}

fn check(code: u32, action: &str) -> Result<(), String> {
    if code == 0 {
        return Ok(());
    }
    let mut text = [0u16; 256];
    // SAFETY: a writable, sized UTF-16 buffer, used only for this call.
    let result = unsafe { midiInGetErrorTextW(code, text.as_mut_ptr(), text.len() as u32) };
    let detail = if result == 0 {
        let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
        String::from_utf16_lossy(&text[..end])
    } else {
        format!("WinMM error {code}")
    };
    Err(format!("{action}: {detail}"))
}

pub fn ports() -> Result<Vec<Port>, String> {
    // SAFETY: the device count has no arguments or ownership requirements.
    let count = unsafe { midiInGetNumDevs() };
    let mut ports = Vec::new();
    for id in 0..count {
        let mut caps = Caps {
            manufacturer: 0,
            product: 0,
            version: 0,
            name: [0; 32],
            support: 0,
        };
        // SAFETY: caps matches MIDIINCAPSW's layout and remains writable for this call.
        check(
            unsafe {
                midiInGetDevCapsW(id as usize, &mut caps, std::mem::size_of::<Caps>() as u32)
            },
            "List MIDI inputs",
        )?;
        let end = caps
            .name
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(caps.name.len());
        ports.push(Port {
            id,
            name: String::from_utf16_lossy(&caps.name[..end]),
        });
    }
    Ok(ports)
}

struct Callback {
    clock: Option<super::sync::Sender>,
    sender: SyncSender<u32>,
    dropped: AtomicUsize,
}

unsafe extern "system" fn receive(
    _handle: Handle,
    message: u32,
    data: usize,
    word: usize,
    _timestamp: usize,
) {
    // MIM_DATA; no MIDI_IO_STATUS flag was requested, so MIM_MOREDATA is not delivered.
    if message != 0x3c3 || data == 0 {
        return;
    }
    // SAFETY: Input owns the context until a successful midiInClose. Callback only shares it.
    let callback = unsafe { &*(data as *const Callback) };
    if let Some(clock) = &callback.clock {
        clock.receive(word as u32);
    }
    if super::Cc::decode(word as u32).is_some() && callback.sender.try_send(word as u32).is_err() {
        callback.dropped.fetch_add(1, Ordering::Relaxed);
    }
}

pub struct Input {
    handle: Handle,
    context: Option<Box<Callback>>,
    receiver: Receiver<u32>,
}

impl Input {
    pub fn open(id: u32, clock: Option<super::sync::Sender>) -> Result<Self, String> {
        let (sender, receiver) = sync_channel(1024);
        let context = Box::new(Callback {
            clock,
            sender,
            dropped: AtomicUsize::new(0),
        });
        let mut handle = std::ptr::null_mut();
        // SAFETY: handle is writable; the system-ABI callback and boxed context outlive the
        // device. The context is not moved by moving its Box. CALLBACK_FUNCTION = 0x30000.
        check(
            unsafe {
                midiInOpen(
                    &mut handle,
                    id,
                    receive as *const () as usize,
                    (&*context as *const Callback) as usize,
                    0x30000,
                )
            },
            "Open MIDI input (close other applications using this port)",
        )?;
        let input = Self {
            handle,
            context: Some(context),
            receiver,
        };
        // SAFETY: handle belongs to the successful open above. On error Input::drop closes it.
        check(unsafe { midiInStart(handle) }, "Start MIDI input")?;
        Ok(input)
    }

    pub fn drain(&self) -> Vec<u32> {
        // Bound work even if a device sends continuously while the UI drains its queue.
        self.receiver.try_iter().take(1024).collect()
    }

    pub fn dropped(&self) -> usize {
        self.context
            .as_ref()
            .map_or(0, |c| c.dropped.load(Ordering::Relaxed))
    }
}

#[repr(C)]
struct OutCaps {
    manufacturer: u16,
    product: u16,
    version: u32,
    name: [u16; 32],
    technology: u16,
    voices: u16,
    notes: u16,
    channel_mask: u16,
    support: u32,
}

#[link(name = "winmm")]
unsafe extern "system" {
    fn midiOutGetNumDevs() -> u32;
    fn midiOutGetDevCapsW(id: usize, caps: *mut OutCaps, size: u32) -> u32;
    fn midiOutOpen(handle: *mut Handle, id: u32, callback: usize, data: usize, flags: u32) -> u32;
    fn midiOutShortMsg(handle: Handle, word: u32) -> u32;
    fn midiOutClose(handle: Handle) -> u32;
    fn timeBeginPeriod(period: u32) -> u32;
    fn timeEndPeriod(period: u32) -> u32;
}

pub fn output_ports() -> Result<Vec<Port>, String> {
    // SAFETY: WinMM enumeration fills a correctly sized MIDIOUTCAPSW structure.
    let count = unsafe { midiOutGetNumDevs() };
    let mut ports = Vec::new();
    for id in 0..count {
        let mut caps: OutCaps = unsafe { std::mem::zeroed() };
        check(
            unsafe {
                midiOutGetDevCapsW(
                    id as usize,
                    &mut caps,
                    std::mem::size_of::<OutCaps>() as u32,
                )
            },
            "List MIDI outputs",
        )?;
        let end = caps
            .name
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(caps.name.len());
        ports.push(Port {
            id,
            name: String::from_utf16_lossy(&caps.name[..end]),
        });
    }
    Ok(ports)
}

/// Opened, used and dropped on the clock worker only. No unsafe Send implementation.
pub struct Output {
    handle: Handle,
}

impl Output {
    pub fn open(id: u32) -> Result<Self, String> {
        let mut handle = std::ptr::null_mut();
        // SAFETY: writable handle, no callbacks or SysEx buffers.
        check(
            unsafe { midiOutOpen(&mut handle, id, 0, 0, 0) },
            "Open MIDI Clock output",
        )?;
        Ok(Self { handle })
    }
    pub fn send(&mut self, word: u32) -> Result<(), String> {
        // SAFETY: this worker exclusively owns the valid open handle.
        check(
            unsafe { midiOutShortMsg(self.handle, word) },
            "Send MIDI Clock",
        )
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        // SAFETY: only owner; no queued long buffers.
        unsafe {
            midiOutClose(self.handle);
        }
    }
}

pub struct Timer(bool);
impl Timer {
    pub fn new() -> Self {
        // SAFETY: paired process timer-resolution request, released by Drop.
        Self(unsafe { timeBeginPeriod(1) } == 0)
    }
}
impl Drop for Timer {
    fn drop(&mut self) {
        if self.0 {
            unsafe {
                timeEndPeriod(1);
            }
        }
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        // SAFETY: this is the only owner of the handle. No SysEx buffers have been submitted.
        // WinMM may still call receive during shutdown, so keep the context until close ends.
        let result = unsafe {
            midiInStop(self.handle);
            midiInReset(self.handle);
            midiInClose(self.handle)
        };
        if result != 0
            && let Some(context) = self.context.take()
        {
            // A driver refusing close must not leave a dangling callback pointer. Leak the
            // small context in this exceptional case; the OS reclaims it at process exit.
            let _ = Box::leak(context);
        }
    }
}
