# Enemy and campaign evidence

This ledger separates static observations from native reconstruction decisions. The source is the original 32 KiB game program, SHA-256 `360cc9ac03244ba14a7452031fcd9da3a10aa69fed2c84e201c240d689eb4adb`, previously extracted from the Windows executable SHA-256 `b7bf279c1b7ce6714aca5634719d300ea60d2a0b8e073918c17a3702ee0a6e9c`. Addresses here are evidence coordinates, not runtime interfaces.

Retained local evidence: `analysis/game-disassembly.txt`, `analysis/game-levels.json`, `analysis/game-findings.md`, and `analysis/audio-tables.json`. The native wrapper audio identification is REA/Ghidra evidence `ev_0c8cc3df5457b9eae1fefc8a53b613c3e1f5675d74b366bdf98fcd70a7e5d9f6`, procedure `0x410a10`. These analysis artifacts are ignored local working evidence. The source gameplay disassembly was produced by static py65 decoding; no successful REA raw-program decompilation is claimed.

## Proven dispatch structure

At `$f791`, object type indexes split pointer tables `$b836`/`$b8da`. Descriptor offsets 0 and 2 hold update and initialization return addresses (big endian, add one). Offset 4 is a collision/hit callback for types that support one, not universally a destruction callback. Some effect descriptors are only four bytes long. The previous broad static root extractor incorrectly treats all as six bytes; it therefore includes data as candidate code. Never turn those candidate roots directly into gameplay identities.

At `$a2b1`, newly created gameplay objects initialize health from `$b7c1 + type`. Objects below slot20 have animation/hit fields; higher slots have a smaller state layout. `$a30a` allocates slots4..19; `$a314` allocates20..31. When exhausted, allocation can replace an object according to `$b894` priority. Players occupy0..3.

## Identified enemies and hazards

Names based on audio association are inferred identities with direct static call evidence. Generic labels describe observed behavior and are not claimed original names.

| Type | Identity | Init | Update | Evidence |
| --- | --- | --- | --- | --- |
| 01 | Inhaling boss | a571 | a581 | a69a plays inhale; a6da plays dash |
| 02 | Summoning boss | 9898 | 98ae | 98ce spawns type1b at player x clamped76..180, y112 |
| 03 | Flying boss | 9d85 | 9d9b | seven-state dispatch9da7; coroutine9ecc |
| 05 | Turret | f60c | f619 | shoots type10 at x±16 after timer; recovery16ticks |
| 06 | Final boss | f83a | f861 | five-state dispatchf870; coroutinef8ad |
| 07 | Goat | 86d3 | 86e4 | creates type3b, sound18 goat bubble |
| 09 | Willowisp | bc8d | bc93 | sound33 willowisp scream; chasing helperf384 |
| 0b | Troll | 9d2a | 9d33 | creates type38, sound19 troll spear |
| 0c | Snake | bcf6 | bcfc | proximity triggerbd60; sound31 snake |
| 0d,13 | Chaser | f343 | f369 | tracks selected player viaf384; offscreen escape |
| 0e | Walker | e520 | e52d | e571 moves1pixel per scheduled step (every4ticks normally) and reverses at wall/ledge |
| 14 | Hopper | e9a6 | e9c3 | e9b3 assigns signed8.8 vertical velocityfd80 = -2.5 |
| 1d | Spiderweb | f9b6 | f9b9 | clears player charge/state, launches vy=-4, sound1 |

Initial raw damage-unit health: type01=76,02=76,03=90,05=20,06=74,07=10,09=88,0b=12,0c=4,0d=2,0e=4,13=2,14=6. These raw values are not a claim about number of normal projectile hits: original projectiles have their own damage strengths.

Goat attack timing is parameterized from `$0568` (idle), `$0569` (attack cycle), `$056a` (burst repetitions); spawn occurs when attack timer reaches20. Troll delay comes from `$056d`, with spear creation synchronized to animation index5. Willowisp chase parameters come from `$0579..$057c`; health affects aggressive state. Snake launch waits for animation offset29, then uses `$056f` horizontal speed. Therefore substituting one universal cooldown cannot preserve all source behavior.

## Boss details

Common hit handling `$d411` rejects damage during startup, invulnerability, or protected attack state. Accepted hit sets20ticks of protection. `$d3cc` activation waits for a flag followed by47ticks. Boss death has additional presentation and campaign scripting.

