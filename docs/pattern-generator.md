# Musical Random and LINK TB

Each of the four pattern sections has an **RND** button. It replaces that machine's selected
pattern slot, using the length already shown by its **STEPS** display (1-16). Select a slot
and length first, then click RND. The changes can be heard while playing and save normally
in the `.rbs` song. Drum sample kits and sound/effect settings are kept.

Open **Options > Pattern Generator...**, or right-click an RND button and choose **Generator
settings...**, to choose a key, scale, note density, accent/slide probabilities and variation.
Scales: natural minor, Dorian, Phrygian, minor pentatonic and major. The default is D minor.

TB patterns build a short recurring motif, weighted toward root/third/fifth on strong beats.
Rests, octave moves, accents and slides add variation. Slides connect active notes, including
the loop boundary, and inactive steps beyond the requested length are cleared.

## LINK TB

The two **LINK** buttons show the same shared state. When enabled, clicking RND on either TB
generates both selected TB slots together. They use the clicked TB's length, shuffle and tuning, and
the generator's common key/scale. TB A is a bass call; TB B answers in its gaps in a higher
register. When both sound on a step, pitch intervals avoid seconds and tritones.

The TBs keep their own envelopes, filter settings and effects. Matching their tune controls
ensures the generated scales also agree audibly. Turn LINK off to randomize only one TB.

## TR-808 / TR-909

Choose **Techno / four on the floor** or **Broken**, then set drum density. Kick anchors,
backbeats, hats and percussion are placed in structured rhythms, with random variation.
909 hits use normal/accent/flam values; its open and closed hats never occupy the same step.
Each machine's selected slot is independent; LINK affects the TBs only.

## Undo Random

Right-click RND, or open the generator window, then choose **Undo Random** to restore the
patterns before the last generation. Linked generation restores both TBs and their previous
tuning together. It is one generation deep. Undo is disabled if those patterns/lengths or
the linked tuning have since been edited; opening a new song also clears it.

For ongoing variations you want to keep, save your song or use a different pattern slot
before generating another version.

## MIDI routing files

In **Options > MIDI Controller...**, **Export routing...** writes a portable `.toml` file
containing all learned CC/channel assignments and the soft-takeover preference.
**Import routing...** validates and replaces those assignments. On another computer, choose
its MIDI input locally: port names and MIDI activation are intentionally local preferences.
Unknown targets, conflicting assignments, unsupported versions and damaged files are rejected
before changing your current assignments. Existing MIDI configuration remains compatible.
