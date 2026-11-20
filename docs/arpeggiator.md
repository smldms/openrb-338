# TB arpeggiators

Click **ARP** on either TB to apply its current arpeggiator settings immediately.
Use **Options > Arpeggiator** to configure the chord, direction, length and other settings.
Each TB has independent settings and an independent one-level Undo.

Choose **Chord** to select a root and chord, or **Current pattern** to extract the distinct
active notes from the selected pattern. Rests and steps beyond its current length are ignored.
An empty pattern needs notes first, or use Chord instead.

Choose Up, Down, Up/Down, Down/Up, Source order or Random. Bouncing directions do not repeat
the endpoints. Random is repeatable until **New random variation** changes its seed.
Start offset moves the first note in the ordered sequence; it is disabled for Random.

Set the output length (1–16 steps), octave expansion (1–3), rhythm, accent interval
(0 disables accents), and optional slides between adjacent active notes.
Rhythm uses the existing sixteenth-note sequencer: 1/8, for example, places a note every two
steps with a rest between them. Slides do not cross rests, and a one-step pattern cannot slide.
Octave expansion is shifted by whole octaves to fit the original TB register; any remaining
out-of-range pitches are omitted. The preview lists the actual note pool and output steps.
Octave labels are relative to the TB base register, and its Tune setting still affects pitch.

The preview changes no sound or pattern. **Apply to pattern** replaces the currently selected
pattern on that TB using the normal engine command path. If playback is running, the updated
pattern is heard as the sequencer plays it. It is saved normally in an `.rbs` song.
There is no separate live MIDI-note arpeggiator mode.

**Undo arpeggio** restores the previous pattern, including its original length and shuffle.
It also works after selecting another pattern. Editing the generated pattern disables Undo
so it cannot overwrite your later edits. Opening or creating a song clears both Undo histories.

The other TB, drum patterns, sound controls, tuning and transport are unaffected by Apply.
The Random generator's LINK TB switch does not link the arpeggiators. Choose the same root and
compatible chords on both TBs to build complementary lines yourself.