Type01: inhale moves at approximately+1.5px/tick (a6a6) and pulls players below it upward; then dashes left at-5.5px/tick with vertical+1 (a6da). Its main coroutine changes attack speed with health and hard mode; the health thresholds are table-driven. It also alters terrain and adds entities in hard mode.

Type02: summons type1b at x clamped76..180, y112. Delay derives from `$0571`; minion behavior differs from generic projectile behavior.

Type03: chooses a player, creates three type29 objects at20tick intervals (`9efd..9f20`), then another pattern of eight at10tick intervals (`9f35..9f65`). Subsequent behavior has hard-mode-specific death/exit processing.

Type06: alternates target positions and summons. Its dash charge coroutine `$f927` waits40ticks, assigns both integer and fraction velocity bytesfa (approximately-5.023px/tick), dashes for50ticks, then waits60ticks. The native final-boss phase model uses this recovered signed8.8 velocity. The native navigation and attack transitions now pass consecutive original movement replays, including dash recovery.

## Campaign and menus

Stage order is explicit, not procedurally generated:

| Index | Presentation | Music |
| --- | --- | --- |
| 0,1 | Tower1 floors1,2 | spooky |
| 2 | Tower1 floor3 | evil |
| 3,4 | Tower2 floors1,2 | heroic |
| 5 | Tower2 floor3 | evil |
| 6,7 | Tower3 floors1,2 | mysterious |
| 8 | Tower3 floor3 | evil |
| 9,10 | Tower4 floors1,2 | dreamy |
| 11 | Tower4 floor3 | evil |
| 12 | Tower4 floor4 | march |
| 13 | normal ending / hard alternate boss | boss |

`$b317` initializes shared lives to3 and participation flags for the selected player count. `$b3dd` resets checkpoint, increments stage, and invokes stage presentation. Death/continuation handling decrements shared lives and reaches game over when exhausted. Mode byte `$6f` bit7 is hard, bit6 additional mode. Stage13 in normal mode routes to ending through tower index4; hard mode presents tower4/floor4 and loads the alternate boss.

The original pause uses input edge bit16 (`b36d`), pauses music, and changes main state. Native settings replace this with explicit host UI navigation and do not require preserving the original pause-state dispatch.

## Native implementation status and tests

`src/enemy.rs` provides named entities, health, movement intent, attack records, and explicit boss phases. Its unit tests verify deterministic native contracts: damage protection and single death, aimed spear cooldown, grounded hop, and charge/dash/recovery timing. Those tests do **not** establish source-game behavioral equivalence.

Validation limits: not every world interaction and every ordinary-enemy animation has an original execution fixture. Every entity in the extracted level catalog now maps to a named gameplay class or prop; there is no runtime unknown-enemy fallback. Projectile lifecycle, spawned hazards, pull forces, and terrain integration belong to the world, not this module.

## Additional spawn classification

The extracted levels contain all of the following; props must not be routed to hostile stationary enemies.

- 04 checkpoint: bc6d saves checkpoint position and plays sound21.
- 1c level exit: c228 switches player state, plays exit sounds, and starts128tick transition.
- 2e/30/33 paired portals: e28f searches another matching type; e2d0 relocates the player.
- 28 flame: c527 extinguishes on hit (sound13); player contact is harmless.
- 19 wall spike: c317 removes it when its supporting solid tile disappears.
- 1a horizontal wind: d264 pushes players1pixel/tick in a73pixel horizontal,19pixel vertical neighborhood.
- 1f wind current: e5b5 emits particles and applies directional movement.
- 1b lightning: c412 cycles warning,16tick active,68tick rest; boss version chains additional segments.
- 08 sorcerer boss: c550, health110, phase thresholds65 and41, summons via turret24 and hazard1e.
- 2f segmented arena controller: b5ca, thirty one-hit segments in a6×5grid; native implementation now uses independent segment collision and health tracking.
- 31 boss entrance trigger: 994e sets game state4, which lifts players and closes the arena before activating the boss.
- 21 player start.

### Wall-torch contact regression (2026-10-19)

The native prop interaction incorrectly called `player.damage()` near a flame.
The source type 40 (`0x28`) descriptor at `$c505` instead selects update `$d2f5`,
initialization `$c50b` and projectile-hit callback `$c527`. Update `$d2f5` calls
animation/render `$a3b9` and screen-bounds handling `$823e`. It skips the contact
damage call to `$857b` at the adjacent entry point `$d2f2`.

