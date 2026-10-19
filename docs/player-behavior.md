# Player behavior evidence

Static source: the original 32 KiB program region, SHA-256
`360cc9ac03244ba14a7452031fcd9da3a10aa69fed2c84e201c240d689eb4adb`.
Addresses below refer to its original `$8000` mapping. The local investigation
retains `analysis/game-disassembly.txt` and `analysis/game-findings.md`.
These are static observations, not a claim of full-game execution equivalence.
REA's original raw-program decompilation did not recover these functions;
these findings come from the separately decoded instruction listing. Existing
REA PC-wrapper evidence does not establish the gameplay conclusions below.

## Coordinates and timing

The original uses signed 8.8 velocity and unsigned byte positions with separate
fractional bytes. Object X/Y integer components are `$004a` / `$0360`; fractional
components `$0220` / `$0240`; horizontal velocity high/low `$0260` / `$0280`,
vertical velocity high/low `$02a0` / `$02c0`. `$a3ee` and `$a41b` add velocity to
position with fractional carry. `$8600` subtracts camera offset `$cc` from Y.
The replacement uses named `Body` fields and pixels per fixed 60 Hz update.

## Recovered movement

| Behavior | Recovered rule | Original location |
|---|---|---|
| Horizontal acceleration | 44/256 pixels/tick²; slippery tile 11/256 | `$cc8d..$cce9` |
| Horizontal speed clamp | ±336/256 before friction | `$cca5..$cce9` |
| Friction | approach zero by 31/256; slippery tile 3/256 | `$89ea..$89f3`, `$82cf` |
| Gravity | add 37/256 per tick | `$a416` |
| Initial jump velocity | −364/256 | `$cdb0` |
| Extended jump | while jump held and initial ten-tick timer nonzero subtract 44/256 | `$cbdc..$cc08`, `$cdbb` |
| Wall jump | horizontal velocity ±2; six-tick horizontal state timer | `$cdd8..$ce1f` |
| Wall slide | downward velocity clamps to 32/256 | `$cd82..$cd93` |
| Charged downshot recoil | vertical velocity −544/256 | `$cfb3..$cfba` |
| Ghost directional acceleration | 38/256, speed limits ±1.25 | `$8a9a..$8af0` |

Acceleration, clamp, then friction is the original ordering, giving a steady
horizontal displacement of 305/256 pixels per tick on ordinary ground.
The original preserves jump extension only during the initial timer or a
separate feather/float state; both are represented by named timers in the
replacement player update.

## Collision geometry

`$9b77` probes a moving player's leading horizontal edge at X ±4, Y −2/+3.
`$9bd0` probes feet at X ±3, Y +4 and head at X ±3, Y −3. Horizontal movement
is resolved before vertical movement. Contact flags `$0380` contain left/right
`0x04/0x08` and ground `0x10`. One-way platforms collide only on descent, with
an additional sub-tile Y threshold (`$9b5f`). Upward collision has unusual
negative velocity reassignment at `$9c19`; do not claim a conventional zero
velocity ceiling response exactly matches it.

`$9b42` maps tile graphics IDs 24..85 to solid, IDs >=86 to empty, and IDs0..23
through `$a93f`:

```
0,64,64,4,6,2,2,2,2,17,17,17,17,17,17,17,129,129,129,129,129,129,129,129
```

Bit1 means solid; bit4 one-way. Bit2 is associated with climbing and bit64
with damage logic. The latter meanings are inferred from consumers, rather
than source identifiers. Other flag semantics require more tracing.

## Shooting and animation

`$cf56` selects ordinary projectile type `$3a`; releasing a charged shot selects
`$35`. Both use speed3 along facing, up or down; horizontal speed4 is selected
under special `$d1` mode. Projectile owner lives at `$0300`. `$cf58` allows at
most two active shots per player (`$0552`), incremented on successful spawn and
decremented at `$c79d` on destruction. `$c715` loads lifetime37 ticks for normal
and41 for charged projectiles. Damage values table `$c709` is `[2,6]` indexed by
normal/charged status. Destruction can interact with destructible world tiles;
projectile collisions must not be reduced to actor-only hit tests.

`$cc58` fires once on B press, starts charge counter at `$49`, and signals full
charge at `$80`: 55 increments. Release while charged spawns the charged shot.
The original counter loops from overflow to `$92`; continued holding remains
charged. The native replacement uses a saturating counter.

Named metasprite frames recovered from animation entries:

| Animation | Frame IDs | Original sequence |
|---|---|---|
| Idle | 1 | `$97a7` |
| Running | 2,3,4,5 | `$9786` |
| Jumping | 6 | `$9797` |
| Falling | 7 | `$97ac` |
| Horizontal shot | 16 | `$9744` |
| Upward shot | 17 | `$97b1` |
| Downward shot | 18 | `$971c` |

These are metasprite IDs, not individual 8×8 tile indices. Animation decoder
`$f495..$f52c` stores the frame at `$0320`; descriptors also supply sprite offsets,
secondary effects and duration. Running sequence encoded duration is18, then
shifted twice, yielding four ticks per frame.

## Coverage and validation boundary

The player module implements movement, ladder attachment/climbing/detachment,
variable jump, wall jump/slide, charged directional spells, feather floating and
shielding, damage/death/ghost/revival, cooperative bumps, portal delay, swapping
and seeking. Player collision uses the source's inset sample points and preserves
subpixel position across contacts. Generic enemy collision remains a separate
rectangle solver. There is no arbitrary four-pixel fall-speed cap.

Recorded reference fixtures below establish the covered numerical outcomes.
They do not establish every possible combination of world objects, pickups,
input timing, mode flags or multiplayer contacts. Campaign automation is a
separate integration concern; isolated subroutine tests are not full playthroughs.

## Recorded regression fixtures

`tests/fixtures/player-run.csv` and `player-jump.csv` were captured from the
previous working static recompilation, in an isolated `/tmp/micro-mages-reference`
copy. They are observations of that reference, not measurements from the original
Windows executable. Both runs start normal mode, stage0, one player. A is pulsed
for six ticks every60 ticks before tick1500 to pass startup/title screens. Right
is held on ticks1650..1689. Jump is pressed on tick1720 and either released on
1721 or held through1754. No feather is present. Fields preserve original
integer/fraction components, signed velocities scaled by256, state/contact bits,
and jump/float timers to make discrepancies inspectable.

Tests compare all51 recorded horizontal velocities and positions, plus vertical
velocities for tap/hold jumps (stopping at ground collision). They caught and
corrected a one-tick jump-extension error: the original timer decrements before
processing, so the ten-tick launch timer gives nine subsequent boosts. Held jump
velocities begin `[-327,-334,-341,-348,-355,-362,-369,-376,-383,-390,-353] /256`.
Tap jump begins `[-327,-290,-253,-216,-179] /256`. With no feather, there is no
additional upward acceleration after that initial window. These limited fixtures
are evidence for ordinary player motion only, not parity of the whole rewrite.

Feather and ghost follow-up: `player-feather-ghost.csv` records two additional
controlled reference scenarios. At tick1650 the reference was given a feather
(`$8c=1`) or placed in ghost state (`$88=9`). The feather run jumps on1650,
releases on1664, presses again on1665 and then holds. The ghost run holds right
from1650. Exact velocity comparisons cover50 ticks of each scenario. Feather
activation cancels existing vertical velocity; the next velocities are
`[37,36,35,34,33,...]/256`. Ghost acceleration after19/256 drag is
`[38,57,76,95,...]/256`, capped at336/256 horizontally and320/256 vertically.

Feather pickup type `$20` sets `$8c=1` at `$9386`. An airborne jump press starts
float timer48, zeroes vertical velocity, then gravity applies. Subsequent held
jump subtracts38/256 until the timer reaches128. Ground contact resets float.
Damage at `$84ef` consumes a feather before health; `$8529` grants110 ticks of
invulnerability. These behaviors now have named fields and tests.

Normal-mode ghosts fire an uncharged freezing projectile on each fire press
(`$8a88..$8a97`, type `$39`), rather than a damaging ordinary spell. The player
emits `GhostShot` and the world owns its target interaction and one-shot limit.
`$be35..$be6a` revives a ghost collecting a fairy or feather when its head space
is clear: health1, vertical velocity−2, normal player state,110 invulnerability
ticks. The feather is equipped before revival. Reviving every player when one
collects a fairy is not original behavior. These observations come from static
code; the runtime fixtures do not exercise death/revival pickups.

Slippery ground is now selected through `Player::slippery` by the world from
material bit128. It uses acceleration11/256 and friction3/256 with the same
336/256 pre-friction speed clamp; dedicated tests verify launch, terminal
horizontal speed and braking. Pickup/revival tests additionally verify that
a collected feather remains equipped after ghost revival with health1.

## Collision, transitions and cooperative follow-up

