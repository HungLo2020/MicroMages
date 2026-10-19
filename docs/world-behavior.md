# World interactions and campaign evidence

The native world uses ordinary Rust entities and editable terrain layers. Original
addresses below identify recovered evidence; they are not runtime dispatch keys.
The reference program SHA-256 is
`360cc9ac03244ba14a7452031fcd9da3a10aa69fed2c84e201c240d689eb4adb`.

## Multiplayer and attempts

- Stage initialization (`f2da`) places participating players in descending ID
  order, eight pixels apart. Player 0 is rightmost when several players join.
- Death starts a 128-tick dying state. Once nobody remains alive, the shared
  attempt countdown runs concurrently for 64 half-rate updates (`ca1d`, `b395`).
  Three reserve lives therefore permit four attempts. The world reloads terrain,
  enemies and pickups at the checkpoint; score and collected campaign bonuses
  remain. Checkpoints stay visible in their activated pose.
- Stage exits set a 128-unit half-rate transition (`c271`, `b3d5`), or 256 native
  simulation ticks. Health and feather equipment survive stage progression
  (`f2c4`, `ca61`). Cooperative bumps, ghost shots, collectible revival and body
  movement are separate rules; player spells do not damage teammates.
- Each collectible is consumed once, even if all four bodies overlap it in the
  same tick. Swap pickups act automatically and choose the highest living
  teammate, excluding the collector (`93a0`). They use the timed seeking motion
  recovered in the player module rather than immediately exchanging coordinates.

## Portals and boss entrances

Portals require grounded contact (`e28f`) and another visible portal on the same
channel. Their 16-tick movement state ends at the destination with upward
velocity (`e2d0`). The special Tower 2 arena portal can create a one-time bonus
collectible; `86ad` creates a collectible effect, not another enemy.

Entity 49 is a **boss entrance**, not an exit. Its trigger uses center distances
less than 16 pixels (`9cf6`, `9c4a`). The entrance sequence (`994e`, `b439`,
`b482`) gathers players, revives missing participants in descending order at
16-frame intervals, seeks toward the doorway, then moves the camera upward by
two pixels and players upward by one until the arena is reached. Collision is
bypassed during this scripted movement. Four doorway graphics open at eight-tick
intervals; the closing sequence replaces the two floor cells with terrain 19
(`b4d8`, `99b8`). Boss activation follows closing; applicable boss classes have a
further 47-tick startup (`d3cc`).

Tests exercise this sequence for both modes and all player counts. A separate
recorded-input entrance test reaches it from a fresh game without setting player
positions or health. Synthetic interaction tests are not presented as playthroughs.

## Combat and terrain

- Hostile bubbles, spears and orbs integrate freely across terrain. Bubbles curve
  upward with difficulty-dependent acceleration; spears have gravity. Bomb
  sparks have their own bouncing terrain response. Viewport bounds remove
  projectiles; a universal arbitrary hostile lifetime is not used.
- A charged spell spends up to six damage units. If it consumes fewer than six,
  it survives as an ordinary spell (`c954`). Protected enemy hits do not consume
  the shot. Enemy score values follow source type aliases instead of a universal
  award.
- Reinforced terrain 16 has six shared hit points across its four graphic tiles.
  Ordinary and freezing shots deal two, charged shots six. The original retains
  only eight partially damaged cells in a circular cache (`945c`); the native
  cache preserves that eviction behavior. Terrain edits update graphic,
  collision and palette layers together.
- Freezing shots cross ordinary walls and opened chests, but are consumed when
  damaging or destroying breakable terrain (`f6bc`, `c7ce`). They do not damage
  actors. Chest opening and block destruction produce rewards once.
- Boss forces, terrain changes, summons and harmless particles are explicit
  world actions. Flying boss bodies bypass generic ground collision.

## Hard-mode final arena

The hard-only final arena intentionally has no terrain collision. Source
`ed4d..ed51` enables its special mode, `ca61..ca69` selects living flight, and
`f83a..f852` restores two health points and removes feathers for every player.
Campaign entry and respawning both apply that setup. Flight uses directional
acceleration and damping without gravity; it does not turn living players into
ghosts. Arena shots always travel right at four pixels per tick (`cf72..cf9c`).
Tests hold every player count in the empty arena for 600 ticks, verify respawn
and campaign-entry equipment, and compare flight against original execution.

## Validation categories

`cargo test --all-targets` includes original-execution numeric fixtures, focused
interaction tests, deterministic stress scenarios, rendered frames, packaged
execution and saved legal-input recordings. `tests/scenarios.rs` covers all
108 stage/mode/player-count combinations with independent inputs; its bounded
runs are stability checks, not completion evidence. `tests/replay_validation.rs`
requires all 108 saved stage/mode/player-count completion recordings to reach
stage clear from `Game::start`, then verifies each next-stage or ending transition.

Use `cargo run --example replay_matrix -- --validate-all` to report current
recording coverage. Use `cargo run -- --replay PATH.json` to inspect a recording
with native graphics and sound, or add `--headless --screenshot /tmp/frame.png`
for deterministic offline rendering. No original executable or emulation runtime
is needed to run these tests or recordings.