An isolated execution probe used the archived program from commit `224eb87`
(SHA-256 `360cc9ac03244ba14a7452031fcd9da3a10aa69fed2c84e201c240d689eb4adb`).
Player slot 0 and initialized flame slot 4 were placed at center (128,100), with
player health 2 and no invulnerability. Calling the actual flame update for 120
ticks left health 2, invulnerability 0, player state 0 and flame type 40. Calling
the adjacent contact-damaging entry point with the same setup instead produced
health 1 and invulnerability 110, providing a positive control. This probe checks
the callback in isolation, not a complete original-game playthrough.

Native regression tests overlap every actual first-level torch with one through
four players in both difficulties, at one and two health. Separate complete-world
updates retain the original level terrain and verify harmless contact, then check
that ordinary and charged shots still extinguish each torch exactly once. The
contact regression failed with health 0 before the fix and passes afterward.
- 0f/11/15/17/20 share pickup callback9325;11 swaps player positions,20 grants fairy/resurrection, others award score via b87c.

Source18 is a separate Leaper and24 a non-damaging SummoningPlatform; alias13(Chaser) preserves its exact descriptor in `Enemy::source_kind` when loaded by the world. Their canonical kind identifier alone is insufficient for rendering the original variant.

Difficulty parameter table recovered from d33b via d37a (normal/hard): goat idle130/45, burst interval99/79, burst count2/3; troll delay128/96; hopper rest25/0; chaser target speed88/116 fractional units; summoner delay180/115; diagonal flyer speed64/96 fractional units; willowisp calm speed30/52 and enraged100/124 fractional units. Fractional units are1/256pixel per tick. The native module now uses these recovered values where the corresponding behavior exists, and tests goat shot ticks209/308(normal) and104/183/262(hard).

## Original execution fixtures

`tests/fixtures/segmented_boss_reference.json` records the prior compiled gameplay routine execution, not expected values generated by the new implementation. The temporary reference checkout was initialized through original routine b5ca, active flag set, then b5e9 called once per tick with no players/projectiles. Eleven samples through tick552 record formation origin, wait counter, and movement step. The new module compares all four fields. Independent segments now move at the recovered rate, descend at steps15/31, speed up when fewer than21/15/6 remain, and disappear below the original boundary. Random segment firing now consumes the shared source-compatible paired-byte LFSR.

`tests/fixtures/sorcerer_boss_reference.json` likewise records original execution through c550/c59b, difficulty initialized through d37a, turret initialized through e50f. Spawned entities were removed after recording to keep available slots constant. It covers1000ticks and18 emitted objects: four type22 at ticks251,261,271,281; ten type30 at491..635 with16tick intervals; four more type22 at851..881. The native test compares the complete emitted timeline and type sequence. This verifies attack scheduling, not collision or RNG equivalence. Fixtures were captured with audio disabled and no game window.

Sorcerer hit phases now use the observed65/41raw-health thresholds: normal mode ends below41; hard mode enters a protected transformation then chases with the source speed92/256 and summons minions. Transformation motion and timing now match original execution, including the pixel displacement before chase; palette/effect presentation is handled separately by rendering.

Freeze is represented as an explicit duration with preserved velocity. Bosses reject it. The original ghost attack distinguishes normal190tick and special105tick hold values (f6e7; special types09/0a), with part of the interval slowing updates rather than fully pausing. The native freeze now follows the full-pause/alternate-update split, with `is_stunned()` and `contact_enabled()` exposing the current tick to world collision. Its alternate-update parity uses the supplied global frame index.

## Coverage after distinct boss implementations

All six boss classes now have explicit behavior; they do not share a generic attack cycle. The summoning boss emits lightning at tick62 then every180normal/115hard ticks with targetx clamped76..180. The flying boss emits three aimed orbs at20tick intervals then eight descending diagonal orbs at10tick intervals, cycling490ticks. `summoning_boss_reference.json` and `flying_boss_reference.json` were captured through the original init/update routines with a stationary player and free spawn slots for1800ticks; tests compare every emitted object and tick.

The inhaling boss approaches its staging point, flashes, inhales across the arena at1.5px/tick, dashes back at(-5.5,+1), and patrols until raw-health phases54/36/18 trigger another attack. Its patrol speeds use the recovered difficulty values. `inhaling_boss_reference.json` captures100ticks of an already-established inhale phase from original routinea69a/a581; the regression compares exact positions after the initial setup tick. The later full lifecycle fixture covers approach, bobbing, all three damage-driven interphase transitions and hard-mode timing; tile removal is represented by explicit world actions.

