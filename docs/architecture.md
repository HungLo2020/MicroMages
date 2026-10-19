# Native runtime architecture

The previous implementation mechanically translated instructions and retained a
CPU-shaped memory/register model. This implementation replaces that model with
named Rust data and functions. The previous version remains in Git history; its
execution was used to record reference traces and presentation observations.
It does not drive the current game.

## Frame flow

1. The host polls keyboard/gamepads and routes settings input.
2. `FrameClock` schedules 60 simulation updates per second. Pause discards elapsed
   time; long stalls have bounded catch-up. Headless runs use explicit ticks.
3. `Game` computes per-player input edges and advances its explicit Intro, Title,
   PlayerSelect, Password, Loading, Playing, StageClear, GameOver or Ending screen.
4. `World` updates players, resolves terrain contacts, updates active enemies,
   resolves projectile interactions, and handles props/checkpoints/rewards.
   Ordinary loading holds simulation still; the source stage-clear countdown
   keeps the remaining world active while departing players finish their exit.
5. Typed `GameEvent` values request sounds or music. The simulation never opens
   an audio device or files.
6. `Video` reads the resulting world and PNG/JSON definitions into a framebuffer.
   Rendering never advances gameplay. Settings draw over the frozen frame.

Positions are world pixels, positive Y downward; velocity is pixels per tick.
Bodies use top-left positions and explicit dimensions. Extracted spawn positions
are centers, converted at the asset/world boundary. Original movement constants
are exact binary fractions expressed as named Rust constants. Terrain uses 8px
collision cells grouped into 16px editable definitions for block destruction.

## Ownership

`Game` owns campaign state and an optional `World`; opening/menu screens do not
require a gameplay world. `World` owns its mutable copy of the
level, players, enemies, props, projectiles, camera, reward counter and deterministic
random state. It cannot mutate the shared source assets. A restart constructs a
fresh game. Ordinary stage changes carry shared and individual scores, lives, reward counter,
health and equipment forward; scripted arena initialization can alter player state. Controllers,
window, audio and settings are host-owned and survive ordinary level transitions.

`Player` and `Enemy` choose velocity and emit semantic actions; `World` owns
movement and interaction order. Projectile owners prevent friendly fire. All
collisions are resolved before the render pass. PNG transparency and palette
selection are rendering concerns, not physics concerns.

## Evidence and test scope

| Area | Verification |
| --- | --- |
| Graphics | All 512 tiles reproduce the original graphics-region digest |
| Stage assets | All expanded layers reconstruct from terrain definitions; 27 stage starts compare to original execution |
| Player physics | Recorded horizontal motion, tap/held jump, feather float and ghost movement traces |
| Bosses | Recorded attack schedules, plus consecutive movement traces across difficulties and player counts, documented in the enemy ledger |
| Collectibles | Motion, lifetime, visibility and animation traces for six kinds in four cadence phases |
| Presentation | Independent intro/title/loading and outcome image comparisons; native screen-flow tests |
| Native systems | Collision, one-way platforms, projectiles, reward rules, checkpoints, portals, campaign, pause, input and timing tests |
| Integration | 108 stage/mode/player-count stress scenarios; legal-input completion recordings; executable with copied assets outside the checkout |

The source-comparison fixtures are observations of the previous native
recompilation. They are intentionally separate from invariants describing this
implementation. Neither a passing scenario nor a bounded trace proves a complete
original-game playthrough. Whole-game equivalence is not inferred from subsystem traces. See the subsystem
ledgers for the exact captured conditions, interaction coverage and presentation
boundaries. Saved completion inputs are replayed from a fresh game; stale
recordings are not accepted as evidence for a changed build.

## Asset authoring

Gameplay reads standard PNG, JSON and Ogg files. Unknown named entities, invalid
layer sizes, invalid palette/tile references, missing campaign stages, and missing
music files produce startup errors. Terrain definitions include every reachable
break-state, so a shot can update graphics and collision together without an
address lookup or decompressor. The manifest is provenance rather than an edit
lock: ordinary edits do not require regenerating checksums to run the game.

The optional extraction scripts take archived original regions as explicit input.
No archived executable, original program image, or generated instruction
translation is needed by Cargo. Cinematics use PNG artwork with editable JSON
timelines; playback draws images and never executes archived instructions.
The source ledgers document remaining differences, including loading fades,
shared actor-slot allocation and the unimplemented secret challenge mode.
