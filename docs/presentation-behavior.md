# Original presentation restored

The normal executable runs the original intro, title, player/password selection,
and tower-loading presentation. Intro and ending artwork is editable PNG with
explicit JSON keyframes. The runtime never loads the reference program. Offline
capture/export tools are research tools, not build or launch dependencies.

The gameplay renderer no longer covers the top 13 pixels with a replacement HUD.
The source stage-clear interval keeps the level visible rather than inserting a
new banner. Game Over displays the original score table and Continue/End choices.
Normal completion shows the time-machine message and offers the hard campaign;
hard completion plays the forest/princess sequence with the selected number of
mages, then displays the original animated final score screen.

## Evidence and checks

- Original outcome routines: `f52d..f5aa` (Game Over), `ebd7..ec72`
  (normal/final score), and `a7c8..a916` (hard ending choreography).
- `d104..d152` formats four individual packed-decimal scores and appends two
  zero digits. Native accounting retains score units; presentation multiplies by
  100. Source font glyphs come from the editable tile PNG. Zero uses letter O,
  as `e3f6` does; tile 246 is an icon, not a zero digit.
- Continue uses `8f6c` and `e1da`, preserving only player/mode/password settings
  while clearing run state. Password table `e24d` restarts towers at stages
  0, 3, 6, or 9. Normal ending confirmation uses `ec62 -> b31b`, which carries
  scores and reserve lives into hard mode.
- Hard ending music hooks observed at original frames 2040 and 3798 after the
  completion trigger at 1650: Fanfare at tick 390 and Dreamy at tick 2148.
- Final stars use the original `a131` algorithm, resetting paired-byte RNG to
  `af84`, moving one pixel per tick, and drawing behind the score glyphs. This is
  native rendering, not a recorded final-screen movie.
- `video::tests::original_outcome_art_and_live_score_digits_match_reference`
  compares every pixel against independent original captures of Game Over with
  four players, normal completion, the four-player hard cinematic, and the final
  hard score screen. It also verifies live score and menu-choice changes.
- `freeze_palette_reference.json` captures 220 original consecutive frames of
  `a399/a3bc` with a Walker frozen for 190 ticks. Native freeze/thaw palette timing
  matches the capture. The source remaps palettes `[0,1,2,3]` to `[2,2,3,2]`,
  flashes during thaw, and restores the original palette. Nonfatal hits use the
  same effect for ten render updates; enemies are not hidden by a replacement
  damage blink.

Outcome research captures used a legal original startup, then selected an outcome
through the original stage-completion/game-over state. These captures establish
presentation behavior, not a claim that those research runs completed a campaign.
`extract_outcomes.py` converts those RGB captures to native assets. Dynamic scores,
selection, input, transitions and star movement remain Rust behavior.

Door entry now walks the entering mage right at 0.5 pixels per tick, switches
behind opaque background pixels within eight pixels of the destination, and
removes the sprite after crossing it. `exit_motion_reference.json` contains two
independent original executions, including gravity, fractional floor contact,
priority and animation timing; the native player test compares each tick. Exit
awards two score units and plays both original exit effects. During the exit countdown, other players, enemies, projectiles and pickups keep
updating. Teammates may still enter the door and earn their own exit award. Team
wipes cannot restart the level during this countdown (source b38f/b3bf/b3d5).

Boss completion uses the recovered half-rate countdown (`b395–b3e2`): ordinary
bosses initialize 255, the final flight boss initializes 70, and exits initialize
128. The current callback frame participates in that countdown, so the remaining
native updates depend on frame parity. Original full-frame probes before frames
1500 and 1501 observed 509/510 updates for ordinary bosses and 139/140 for the
final boss, including the first callback frame's countdown processing.

`d471` replaces actors whose descriptor has bit08 with the short rising
appearance at `c33a`; other actors continue running. The native cleanup preserves
that distinction. The source disappearance lasts 11 updates, rises on alternating
updates, and draws tiles E3/E4/E5 from `c352`. These tiles and the treasure tiles
18–1B are explicit editable sprite definitions, recovered by the asset exporter.

The segmented arena is special. `b723` counts segments hit by player shots. If
all segments leave the arena without a shot, `b6df` spawns a four-unit treasure
at (128,168) and waits for collection; any shot bypasses this reward gate. After
completion `b73a` waits 210 updates before lifting remaining players out of the
screen. `segmented_clear_reference.json` records 335 original full-frame updates
of this sequence. Its probe explicitly resets the camera while retaining the
source collision ring, producing a synthetic floor at y176; the regression test
reproduces that floor rather than treating it as the authored arena geometry.

Boss completion uses the original half-rate counter, including the callback
frame's decrement: regular bosses leave 508/509 updates, and the hard final
arena 138/139. Ordinary doors leave 254/255. Descriptor-marked summoned actors
become the original disappearance effect. The segmented boss has a separate
210-tick script wait, disables vertical terrain collision, then raises the
mages while preserving the fractional velocity byte. Top-of-screen removal
and next-stage timing are compared with `segmented_clear_reference.json`.
That isolated probe's retained collision ring supplies a floor at y176; its
test reproduces that geometry explicitly instead of treating it as the arena
map. If no segments were shot, its special treasure must be collected before
this completion sequence begins.