The final boss navigates between center/lower/upper targets, emits aimed triples and radial volleys, and adds dashes after two cycles. Its radial volley uses the decoded seven signed8.8vectors rather than evenly spaced synthetic angles. `final_boss_dash_reference.json` captures90ticks through original charge/dash routinesf927/f861; native tests verify charge velocity216/256, initial dash-1286/256, and12/256per-tick deceleration. The fixture intentionally excludes the subsequent steering recovery. Full navigation and movement-dependent transitions now have the consecutive-frame validation described below; separate projectile fixtures cover emissions.

The native enemy module has no CPU registers, memory-address dispatch, program-byte decoding, or dependency on the original program at runtime. Original addresses only appear in explanatory provenance comments and this evidence ledger.

## Full movement replay validation

`boss_motion_lifecycles.json` contains 73,600 consecutive original-execution frames: Inhaling and Final bosses, normal and hard modes, each with one through four moving player targets, 1,600 ticks per case. Each row records tick, center x/y and velocity x/y as signed fixed-point units (1/256 pixel). The original routines were initialized at (128,72), with active boss flag set, zero random seed, and free spawned-object slots. Targets follow x=64+(tick/20+40*player)%128 and y=128+(tick/60+12*player)%64. Capture used the temporary reference checkout; the fixture is not generated by native code.

The regression integrates the native velocity each frame and compares every recorded position and velocity, including the final boss's two full attack patterns, dash, steering recovery, and inhale approach, dash, patrol bobbing and boundary reversal. All 32 cases pass without selecting passing subsequences. This establishes exact body movement under those conditions, not full campaign equivalence: zero RNG, unchanged boss health, and removal of spawned objects limit the claim. Attack damage gates, collisions with actual players, effects, and interactions with the shared world still require distinct evidence.

Source steering now uses component-wise signed fixed-point acceleration and seeded division rather than normalized vectors. Inhaling emits explicit world actions for player pull (vertical velocity -258/256), control lock, platform removal, and hard-mode minion spawning; cosmetic inhale particles are separate from damage. Final preserves vertical velocity during its horizontal dash charge and dash, then resumes navigation. Summoning chooses a player through the shared random generator for every lightning spawn and accounts for camera y. Transformed Sorcerer uses acceleration14/256, component limit92/256 and a two-minion cap (c6bf..c6f6).

Further fixtures cover all three damage-gated inhale repeats (health53/35/17 at ticks500/1500/2500, eight4000-tick cases) and hard Sorcerer transformation/chase (eight2000-tick cases, one through four moving players, both zero and0xbeef random seeds). All body positions and velocities match. Remaining evidence boundaries include whole-world interaction combinations and some ordinary-enemy animation states. The older bounded fixtures above remain useful scheduling checks but should not be described as whole-game validation. Native health and reward units preserve the source tables, including sprite variant-specific rewards.

`chasing_enemy_reference.json` records 3,466 consecutive original movement frames for Chaser and Minion in both difficulties, through target-lock expiry and escape. It verifies the four-frame steering cadence,102-update Minion/256-update Chaser target lock, uncapped outward escape acceleration, and Minion movement-before-steering order. `troll_animation_reference.json` records2,000 original frames; sprite and projectile count match every frame. Troll's128/96 timer is followed by16 animation ticks before shooting, yielding144/112 total intervals. The renderer receives named editable animation streams with local clocks, including original sprite offsets.

Bomb fuse remains60 ticks. Explosion allocates two type34 bouncing projectiles with(+0.5,+0.5) and(-0.5,+0.5) velocities; it is not a synthetic area attack. Projectile collision owns their four-contact lifetime. Rewards use source descriptor-specific score units.

## Final hazard and summon corrections

Turret type05 now has its own source lifecycle: arena-gated rise on alternate ticks, position-dependent initial delay, alternate-tick repeat countdown,16-tick recoil, and fixed-facing type10 summoned walkers. Type24 is a distinct SummoningPlatform that only renders and rejects damage. Type18 is a distinct Leaper with grounded animation windup and fixed horizontal air speed. Type14 Hopper responds to nearby shots or a ledge instead of jumping automatically toward a player. Its source shot-proximity and jump constants are recovered; its terrain interaction is covered by native behavior tests, not a complete source movement replay.

