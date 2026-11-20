# TR Sidechain and independent 909 Closed Hat level

Click **SC** beside the transport's Loop button, or choose **Options > Kick Sidechain...**.
Enable ducking, choose the 808, the 909, or both, select a voice (BD, SD, CH, OH...) and set the depth separately
for TB-303 A and B. Depth 0 leaves that TB unchanged; 100 fully ducks it at the deepest point.
Return sets the recovery time (20–1000 ms). Start around 50–65% with 150–220 ms return.
The gain-reduction bars show what the audio processor is doing.

The trigger follows actual notes of the selected voice, including shuffled steps and 909
flams. Other voices do not trigger it. Voice level does not change its logical triggers;
muting the whole drum machine or setting its mixer level to zero disables its trigger.
Old settings default to BD. Changing voice does not replay a hit from earlier playback.
A short smoothed attack, a 10 ms hold and a smooth recovery leave space for the kick without
hard gain jumps. Each TB is ducked after its inserts and before its dry mix and new delay sends.
Existing delay tails continue naturally. Shared master compression can still react to the
changed overall mix. Disabled and zero-depth operation preserve the original rendering.

The TR-909 now has a **LEVEL** knob above **CH DECAY**. In MIDI Controller, learn
**TR-909 / ch level** to assign it to the UC-33e or another controller. The existing level in
the OH column controls the Open Hat. Editing either hat opts into independent CH/OH levels.
Until either is edited, older songs retain their original shared-level behavior. The new CH
control supports the same minimum-one calibration and soft takeover as other knobs.

These additional mix settings are saved automatically with the app preferences and included
in WAV/FLAC/Ogg audio exports. The standard `.rbs` format is unchanged: these extra settings
are not stored in the song and are not recorded as legacy ReBirth automation. They remain
active when changing songs, so check them when loading another track. Standard songs and
CLI reference renders keep both extensions bypassed by default.
