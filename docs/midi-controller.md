# UC-33e MIDI controller (Windows)

This fork adds MIDI CC input through Windows' built-in WinMM MIDI API. The existing audio
engine, song format and offline rendering are unchanged. macOS and Linux still build, but
their MIDI device backends are not implemented in this first version.

## Connect and learn

1. Connect the UC-33e over USB before starting OpenRB-338. Close any other application using
   its MIDI input. Use a controller preset that sends ordinary absolute Control Change
   messages (0 to 127); NRPN, relative encoders, SysEx and instrument notes are not supported.
2. Open **Options > MIDI Controller...**, enable MIDI input and choose the UC-33e input.
3. Check **Last input** changes when moving a fader or knob.
4. Choose a parameter (the filter accepts `303`, `TR-808`, `TR-909`, `delay`, etc.), click
   **Learn**, then move the physical control. That first message assigns without changing
   the parameter. Learning replaces any previous owner of that CC/channel or target.
5. Repeat for other controls. Assignments, the exact port name and the pickup preference
   save automatically in the existing `OpenRB-338/ui.toml` configuration. They do not change
   `.rbs` files. A MIDI edit marks the song modified just like a mouse edit.

Use **Remove** to clear an assignment. Close the window to keep performing; input stays
active. Disable MIDI input to close the port. After unplugging/replugging the device, use
**Refresh / reconnect**. An unavailable saved port never silently falls back to another device.
If two inputs have the same Windows name, give the intended input a unique name.

## Suggested UC-33e layout

No factory CC numbers are assumed, since a used UC-33e may have custom presets. Learn the
physical controls you want:

| Physical controls | OpenRB parameters |
| --- | --- |
| Faders 1-4 | Mixer / 303 A, 303 B, TR-808, TR-909 / level |
| Faders 5-8 | The same four mixer channels / delay |
| Fader 9 | Master / fader |
| Knobs | 303 cutoff/reso/envmod/decay; TR sound tuning/decay/levels; effect parameters |
| CC buttons | Play/Stop, mixer mutes, distortion/routing toggles, pattern/bank buttons |

The PCF, distortion, compressor and delay keep ReBirth's original shared routing. The
controller can change their parameters, the four delay sends and available routing switches;
it does not add separate effect instances to each machine. The mixer `mute` switch follows
the original panel's enabled/on state.

## Soft takeover

Enabled by default. A fader/knob waits until its hardware value reaches or crosses the current
software value, then follows normally. Mouse edits, automation and song loads which change
the value re-arm pickup. Turn it off for immediate absolute control. Buttons react once on a
CC rising above 63 and re-arm below 64; configure momentary CC buttons accordingly. Two-state
switches use below 64 / above 63 directly. Effect numeric selectors map to their valid ranges.

MIDI input is handled on the UI thread, polling at 8 ms while connected, and forwarded to the
existing player command queue. Its 1024-frame render-call timing still applies. No MIDI work
or allocation is added to the audio callback. Clock input/output and direct note playing are
reserved for a later change.

## Verification

Use **Export routing...** to back up all CC/channel assignments and soft takeover in a portable
`.toml` file. **Import routing...** validates the file and replaces the current assignments.
Choose the MIDI input on each computer: the port and enable state stay local. Invalid files
leave your current routing intact. The controls remain in the existing MIDI Controller window.

`cargo test -p orb-ui --test midi` checks CC decoding, channel filtering, full parameter
ranges, pickup/re-arming, button edges, selectors, mapping replacement, persistence and
delivery through the real player. The Windows preview also runs settings, wiring, repaint and
audio allocation tests and builds a portable executable; the PR CI runs the full workspace.
A physical UC-33e test is still needed to verify USB enumeration,
driver opening, button preset behavior and shutdown/reconnection on the user's PC.