`turret_reference.json` compares every phase timer, position and emission across1,200 original frames. `turret_minion_reference.json`, exercised by physics tests, covers the summoned walker's initial fall, ground hop, two wall reversals and removal in both difficulties. Source update order requires the hop's horizontal motion to start on the following tick and wall reversal to affect the following tick's assigned velocity.

`lightning_reference.json` verifies all four difficulty/boss combinations, warning and active timers, all three boss segments and each tick's player hit mask. Active lightning tests a rotating8x8 collision probe at y offsets-12,-4,+12,+4 in countdown order; warning/rest never damage players. The boss chain has centers32pixels apart and disappears after its16th active contact tick. Repeated artificial player hurts in the first fixture capture spawned unprocessed fairies that exhausted the source entity pool; the corrected isolated capture removes those unrelated actors after each tick. No native behavior was adjusted to reproduce that fixture artifact.

Chaser/Minion fixtures also include assigned targets that do not exist: source enters outward escape on its next steering update instead of retargeting another player. This matters for the Sorcerer's fixed target order3,2,1,0 in games with fewer than four players.

### Summon caps and viewport cleanup

The source counts allocated actor types, not a visibility estimate: `a450` scans slots4..19. Sorcerer `c6e6` counts type16(hex) Minions and requires fewer than2; Inhaling `a620` counts type0d Chasers and permits1 in hard mode or3 in extra mode. These are distinct native context counters. Escaping actors continue counting until removed.

Minion `9449` integrates movement, then calls `d2f2 → d2f5 → 823e/8247`. The slot clears when integer center x is outside `[7,249)` or screen-relative y outside `[0,240)`. Chaser takes the same bounds path on its two-of-three rendering cadence. Native cleanup follows those bounds after movement, preserves actors on frozen/skipped updates, and keeps escaped summoned actors scheduled so freezing cannot strand them beyond the world's ordinary activation margin. Summons update before boss cap reads, matching their later source slots. Regression tests cover exact boundary inclusion, frozen offscreen actors thawing, resumed Sorcerer summoning, and the separate Inhaling cap.

## Ordinary-enemy cadence correction

The earlier validation missed patrol cadence: calling the one-pixel movement
helper once per display tick made Walkers four times and Bombs eight times too
fast. Source `e551 → 822c` with Y=1 selects `(slot+frame)&3`; `fa59 → 822c`
with Y=2 selects `&7`. Hard mode keeps those rates; the separate extra-mode
bit halves each interval. The native movement now preserves these gates.
The shared thaw dispatcher `a3ae` halves the local frame counter on alternate
updates; native per-enemy cadence now uses that same divided counter.

`ground_cadence_reference.json` records original helper execution for all three
mode bytes, eight slot phases, both species, and thawed/nonthawed updates.
These are scheduling observations, not whole-world recordings. Separate world
tests verify resulting displacement and reversal at platform edges. Patrols
use source center probes (forward4/down9) and do not fall under player gravity.

Hard Walkers previously omitted their attack pause. Original `e55c` counts35
patrol steps, then `e535` waits through the animation, emits a type34 bouncing
spark35ticks later and resumes patrol52ticks later. The native test compares
all800 frames of `walker_motion_reference.json`, including every position,
attack-state transition and emitted-shot count in normal and hard mode. This
capture used the original routine on a continuous solid floor with one player.

DiagonalFlyer previously moved immediately upward. Source `a4f5` checks nearest
player Manhattan distance<64 every four ticks, awakens for26normal/18hard ticks,
then assigns a downward diagonal velocity of64/256 or96/256 pixels per tick.
Integration starts the next tick; solid terrain reflects each component using
four-pixel center probes (`a50a`). `diagonal_flyer_reference.json` compares360
original execution frames covering activation, the complete wait and flight in
both difficulties. The fixture has empty terrain; reflection has native checks.

The cadence audit also inspected stationary Goat/Troll timers, Snake's continuous
fractional motion after its attack windup, Leaper's fixed160/256 horizontal air
speed, Hopper's one-of-three grounded steps, and four-tick Chaser/Minion/Willowisp
steering. Turret and summoned-walker timing retain their existing original
execution fixtures. This is a focused movement audit, not a claim that every
ordinary-enemy animation and world interaction has been reproduced exactly.

