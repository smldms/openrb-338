# ACID RECALL — MIDI controller setup (Windows)

[ACID RECALL website](https://acid-recall.netlify.app/) · [Studio guide](https://github.com/smldms/Acid-Recall/blob/feat/studio-v2/docs/studio.md)

Control ACID RECALL with a UC-33e or another controller sending absolute MIDI CC messages.
Windows MIDI input uses the built-in WinMM API. macOS and Linux still build, but their MIDI
device backends are not implemented. MIDI Clock sync has its own [setup guide](midi-clock.md).

## Connect and learn

1. Connect your controller over USB before starting ACID RECALL. Close any other application using
   its MIDI input. Use a controller preset that sends ordinary absolute Control Change
   messages (0 to 127); NRPN, relative encoders, SysEx and instrument notes are not supported.
2. Open **Options > MIDI Controller...**, enable MIDI input and choose your controller input.
3. Check **Last input** changes when moving a fader or knob.
4. Back on the rack, **right-click the knob, fader or button**, choose **MIDI LEARN...**,
   then move the physical control. That first message assigns without changing the
   parameter. Learning replaces any previous owner of that CC/channel or target.
5. Right-click an assigned continuous control again to choose **MAX**, anywhere from
   **0 to 127**, or **Remove MIDI assignment**. MAX is optional and defaults to 127.
   For example, MAX 50 makes a physical fader at 127 reach 50 on the MIDI value scale.
   The bottom remains zero; there is no per-assignment MIN.
6. Repeat for other controls. Assignments, the exact port name and the pickup preference
   save automatically in the existing `OpenRB-338/ui.toml` configuration. They do not change
   `.rbs` files. A MIDI edit marks the song modified just like a mouse edit.

You can also learn from the MIDI Controller window: filter for a parameter (`303`,
`TR-808`, `TR-909`, `delay`, etc.), select it, click **Learn**, then move the controller.
Use **Remove** there to clear an assignment. Close the window to keep performing; input stays
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
CC rising above 63 and re-arm below 64 for actions such as Play, Stop and pattern selection.
Machine ON/OFF, effect enables, distortion, PCF/compressor routing, pattern enable and swing
use absolute states instead: **0–63 = OFF, 64–127 = ON**. Set controller buttons to their
latching/toggle preset (alternating **0 / 127**), as for sampler ON/OFF. Repeated identical
messages keep the state rather than toggling it. Sending OFF to an inactive PCF/compressor
route leaves another machine's active route untouched. Reverb and delay sends remain
continuous controls and also support 0 = no send, 127 = the chosen MAX (full send by default).
Those send endpoints apply immediately even with soft takeover enabled, so a latching
button works on its first message. Intermediate send values still use soft takeover.
Effect numeric selectors
map to their valid ranges. Existing assignments retain their saved identifiers; no relearning
is needed for these changes.

MIDI input is handled on the UI thread, polling at 8 ms while connected, and forwarded to the
existing player command queue. Controller-only playback retains 1024-frame render calls;
MIDI Clock sync uses 64-frame calls and a separate driver worker. No MIDI driver calls or
allocation run on the audio callback. See [MIDI Clock IN/OUT](midi-clock.md) for sync setup.
Direct MIDI note playing is not implemented.

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

## Right-click MIDI Learn and a maximum per control

Right-click an assignable knob, fader, switch or button and choose **MIDI LEARN...**.
Move the physical controller you want to assign. The same mapping is used by the rack,
detailed editors and the MIDI Controller window. Configure an input port first; if no
controller is connected, Learn opens the MIDI settings. Only CC messages can be learned.

Once assigned, right-click again to **Remove MIDI assignment** or set **MAX** for a
continuous control. MAX is freely adjustable from **0 to 127**, independently for every
assignment. A physical fader at 127 reaches your chosen MAX on the MIDI value scale:
choose 50, 120, 127 or any other value in range. The parameter's own units (dB, Hz, etc.)
still apply. The whole fader travel is
rescaled smoothly; the bottom remains zero. There is no per-assignment MIN setting.
**Reset MAX to 127** restores the full range. MAX can also be edited in the assignments list.

These limits affect MIDI input only; mouse edits remain unrestricted. Device Minimum CC
calibration still happens first, then the per-control maximum. Soft takeover works in the
resulting output range, including when the current value is above a newly reduced MAX.
Assignments and their maxima save automatically and travel together in routing exports.
Old routing files retain the full 0–127 range. Relearning a target keeps its chosen MAX;
removing its assignment removes the associated cap.

## Choosing the independent 909 hat levels

The targets are **TR-909 / CH level (Closed Hat)** and **TR-909 / OH level (Open Hat)**.
Their saved identifiers remain `tr909.ch_level` and `tr909.hh_level`, so existing routing
files stay compatible. Both the new OH label and the old `hh level` search are supported.

An exact filter with only one result selects that target automatically. If a filter hides the
previous selection and several results remain, choose the new target explicitly before Learn.
Changing the filter or target cancels any pending learning so a previous hidden target cannot
capture the next MIDI movement. Check **Waiting for CC** before moving the controller.

Assign different CC/channel pairs to CH and OH. If a pair already belongs to another target,
the assignment confirmation says which routing was replaced.

## A controller stops at value 1 or 2

If **Last input** shows **1** or **2** when a physical knob or fader is fully down, set
**Minimum CC** for that input in the MIDI Controller window. For an UC-33e whose lowest
value fluctuates between 1 and 2, choose **2**. Values at or below the selected minimum map
to exact zero, while 127 remains full scale. The calibration applies to continuous controls;
button presses and switches retain their normal thresholds. Soft takeover uses the calibrated
values. Last input continues to display the raw hardware message for diagnosis.

The minimum defaults to 0 for controllers that already send 0-127. Old minimum-one settings
automatically migrate to a minimum of 2. Each input has its own calibration, saved automatically
on this computer and stays local when importing/exporting routing, since it describes the
connected hardware. It does not change mouse controls or the audio engine. A volume at zero
cuts that volume path; other parameters such as Tune or Decay retain their musical meaning.
