# ACID RECALL — Studio guide

[ACID RECALL website](https://acid-recall.netlify.app/) · [MIDI controller setup](https://github.com/smldms/Acid-Recall/blob/feat/studio-v2/docs/midi-controller.md)

## Quick start: MIDI Learn and TR balance

1. Connect your controller and select its input in **Options > MIDI Controller...**.
2. Right-click an assignable rack control, choose **MIDI LEARN...**, then move the
   physical knob, fader or button. Repeat for the controls you want to play.
3. Right-click an assigned continuous control to set its optional **MAX (0–127)** or
   remove the assignment. The physical control's full travel reaches your chosen maximum.
4. Start drum balancing with both **TRIM** controls at **0 dB**. They range from −12 to
   +12 dB and now act after the insert effects. Use each **0 dB** reset button if an older
   project has kept the previous +6 dB 808 boost, then adjust levels for your mix.

Machine and effect ON/OFF buttons follow absolute **0 / 127** CC states; use a controller
preset with latching/toggle buttons. Reverb and delay sends also accept those endpoints.
The sampler's **MIX** strip matches the four original machines, including its green meter.

See [MIDI controller setup](midi-controller.md), [MIDI Learn details](#midi-learn-on-the-rack)
and [TR-808 / TR-909 balance](#tr-808--tr-909-balance) below.

## Graphite Rack

The default desktop view is a native five-module rack: two TBs, the 808, the 909 and an
integrated sampler. SPACE, Delay, Distortion, PCF, Compressor and the stereo master occupy
the right column. Per-machine DIST / PCF / COMP / REV switches share one visual style.
COMP MASTER stays visible in the transport and master strip; it uses the existing compressor
bus selector, so selecting master compression moves that compressor from a machine to master.

Studio keeps the Graphite interface fixed. The obsolete **Mods** menu and mod startup-sound
option are hidden; color customization is not part of this preview.

Each TB has a SIDECHAIN block below its routing buttons, with independent enable and depth.
Bypass retains the depth. TR source, trigger voice and release remain shared, including
sampler ducking. Select BD for kick pumping, or another native voice such as CH or OH.
With BOTH, the labels show the matching native 808 / 909 columns where they differ.
Enabling one TB from global bypass leaves the other TB bypassed. These settings are saved
locally and in Studio projects; existing routing files remain compatible.

Select a sampler lane, Load or Audition it, drag IN/OUT, select its playback mode and edit the
visible bar directly in the rack. EDIT / ALL LANES opens the detailed eight-lane editor for
velocity, copy/paste/repeat, envelopes and less frequent controls. Rack content can scroll in
smaller windows so sequence controls remain reachable. Each machine's EDIT retains its
detailed controls; waveform/synthesis, patterns and effects continue to use the same MIDI IDs.

The TR modules expose their real per-voice controls in aligned columns: Level, Tune/Tone,
Decay, Attack, Snappy and other parameters where that instrument supports them. Unavailable
parameters are left blank rather than represented by inactive or nonfunctional knobs.
Smaller TR knobs leave more space between the control and its label. TB **ARP** applies
the current arpeggiator settings immediately; **Options > Arpeggiator** opens configuration.

## Pattern styles

The generator offers **Techno | IDM | Break**, shared by both TBs and both TRs.

- Techno: quarter-note kicks, offbeat hats, repeating bass motifs and interlocking percussion.
- IDM: irregular pulse distribution, metallic details, ghost notes and wider TB register changes.
- Break: syncopated kicks, backbeat snares and broken-beat / drum-and-bass variations.

These are musical grammars, not copies of any artist's tracks. The selected key and scale,
pattern length, LINK TB harmony, accents, slides and Undo Random continue to work.
The TR patterns retain the original machine's 1–16-step limit.

LINK TB preserves each TB's own pattern length (for example A=8, B=16), while sharing
key/scale and tuning. Set each length in Musical Random before generating from either TB.
The answer considers every bass note it will meet over the complete shared cycle, including
non-dividing lengths such as 7 and 11. In the rare case with no consonant answer, the affected
bass phase moves to the nearest tonic-triad notes while preserving rhythm and accents.
Undo restores both patterns with their original, separate lengths.

## SPACE — stereo convolution reverb

Click **SPACE** in the right FX column, or Options > SPACE / Convolution Reverb.
Turn up one or more machine sends. The **REV** button in each mixer strip toggles its send;
SPACE provides its exact amount. Sampler lanes have their own sends, plus a separate global
sampler send on its mixer strip and in SPACE.

SPACE convolves the audio with a stereo impulse response. Four original **synthetic** spaces
are available: Tight room, Concrete chamber, Warehouse and Diffuse plate. Choose a space and
a duration, then **Build space**. These are generated IRs, not recordings of real rooms or
third-party presets. **Load IR** accepts mono/stereo WAV or uncompressed AIFF responses up
to six seconds. Imported IRs are energy-normalized.

Controls: Return, Pre-delay (0–200 ms), Stereo width, Low cut and High cut. Sends are
post-insert, post-sidechain and post-fader. Turning a send down lets the existing tail decay.
The return is added before the master compressor and master fader. IR changes crossfade;
rapid requests keep the latest prepared response queued behind the current fade.

PCF, Delay, Distortion and Compressor retain their existing sound and routing. Click their
cards to edit all their controls, including PCF faders/pattern/mode. The Delay card now
exposes **STEPS (1–32)** and **STRAIGHT / TRIPLET** directly, alongside Pan and Feedback.
Turn the STEPS knob; hold Shift for fine adjustment. The detailed editor uses the same controls;
its formerly unresponsive step buttons are fixed. Both timing controls support MIDI Learn.

## Sampler — eight lanes, up to 64 bars per lane

Each selected sample has **PITCH**, **LEVEL** and **REVERB** knobs on the rack. Drag
vertically, with **Shift** for fine adjustment. Pitch uses whole semitones; Level and
Reverb remain continuous. Right-click MIDI Learn and saved assignments are unchanged.

A white **playhead** follows the most recently triggered active voice in the selected
lane. The rack and detailed sampler editor show its position in the original sample;
the WARP editor shows its corresponding position on the musical grid. Reverse, trim,
pitch, warp markers, looping and audition follow the engine's playback position. The
head disappears when playback of that lane ends, is stopped or its sample is removed.
Audition keeps the head moving even when the sequencer is stopped or the sample is silent.

Right-click a sample's **name** in the rack or detailed editor and choose **Unload sample**
to free that lane's audio. Its sequence and lane settings stay available for another sample.

### Undo / Redo

The sampler and WARP editor provide **UNDO / REDO** buttons. Keyboard shortcuts are
**Ctrl/Cmd+Z** to undo, **Ctrl/Cmd+Shift+Z** or **Ctrl+Y** to redo. They apply to sampler
settings, sequences, trim, WARP anchors, sample loading and unloading. A mouse drag counts
as one edit. Other machines and global effects have separate controls and are not included
in this sampler history. Text fields retain their own text-editing shortcuts.

History belongs to the current session and resets when opening or creating a project.
It holds up to 32 edits, with a 480 MiB budget for distinct audio retained by the current
sampler and its history; older entries are discarded first. Undoing a sample replacement
or unload prepares its audio in the background. Editing waits until that restoration ends;
voices on replaced lanes stop and the restored sample plays on its next trigger or audition.

### WARP: beat matching, musical grid and local stretch markers

1. Trim **IN / OUT** to the loop you want to use. Set **Beats** to its musical length:
   four quarter notes for one bar in 4/4, sixteen for four bars.
2. Enable **WARP** or press **FIT**. The selected region now lasts exactly that many beats
   at the song tempo, retaining its pitch. For example, four beats last 1.6 seconds at
   150 BPM. Changing Beats or the song tempo updates the duration live.
3. Open **MARKERS / GRID** beside WARP. The ruler is numbered **measure.beat** (1.1, 1.2,
   1.3, 1.4, 2.1...). Strong vertical lines mark bars, medium lines mark beats and light
   lines mark sixteenths. Dense labels thin out at long lengths; zoom reveals them again.
4. **Double-click a transient** to anchor its original audio position. Drag the anchor
   line to move that sound to another musical time. The waveform redraws with the local
   stretch; the surrounding endpoints remain fixed. Up to **16 anchors per lane**.
5. **Snap 1/16** aligns destinations to sixteenths. Disable it or hold **Shift** for free
   placement. Zoom up to **16×** and scroll horizontally for precise work. The zoom uses
   finer waveform detail, including when IN/OUT selects a small region of a long file.
6. Select an anchor to see its source time and edit its **Beat offset** numerically
   (zero = the beginning of the region). Use **Remove selected**, Delete while the
   waveform has focus, or **Reset markers** to return to a linear beat fit.

Warp anchors use the interface's **acid green**; the selected anchor is thicker. The
playhead stays **white**, so playback remains distinct from editing markers.

For a starting point, press **ANALYZE BPM / TRANSIENTS**. Analysis runs in the background on the
selected IN/OUT region and proposes up to 16 separated attacks. Dim green lines preview
these suggestions. After fitting to Beats, **APPLY SUGGESTIONS** adds anchors at their
current musical positions, preserving existing anchors and their timing. The lane still
has a total limit of 16 anchors. Move the new anchors yourself to adjust timing, or use
**UNDO** to revert the addition. **Dismiss analysis** removes the preview.

The **Est. BPM** row shows an optional estimate from rhythmic attacks in IN/OUT. Use
**÷2 / ×2** to choose half-time or double-time (20–400 BPM). The initial search covers
50–200 BPM. Short, silent or irregular audio may yield no reliable estimate; rhythmic
subdivisions can suggest a different octave. Always check by ear. This is a tempo estimate,
not a downbeat or time-signature detector.

**USE ~N BEATS** rounds the region's estimated duration to the nearest whole beat (1–256),
then enables the existing beat fit. It preserves anchors and song tempo, and can be undone.
This works best on trimmed loops containing complete beats; adjust Beats manually for
an incomplete phrase. Analysis and the half/double buttons alone never change playback.
Applying transient suggestions keeps the BPM estimate available.

Both mouse and MIDI WARP/FIT use the beat fit. Beats is independent from the sequencer's
Length in bars. This fits the musical length you confirm. Use the correct beat count for
your sample, entered manually or chosen after checking the optional estimate.

Older projects retain their saved source-BPM behavior. In that mode, **Src BPM** remains
20–400 BPM. For a known 120-BPM recording, enable WARP, choose **BPM mode**, then enter
120. Editing Src BPM by MIDI also selects this mode when there are no anchors. Opening
the marker editor does not silently convert an older project: press **FIT TO BEATS**
before editing its preview grid. Once anchors are present, Beats defines the duration.

Pitch remains independent from duration while warped. Turning WARP off restores ordinary
sample playback, where transposing also changes duration. Loop, One Shot, Auto Cut, Gate
and Reverse remain available; Reverse mirrors the anchor map in time. Anchors use absolute
source positions; trimming removes anchors outside the new region and preserves those inside.

The built-in time-domain engine uses stereo waveform-similarity overlap-add with a fixed
grain/search budget per voice. It reads the original PCM and does not allocate memory,
copy whole stretched files or decode audio on the callback. Both stereo channels share
grain alignment. Warp settings and anchors save in portable `.orb338` projects. Loading
a replacement sample resets Warp, beat fitting and anchors instead of reusing another
recording's map.

Ableton's proprietary Complex/Complex Pro processing is not included. Dense polyphonic
material and extreme tempo changes can color transients and timbre; use moderate ratios
for more natural results. Tempo changes remain live and do not require rebuilding a sample.

MIDI targets per lane: **warp** (On/Off), **warp_bpm**, **warp_beats**, and **warp_fit** (button).
MIDI Beats selects musical lengths 1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192
or 256; press FIT after choosing the count. The mouse editor accepts all integer counts.
Existing MIDI calibration, pickup, lane reverb, gater and sidechain remain available.

Design references: [Ableton's Warp concepts](https://www.ableton.com/en/manual/audio-clips-tempo-and-warping/)
and [SoundTouch's description of time-domain WSOLA](https://www.surina.net/soundtouch/README.html).
The implementation here is built into ACID RECALL; neither library nor Ableton code is bundled.

The sampler has a global ON/OFF switch (MIDI target **sampler.on**) that retains its level
and lane settings. OFF smoothly silences the bus and new sends while existing effect tails
decay; its playheads keep time so it resumes in sync. Old projects default to ON.
The sampler has a global mixer strip with Level, stereo signal meters, Pan, DIST routing,
Delay send and REV send. DIST and Delay use the same shared effects configured in the FX
column; enable those effects there as for the other machines. MIX level and pan act on
the summed lanes and their reverb sends without changing each lane's own settings. Global
REV adds another send and does not replace individual lane REV amounts. Existing effect
tails decay when sends or volume are lowered.

Each sample name has **S** (Solo) and **M** (Mute) immediately to its right. These controls
work without selecting that lane. **REVERSE** is visible beside the playback modes in the
integrated sampler, and still follows its MIDI assignment and saved trim points.

Click **SAMPLER**, or Options > Sampler / 64-bar sequencer.

1. Choose a lane and click **Load**. WAV PCM 8/16/24/32-bit, WAV float 32/64-bit,
   uncompressed AIFF and MP3 are supported, mono or stereo. Imports are resampled to 44.1 kHz.
2. Choose **Length**, from 1 to 64 bars. Each bar contains sixteen steps: 1,024 steps at
   the maximum length. The eight lanes can have different lengths.
3. Click a step to toggle it; right-click to edit velocity (0–127).
4. Choose a bar in the overview. Disable Follow to edit another bar during playback.
   Copy/Paste/Clear operate on the selected lane's current bar; Repeat fills its chosen length.
5. Press Play in the sampler or on the main transport. Playback uses the same engine clock.

Modes:

- **One Shot:** plays IN to OUT across sequencer/pattern wraps; up to four overlapping voices
  per lane. Existing voices finish before further triggers can use their slots; excess
  triggers are ignored. Transport Stop and explicitly selected choke groups still stop voices.
- **Auto Cut:** a new trigger cuts that lane's previous voice with a short fade.
- **Gate:** releases after the chosen duration, expressed in sixteenth-note steps.
- **Loop:** repeats the selected region until retrigger, choke or Stop. Short seam fades
  reduce clicks while retaining the region's duration.

Each lane has Mute, Solo, Level, Pan, Pitch (±24 semitones), Reverse, Start/End trim,
Attack, Release, Gate and a reverb send. Pitch uses **varispeed**: it changes pitch and
duration together, not a phase-vocoder or tempo-warp algorithm. Choke 0 is independent;
lanes in the same group (1–8) cut each other. Audition works with the transport stopped;
Stop also ends an auditioned loop.

There is no three-minute sample-duration limit. WAV, AIFF and MP3 use the same memory budget:
240 MiB of decoded stereo audio for the whole bank (about 11 minutes 53 seconds at 44.1 kHz
if one sample fills it). Import also bounds its intermediate decoded buffer to 240 MiB;
high sample-rate files may therefore use that budget sooner. Source files are limited to
256 MiB. Exceeding a memory budget reports an error and leaves existing audio intact.
Compressed AIFF/AIFC and multichannel audio are not supported. Audio-file decoding,
resampling and IR preparation run in the background.

### Sample region and rhythmic modulation

Drag the waveform's **IN / OUT** markers, or enter their times in seconds below the display.
Clicking moves the nearest marker; double-clicking restores the whole file. The markers cannot
cross. They affect playback and export, including reverse and loop modes, and are saved in
the project. Moving markers also changes the region of an already playing voice.

Each lane's **GATER** selects TR-808 or TR-909 and an instrument (BD, SD, CH, OH and others).
Real instrument hits open the gater, including 909 flams; it is independent of the TR and
instrument mixer volume. Depth sets the closed level; Hold chooses a tempo-synchronized
duration from 1/128 to one bar; Smooth sets the closing ramp. The detailed editor also offers
an opening ramp. The sampler playhead keeps moving while closed, chopping a vocal without
restarting on each drum hit. The source pattern must play an actual note. Old TB-gate source
IDs remain supported for existing projects and MIDI routing; new UI selections use the TRs.

**Sidechain** independently opts each lane into drum-triggered ducking, before dry mixing and its
reverb send. It follows the global sidechain On, TR source, selected voice and release with a separate
65% sampler depth; TB depth controls do not change sampler ducking. Gater transitions do not
slow down the kick envelope. Both effects are included in offline export and saved in projects.

## MIDI

CH and OH are independent in the desktop app: both notes can occupy the same step,
neither voice chokes the other, and the 909 closed-hat level stays separate from OH.
This applies to playback and desktop audio export; the standalone legacy renderer retains
its original compatibility mode unless independence is explicitly enabled.

The existing MIDI Learn and pickup work with the new controls:

- SPACE: Return, Pre-delay, Stereo width, Low cut, High cut and four machine sends.
- Sampler: Level, Pan, Pitch and Reverb send for each of the eight lanes.

Search **SPACE** or **Sampler / Lane N** in MIDI Controller. Existing mappings keep their IDs.
The per-input minimum-CC calibration applies to the new knobs as well.

## Saving and exporting

Once Studio content is used, **Save / Save As** writes a portable **.orb338** project. It
embeds the original RBS song, samples, IR, all sampler steps, settings, sidechain and
independent closed-hat level. Open it through File > Open. The versioned container validates
section sizes and checksums before replacing the current song. A standard RBS opened and
edited without Studio content continues to save as RBS.

MIDI routing remains an application/controller preset; use MIDI Import/Export to move it.
Custom RBM drum kits remain external, as with ordinary RBS songs.

**Export Loop as Audio File** includes Studio's sampler and reverb with the same DSP as
live playback. Set the main song loop to the desired export length, e.g. 64 bars, and use
song mode as required by the existing export command. Audio export ends at that loop's end;
it does not append an extra reverb tail.

## Implementation and validation

The engine uses nonuniform FFT partitioned FIR convolution: the first 2,048 taps have
64-frame partitions, and the tail has 2,048-frame partitions. It adds no DSP lookahead delay.
The existing audio device's buffering still applies. Plans, spectra and sample buffers are
prepared off the audio thread; replaced resources return to the UI for destruction.

Sampler playback reads PCM directly at its original pitch and uses a precomputed 16-tap
bandlimited interpolation table for transposition, fixed voice
pools, smooth level/pan changes and bounded cut fades. Step crossings are resolved within
each 64-frame engine block. The original machines retain their original block scheduling.

Waveform caches are owned by the nine fixed display slots, each at most 2,048 peaks. DSP
ramps and filters discard inaudible residuals before they become subnormal values. The
convolution processor sleeps after its maximum possible tail has fully drained. Regression
checks include two minutes of playback/retrigger/silence and bounded repeated waveform updates.

MP3 decoding uses Symphonia 0.5.5 (MPL-2.0), entirely on the import worker. Source and license:
https://github.com/pdeljanov/Symphonia/tree/v0.5.5. The portable package includes its license.

Tests cover direct-FIR agreement across partition/history boundaries, audio import bounds,
sampler modes/chokes/stop, the last step of bar 64, precise step crossings, portable project
integrity, genre/harmony constraints and the real player's allocation/free guard.

## MIDI controllers and Studio

Options > MIDI Controller can listen to several Windows input devices simultaneously.
Input 1 preserves the existing controller and assignments. Use **Add MIDI input** for
Input 2 and further slots, choose a different device in each, then Learn as usual. The next
CC identifies both its controller slot and channel, so two devices may use the same CC
number for different parameters. Unavailable inputs do not stop the other inputs.
Keep the same slot order when moving a routing preset between PCs; device names and each
input's minimum-CC calibration stay local. A disconnected slot keeps its assignments.

Set each input's **Minimum CC** to the lowest value its knobs/faders send (0-16). Values at
or below that threshold become exact zero, and 127 remains full scale. Existing minimum-one
calibration migrates to a two-count tolerance, so an UC-33 returning 1 or 2 at its physical
minimum still mutes. Ordinary uncalibrated inputs preserve low values such as CC 1. The
controller's own display continues showing its raw value; only the software value is remapped.

Studio MIDI targets cover SPACE enable, return, filters, pre-delay, width and four sends,
sampler global level/pan/distortion/delay/reverb (`sampler.level`, `.pan`, `.dist`, `.delay`, `.send`),
plus every lane's gain, pan, pitch, send, trim, envelope, gate, mode, reverse, mute, solo,
choke, gater source/instrument/hold/depth/attack/release and kick sidechain. TB sidechain
enable/depth, shared release, TR source and voice (`duck.instrument`) have their own MIDI targets.
Audition responds once per
CC button press (64-127 pressed, 0-63 released). Boolean controls use that same absolute
threshold, while continuous controls retain pickup. Quantized pitch and mode selectors
preserve the controller position until an external UI or project change requires pickup.

## MIDI Clock IN / OUT (Windows)

Open **MIDI > MIDI CLOCK / SYNC**. Choose **MASTER / OUT** to send tempo and
Start/Continue/Stop to an external machine, or **SLAVE / IN** to follow its clock and
transport. Clock uses 24 pulses per quarter note; Song Position Pointer locates in sixteenth
notes. All four original machines and the sampler share the transport. Enable **TIME WARP**
on samples that should follow tempo while preserving pitch.

Select the corresponding MIDI port. The OUT delay (0–500 ms) can compensate audio output
latency; new configurations default to 30 ms. A signed **IN offset** (−500 to +500 ms,
default 0) advances playback with negative values and delays it with positive values.
This converges gradually during playback and cannot predict the first external Start.
Clock loss in slave mode stops playback; a new external Start or Continue resumes
it. MIDI Learn and controller assignments remain available. IN and OUT are exclusive modes.

See [MIDI Clock setup and limits](midi-clock.md) for hardware connections and transport
behavior. This preview needs real-hardware timing feedback from testers.

## MIDI Learn on the rack

The sampler's **MIX** strip uses the same compact fader, green ten-segment meter and
control geometry as the four original machines. Its meter displays the higher L/R peak
in one column; audio remains stereo. The MIDI target stays `sampler.level`, so existing
assignments, saved gain, custom MAX values and portable projects continue to work.

Machine power and effect ON/OFF/routing buttons follow absolute **0 / 127** CC states,
just like sampler ON/OFF. A controller in latching/toggle mode changes state on every press.

Right-click an assignable control, choose **MIDI LEARN...**, then move your physical
controller. Right-click again to remove the assignment or choose its **MAX** (0–127).
Each continuous control has its own freely chosen maximum: a fader fully up reaches that
value and fully down reaches zero. MAX defaults to 127; no per-assignment MIN is needed.
Limits save with routing presets and affect MIDI only. See [MIDI controller setup](midi-controller.md).

## TR-808 / TR-909 balance

Each drum mixer has a **TRIM** control from −12 to +12 dB, also assignable through MIDI
Learn. New Studio sessions start at **0 dB on both drums**, preserving the original
instrument levels. The previous automatic +6 dB boost on the TR-808 has been removed:
voices, patterns and imported kits have different dynamics, so a fixed boost is not a
reliable balance for every mix. Adjust TRIM to taste and leave headroom at the master
when stacking loud sounds or boosting a channel.

Trim changes are smoothed and apply **after distortion, PCF and compression**, before
the meters, delay and reverb sends. Changing TRIM therefore adjusts the output level
without pushing the insert effects harder. Each TR has a **0 dB** button to reset its
trim, including a +6 dB value saved by a previous preview. Saved values are not silently
overwritten. They save in portable `.orb338` projects. Older Studio projects without trim metadata
load at **0 dB on both drums**, preserving their previous mix. Legacy engine renders
without Studio retain their original arithmetic and output.