The world audit also removed artificial gravity from stationary Goat/Troll
actors. Their source update paths end in collision/render helpers without any
position integration; dropping their8pixel collision boxes onto terrain shifted
their anchors even when their direct AI tests passed. The world regression uses
`anchored_enemy_reference.json`:960 original updates, both families and both
modes, on empty terrain. Their centers remain exactly(128,72). Snake, Hopper,
Leaper and summoned walkers retain their respective source movement/terrain
paths. The source modulo3 clock is independent of the halved thaw-local frame
counter, so Hopper stepping and Chaser cleanup keep the global modulo3 phase.

### Complete ordinary initialization and damage audit

`ordinary_initialization_reference.json` captures the original allocator `a2b1`
through descriptor initialization for all18ordinary source kinds, in normal and
hard mode. This corrected Goat health10/11, Snake4/5 and Leaper4/5; Troll stays12
in both modes. SummoningPlatform stores33 raw health but rejects all hit callbacks.
Source hazards store0 damage health; the native model uses a positive active
lifetime sentinel and separately rejects damage/freezing for them. No universal
hard-mode health increase is inferred.

All21 entries of the normal/hard `d33b` parameter rows were checked against their
read sites: Goat delays/bursts; Bubble speed/rise; Troll delay; Hopper recovery;
Snake speed; Chaser speed; Summoning delay/turret period; summoned-walker speed;
Lightning warning; Sorcerer delay; Spark speed; DiagonalFlyer speed; Minion speed;
Willowisp calm/aggressive acceleration and speed. Existing normal/hard numeric
parameters match those rows. Extra mode uses a third row and separate feature
flags; it must not be substituted for the hard final flight stage.

Willowisp damage is not ordinary linear health loss. `bc93` decays damaged
health toward the next lower multiple16 on odd global ticks. Reaching0 by decay
does not remove the actor; a final successful hit still invokes destruction.
`willow_health_reference.json` captures768 original updates for undamaged,
partially depleted and terminal health states in both modes, including allocated
actor type. Native `is_alive()` preserves that distinction, with a world test
for the final projectile hit.

Goat/Troll keep their map-authored facing instead of auto-aiming at players.
`stationary_shot_reference.json` captures first-shot timing, position and signed
velocity in both modes and directions while the player stands behind the actor.
The native test includes projectile construction: Goat Bubble speed152/186
fractional units and Troll Spear velocity(±1,-3), with its4pixel muzzle offset.

### Projectile callbacks and hit feedback

`hit_response_reference.json` captures 704 original `f773` callback dispatches:
22 source kinds × 2 local states × 4 damage values × 4 boss guard contexts.
Each row records `[kind, state, damage, active, protected, invulnerability,
response, resulting_state, resulting_invulnerability]`. Boss health is 100;
ordinary health is 4. The callback is invoked with a projectile slot and target
slot, without running generic damage, so callback results remain distinct from
health subtraction. Original `c91f–c96c` interprets response 0 as damage, 1 as
absorb without damage feedback, and 2 as pass through. The native `HitResponse`
models all three, including charged shots: absorption removes the projectile
without downgrading it into an ordinary shot.

An inactive boss passes shots; a protected or recently hit active boss absorbs
them. Bomb `fa66` arms a 60-tick fuse on first lethal damage and absorbs that
shot. While armed it passes lethal shots but still accepts sublethal damage.
DiagonalFlyer `a4a6` wakes on nonzero damage, independently of player proximity.
Turret and ordinary walkers have no phase-specific hit callback protection.
Spike, Lightning, Spiderweb and SummoningPlatform use the pass-through callback.
Ordinary damage has a 10-tick palette effect, not invented hit immunity; only
boss guard `d411` starts a 20-tick damage lock. Separate integration tests check
shot lifetime, charged-shot state, unchanged health and absence of damage sound
for absorbed shots.

Willowisp callback `bce9` restarts the repeating `9659` hurt animation on each
nonzero hit. The native `willowisp_hurt` asset names that source stream, and its
local playback clock restarts independently of the actor's lifetime. Zero-damage
freeze callbacks retain the existing animation. A regression checks idle, first
hit, elapsed playback and a repeated hit.

The segmented controller uses its own projectile path (`b715–b731`): every
successful segment hit consumes the spell, including charged spells, awards one
score unit to that spell's owner, and emits destruction sound08. The native world
handles this before generic charged-spell piercing. A regression test verifies
one removed segment, score ownership and projectile removal together.