`player-collision.csv` captures40 successive calls to source `$a42f` in each of
seven controlled terrain arrangements: floor, ordinary ceiling, floating ceiling,
left/right walls and upward/downward one-way-platform crossings. Tests compare
all280 positions and velocities exactly. Ceiling response preserves the fractional
coordinate and sets vertical velocity−128/256 (floating−18/256). Horizontal snaps
also retain fractions. One-way collision samples the original local Y threshold.
Player downward input is ladder attachment/movement, not a general platform-drop
command; generic physics retains its separate platform-drop option.

`player-transitions.csv` captures original ladder callback `$8a26` for41 ticks,
portal `$8af3` for16, swap-delay `$8bd9` for36 and death `$8b2c` for128.
The native tests compare positions and velocities on every tick and the final
death-to-ghost state. Source byte-coordinate wrapping is normalized only in the
isolated death fixture; the native world uses full-height coordinates.
Death launches at−2, applies ordinary gravity and remains uncontrollable128 ticks.
Ladder movement is±0.75 vertically, horizontal alignment nudges±80/256; jumping
from a ladder omits gravity on that initial frame. Portal falling velocity is0.5
for16 ticks before relocation and−3 launch. Swapping integrates before applying
8/256 drag during its36-tick delay.

`player-coop.csv` captures five source `$88a9` outcomes: ordinary/held/feather
head bounce, meeting an already-ascending player, and horizontal body bump.
The native helper matches the resulting velocities and control lock. `Player`
exposes `lock_controls` for the same source input suppression used by bosses.

`player-seeking.csv` captures all23 ticks of `$8b9a` through arrival. It caught
an otherwise easy-to-miss detail: source easing quantizes integer distance and
carries a fractional seed between axes. The native named seeking state retains
that arithmetic without byte-addressed state or instruction dispatch, including
preserved subpixel coordinates when arrival snaps to the destination.

These probes execute isolated callbacks in the saved reference recompilation,
with explicit initial state and synthetic terrain, rather than the original
Windows executable. They are source-behavior regression fixtures; no claim of
independent original-binary playthrough coverage is made from them.

Rendering follows the source pose streams with local pose timing. `$ce80` selects
idle/run/look-up/look-down, wall slide, feather and fast-fall poses; `$ca58` starts
the death stream. `$cb63` draws the attachment only with feather equipment and
uses palette 1, independent of player color. Its offsets are center X plus the
signed/mirrored attachment X minus four, and center Y plus attachment Y minus
five. `$cb96` renders the extra-health fairy before hurt flicker, using the two
16-entry orbit tables at `$cbbc/$cbcc`. Hurt flicker tests bit 1 of the remaining
invulnerability counter. Charge colors follow `$cb47` and its wrapping counter.
The rendering test isolates original PNG feather/fairy tiles and checks their
pixel coordinates, visibility and palettes independently of the player sprite.

Viewport guards `$caf7/$cb1e` retain subpixels and clamp integer center X to
4..252, top center Y to camera+6, and bottom to camera+240. Ghost callback `$8a55`
sets Y velocity to raw `$fcfc` (-772/256) at screen Y >=232 before drag/control.
Free fall `$cd0f` clears fractional downward velocity when its integer part is
at least two; it does not clamp the integer part. Four consecutive qualifying
ticks select the fast-fall pose. The source fixture covers 60 falling ticks.

Terrain damage distinguishes the sampled graphic ID from its material flags.
`$cab1` samples the center at Y-2 into `$99`; `$89c9` assigns horizontal knockback
only when that graphic is 1. Graphic 2 shares hazard material bit 6 but retains
horizontal momentum. The world regression test checks both graphics and both
facing directions, with equal health loss and hurt protection.

## Extra final arena flight

The hard extra arena intentionally has no terrain collision. `$ed51` makes `$d1`
negative; initialization `$ca61` therefore assigns living player state 2 rather
than normal platforming state 0. Its callback `$8a4f` selects the floating pose
`$979c` and enters the inertial flight controls at `$8a68`: drag 19/256 followed
by directional acceleration 38/256, directional X limits ±336/256 and Y limits
±320/256. It integrates position directly, without gravity or terrain collision.
It skips the bottom rebound belonging to ghost state 9.

This is a living, damageable flight mode. Fire uses the ordinary charge mechanism;
`$cf96` forces every shot rightward at four pixels per update regardless of held
direction. Fatal damage still runs its 128-update death sequence, after which
`$8b39/$8b6d` removes the player instead of creating a controllable ghost. Final
boss initialization grants health two and removes feathers from all players.

`player-flight.csv` records 120 updates of callback `$8a4f` from center (48,188),
with four movement directions followed by coasting, an ordinary shot and a
charged release. The regression compares all source positions and velocities
and both firing events. As with the other probes, this is isolated reference
recompilation evidence, not a claim of a completed original-binary playthrough.
