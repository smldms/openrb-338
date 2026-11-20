# ACID RECALL

![ACID RECALL logo](../assets/brand/acid-recall.png)

**The memory returns. The machines come alive.**

**[Visit the ACID RECALL website](https://acid-recall.netlify.app/)**

ACID RECALL is a performance-focused fork of **[OpenRB-338 by pingw33n](https://github.com/pingw33n/OpenRB-338)**,
the open-source reimplementation of ReBirth RB-338. It builds on that foundation with hands-on
MIDI control, MIDI Clock IN/OUT sync, musical pattern generation, two arpeggiators,
an integrated sampler and a graphite rack.

Two acid bass lines. An 808. A 909. Eight sample lanes. A controller under your hands.

<p align="center">
  <a href="https://github.com/smldms/Acid-Recall/releases/download/windows-preview/ACID-RECALL-Windows-x64.zip">
    <img src="https://raw.githubusercontent.com/smldms/Acid-Recall/feat/studio-v2/assets/brand/download-windows.svg" alt="DOWNLOAD — Windows x64 portable ZIP" width="300" height="64">
  </a>
  <br>
  <sub>Windows x64 · Free preview · No installation · No GitHub account required</sub>
</p>

## Latest preview — 20 November 2026

- **Safer sampler editing:** UNDO / REDO for sampler and WARP edits, including sample
  loading and unloading. Right-click a sample name to **Unload sample**.
- **WARP assistance:** acid-green anchors, a white playhead, and **ANALYZE BPM / TRANSIENTS**.
  Review the estimated BPM, adjust **÷2 / ×2**, then choose **USE ~N BEATS** to fit the loop.
  **APPLY SUGGESTIONS** adds optional transient anchors; existing anchors stay.
- **One Graphite interface:** the obsolete Mods menu is hidden in Studio.

- **Sampler playheads and knobs:** live source-position heads on the rack and sampler
  editor, plus a musical-position head in the WARP grid. Pitch, Level and Reverb per sample
  and Delay Steps now use knobs, with Shift for fine adjustment and existing MIDI mappings.
- **Delay timing on the rack:** STEPS (1–32) and STRAIGHT / TRIPLET are directly accessible,
  MIDI-assignable and shared with the corrected detailed editor.
- **Sampler WARP grid:** fit the trimmed sample to Beats, then use MARKERS / GRID to place
  up to 16 local stretch anchors on a numbered bar/beat/sixteenth grid. Includes snap,
  free placement, 16× zoom and portable project saving. Enter Beats yourself or review
  the optional BPM estimate and proposed beat count before applying them.
- **MIDI Learn directly on the rack:** right-click an assignable control, choose
  **MIDI LEARN...**, then move your controller. Right-click again to remove the assignment
  or choose a personal **MAX from 0 to 127** for each continuous control. Your full fader
  travel reaches that maximum; mouse edits remain unrestricted.
- **TR-808 and TR-909 output TRIM:** −12 to +12 dB, smoothed and MIDI-assignable, with
  **0 dB on both TRs by default**. Trim now follows the insert effects instead of driving
  them harder. Use each **0 dB** button to reset a trim saved by an earlier preview.
- **Consistent MIDI buttons:** machine power and effect ON/OFF/routing follow absolute
  **0 / 127** CC states. Use a latching/toggle controller preset. The sampler now has the
  same **MIX** fader and green ten-segment meter as the four original machines.

Start with the [MIDI controller guide](https://github.com/smldms/Acid-Recall/blob/feat/studio-v2/docs/midi-controller.md)
or the [TR balance guide](https://github.com/smldms/Acid-Recall/blob/feat/studio-v2/docs/studio.md#tr-808--tr-909-balance).

## Start playing

The ACID RECALL Studio features described here are currently developed on
[`feat/studio-v2`](https://github.com/smldms/Acid-Recall/tree/feat/studio-v2).
The default branch still contains the upstream application code while integration is in progress.

- **[Download the Windows preview ZIP](https://github.com/smldms/Acid-Recall/releases/download/windows-preview/ACID-RECALL-Windows-x64.zip)**:
  extract the ZIP and launch **ACID-RECALL.exe**. No installer or GitHub account is needed.
  The download updates after successful Studio builds; it does not expire after 30 days.
- **[Release notes](https://github.com/smldms/Acid-Recall/releases/tag/windows-preview)**
  identify the current build. For previous test builds, see
  [GitHub Actions](https://github.com/smldms/Acid-Recall/actions/workflows/midi-windows.yml).
- **[Read the Studio guide](https://github.com/smldms/Acid-Recall/blob/feat/studio-v2/docs/studio.md)** for sampler, effects, MIDI setup and project limits.
- **[Visit the original OpenRB-338 project](https://github.com/pingw33n/OpenRB-338)** for the
  upstream implementation, its releases and documentation.

## The story — finding that first spark again

ReBirth was the first music software I ever installed on my PC.

That is where this story starts: with the discovery that a computer could become a place
to make music. Years later, finding OpenRB-338 brought that feeling back. Someone had
recreated ReBirth, and I was thrilled. I could play with those familiar machines again.

There was another part of that reunion: my old Evolution UC-33e controller.
Its revival is thanks to my friend **[MATTH23D](https://x.com/matth23d)**, who is skilled at restoring this kind of
older equipment. He also builds **analog x0xb0x synthesizers — TB-303 clones**.
He brought the UC-33e back into action, and I wanted to put its faders and knobs to work.

The first idea was simple: connect the controller and play the machines with my hands.

That became a fork, MIDI input and a few assignments. I am not a software developer;
I work with AI assistance to turn musical ideas into features, try them out, identify
what gets in the way and improve them.

One idea led to another. Generate bass lines that stay in harmony. Let one 303 answer
the other. Give each one an arpeggiator. Run eight steps on one and sixteen on the other.
Make room for the kick.

Then came the sampler, vocals chopped to the hats, convolution reverb and a graphite
interface with the controls in plain sight. The software started to follow the way
I want to make music today.

ACID RECALL grew out of that process: rediscover a first spark, reconnect the hardware,
and see where the next session takes it.

## What this fork adds

OpenRB-338 provides the original reimplementation and compatibility work.
The additions below belong to the ACID RECALL Studio preview built on top of it.

| Area | ACID RECALL additions |
|---|---|
| Hands-on control | Right-click MIDI Learn and removal, a freely chosen MAX (0–127) per continuous control, multiple inputs, soft takeover and per-device calibration. |
| MIDI Clock sync | Windows MASTER / OUT and SLAVE / IN, 24 PPQN, Start/Continue/Stop, Song Position Pointer, clock-loss stop, 30 ms default OUT delay and signed IN offset. |
| Drum balance | Smoothed, MIDI-assignable output trims after the inserts; both TRs default to 0 dB, with a quick 0 dB reset. |
| Portable mappings | Import and export MIDI assignments to move between computers. |
| Pattern generation | Techno, IDM and Break styles, with notes, accents, slides and Undo. |
| Linked bass lines | Harmonically linked TB patterns with independent lengths, such as 8 and 16 steps. |
| Arpeggiators | One per TB, using a chosen chord or existing pattern notes; ARP applies the current settings immediately. |
| Sidechain | Ducking triggered by a selected 808 or 909 voice, with independent depth for each TB. |
| Independent hats | Separate TR-909 CH/OH notes and levels. |
| Convolution reverb | Stereo convolution, synthetic spaces and WAV/AIFF impulse-response import. |
| Sampler | Eight lanes, sequences up to 64 bars, WAV/AIFF/MP3, IN/OUT trimming and Reverse. |
| Sample warp | Pitch-preserving beat fit, numbered musical grid, local stretch markers, sixteenth snap and 16× waveform zoom. |
| Delay timing | Direct STEPS and STRAIGHT / TRIPLET controls, also available to MIDI Learn. |
| Sample performance | One Shot, Auto Cut, Gate and Loop, with Solo/Mute beside each sample name. |
| Rhythmic gating | A gater driven by a selected TR voice, tempo-based Hold, smoothing and optional sidechain. |
| Sampler mixer | MIDI-assignable ON/OFF, a MIX strip matching the four machines with a compact fader and ten-segment meter, pan, distortion, delay and global reverb. |
| Interface | A graphite rack with visible TR voice parameters, an integrated sampler and a permanent FX column. |
| Portable sessions | `.orb338` projects containing samples, impulse responses and Studio settings. |

Sampler, reverb and sidechain controls can be mapped to MIDI. The UC-33e inspired the first
steps; the mapping system is not limited to that controller.

## Why ACID RECALL?

**ACID** points to the bass lines, sequencers and musical territory of the project.
**RECALL** is about bringing back a memory and calling the machines into play again.

The AR monogram incorporates a recall arrow. Graphite and acid green carry the same idea
into the interface: a dark workspace, clear active states and restrained decoration.

## Short description

ACID RECALL is a fork of OpenRB-338 that extends the spirit of ReBirth with hands-on MIDI
control with right-click MIDI Learn and custom per-control maxima, MIDI Clock IN/OUT
synchronization, musical pattern generators, two arpeggiators and an integrated sampler.
Smoothed TR output trims help balance the drums while preserving their insert-effect drive.
A graphite rack for rediscovering familiar machines and building new sessions.

## Launch copy

ReBirth was the first music software I ever installed on my PC. Discovering OpenRB-338
years later made me want to play those machines again. My friend [MATTH23D](https://x.com/matth23d) had brought
my old UC-33e back to life, so I started by connecting it. Then came harmonically linked
patterns, arpeggiators, sidechain, a sampler, a new interface and MIDI Clock IN/OUT
sync with external machines.
This fork is now called ACID RECALL. The memory returns. The machines come alive.

## Current scope

This is an evolving standalone rack and sequencer. **Windows MIDI Clock IN/OUT is now
available in the Studio preview.** Choose **MASTER / OUT** to send tempo and transport to
external machines, or **SLAVE / IN** to follow their tempo and Start/Continue/Stop.
Song Position Pointer supports locating; an OUT delay setting helps compensate audio latency.
The four original machines and the sampler share the synchronized transport. Sampler
**TIME WARP** follows tempo while preserving pitch.

Open **MIDI > MIDI CLOCK / SYNC**, select one mode and the appropriate MIDI port.
Controller assignments and MIDI Learn remain available alongside sync. Clock input and
output modes are exclusive to avoid feedback. This is MIDI Clock, not MTC or audio sync.
See the [MIDI Clock setup guide](https://github.com/smldms/Acid-Recall/blob/feat/studio-v2/docs/midi-clock.md)
for connections, transport behavior and limits. Automated tests pass; real-world timing
still needs testing with external hardware.

The sampler loads audio into memory. File-size and memory limits are explained in the
[Studio guide](https://github.com/smldms/Acid-Recall/blob/feat/studio-v2/docs/studio.md). Windows preview builds provide the current ACID RECALL experience;
the upstream project's release packages are a separate distribution.

## Credits and thanks

- **[pingw33n and the OpenRB-338 contributors](https://github.com/pingw33n/OpenRB-338)**:
  the original open-source reimplementation, engine and historical compatibility work.
  ACID RECALL builds on their work.
- **[MATTH23D (@matth23d)](https://x.com/matth23d)**: for restoring my Evolution UC-33e and keeping older music hardware alive,
  alongside his work building analog x0xb0x TB-303 clones.
- **Propellerhead's ReBirth RB-338**: the software that started this story.
- **[smldms / ACID RECALL](https://github.com/smldms/Acid-Recall)**: this fork's musical direction, additions and continuing development,
  with AI-assisted implementation.

The code remains licensed under **GPL-3.0-only**. See LICENSE in the repository root.
