# MIDI Clock IN / OUT — ACID RECALL (Windows)

Open MIDI, then **MIDI CLOCK / SYNC**. Clock is off by default. Preferences remember the
mode, ports, OUT delay and IN offset locally; importing a controller routing file never changes them.
Clock is a system message, independent from MIDI channels and CC assignments.

## MASTER / OUT: ACID RECALL leads the hardware

1. Connect your interface's MIDI OUT to the drum machine's MIDI IN (or use its USB MIDI port).
2. Set the drum machine to external MIDI Clock and enable reception of Start/Stop.
3. Select MASTER / OUT and the correct output port in ACID RECALL.
4. Press Play. ACID RECALL sends Start and 24 Timing Clocks per quarter note. Stop sends Stop.
   Starting from a nonzero location sends Song Position Pointer and Continue; the receiver
   must support these messages to resume at that position. Song Position Pointer has
   sixteenth-note resolution. Clock runs during playback, not while stopped.
5. **OUT delay defaults to 30 ms** (adjustable from 0–500 ms). Adjust it if the drum machine sounds
   ahead of the software. This delays
   outgoing Clock and transport to compensate your audio interface's audible output latency.
   Reconfiguring the connection sends Stop; restart playback afterwards.

Saved delay values remain unchanged, including an explicitly saved 0 ms. Use
**Default OUT delay: 30 ms** to apply the new starting point to an existing setup. The best
value depends on the audio interface, buffer size and connected machine; 30 ms is a starting point.

Timing comes from audio frame counts, retaining fractional clock phase across blocks and
tempo changes. A separate worker schedules timestamped events and calls WinMM. No MIDI
driver calls, memory allocation or waiting run on the audio thread. When sync is active,
the player renders 64-frame blocks; OFF retains its original 1024-frame calls.

## SLAVE / IN: the drum machine leads ACID RECALL

1. Connect the drum machine's MIDI OUT to your interface's MIDI IN, or use USB MIDI.
2. Make the drum machine the master and enable sending MIDI Clock and transport.
3. Select SLAVE / IN and the correct input in ACID RECALL.
4. Start the drum machine. Start resets to the beginning; Continue resumes; Song Position
   Pointer locates in sixteenth notes. Playback begins at the next received Timing Clock.
5. The BPM is estimated from timestamped pulses and follows changes (supported range
   20–400 BPM). A bounded phase correction follows the clock without seeking on every pulse.
   Large discontinuities re-locate. All four machines and the sampler share this transport;
   enable TIME WARP on samples that should follow tempo without transposition.

6. **IN offset** is a manual phase adjustment from **−500 to +500 ms**, default **0 ms**.
   A **negative** value advances ACID RECALL relative to the incoming clock (try −30 ms if
   the software is late); a **positive** value delays it. The adjustment converges gradually
   during playback rather than seeking or retriggering notes on each pulse. The master still
   controls Start, Stop and position. An offset cannot predict an external Start before it
   arrives, so the first downbeat remains subject to the PC's audio buffering. After changing
   sync settings, restart the external transport. This is manual calibration, not automatic
   measurement or removal of the audio driver's latency.

In slave mode local Play, tempo and locate changes do not override the external master.
Local **Stop** is an emergency stop; the next external Start/Continue releases it. MIDI Stop
holds position instead of the ordinary local bar rewind. If Clock disappears for 500 ms,
playback stops and status shows CLOCK LOST. A new Start/Continue is required to resume.
Disconnecting slave sync also halts playback instead of silently switching to free-running.

## Controllers, safety and limits

- Your UC-33e and additional CC controllers continue working. Clock input may share one
  already-open controller port; the software opens the device only once. Clock messages do
  not enter MIDI Learn. A separate clock input works even with CC input disabled.
- Choose only one master. IN and OUT are exclusive modes; incoming Clock is not echoed to
  output, preventing accidental clock feedback. This is MIDI Clock, not MTC, DIN Sync,
  ReWire or live MIDI note output. It requires an actual MIDI interface/USB connection;
  an audio cable carries no MIDI Clock.
- Stop is sent on output disconnect/shutdown. Driver failures, queue overflow, stale audio
  and long worker stalls stop synchronization instead of bursting queued pulses. Use
  Refresh / reconnect after connecting hardware again or resuming from sleep.
- WinMM drivers, Windows scheduling, audio buffering, USB and hardware introduce latency
  and jitter. OUT delay is manual; there is no automatic round-trip calibration. Test on
  your real interface and drum machine; automated tests do not certify physical timing.
- Use a MIDI Thru box/interface routing to distribute one output to several machines.
  This version selects one Clock OUT destination.

Protocol references: MIDI Association, MIDI 1.0 Detailed Specification (Timing Clock,
Start/Continue/Stop and Song Position Pointer), and Microsoft Learn WinMM `midiOutShortMsg`,
`midiOutGetDevCapsW` and `midiInOpen`. No new MIDI library is bundled.
