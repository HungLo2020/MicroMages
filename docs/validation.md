# Native validation

The tests run the Rust simulation with repository PNG, JSON and Ogg assets.
They do not load the original program, invoke an emulator, or require Steam.
Original-execution fixtures are numerical research evidence captured separately;
their provenance and scope are documented alongside each subsystem.

## Regression audit

Stage-completion recordings alone do not establish behavioral fidelity or test
startup. The suite therefore pairs legal controller recordings with original
execution traces and packaged boot tests. Corrected walker/bomb cadence, combat,
pickups and scene transitions have independent regression evidence; the controller
routes are then replayed under those corrected rules.

## Repeatable checks

```sh
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
python3 -m unittest discover -s DevUtils
cargo run --example replay_matrix -- --validate-all
xvfb-run -a cargo test --test presentation -- --ignored --test-threads=1
```

The presentation regressions check Quasar's viewport math for landscape, square and
portrait targets (16:15, bars on one axis only, never cropped), the Sharp/Smooth
filter mapping, a pixel-exact Vulkan framebuffer round trip, and a native window
presentation under Xvfb.

The original audit passed 164 Rust tests and nine Python asset-tool tests, plus
formatting, strict all-target Clippy and `git diff --check`.

The 2026-10-19 torch-contact follow-up adds two regressions and reruns all 166 Rust
tests and all 112 completion recordings. It verifies that every first-level wall
torch is harmless on contact, including both difficulties and one through four
players, while shots still extinguish flames. Original callback evidence and a
positive-control probe are recorded in `enemy-behavior.md`. The normal four-player
recording was replaced because the old route depended on the incorrect damage.
On Rust 1.99.1, strict Clippy reports six pre-existing `chunks_exact_to_as_chunks`
lints in image decoding/tests; all-target Clippy passes with only that new lint
allowed. Formatting and `git diff --check` pass.

All 108 individual stage/mode/player combinations have passed fresh verification,
including 513,820 controller-input frames and every subsequent stage/ending
transition. All four continuous campaigns below also reach Ending from a fresh
game using only controller inputs: 112 completion recordings in total.

## Original-behavior regression evidence

- The startup checks compare original intro/title frames and exercise normal
  boot through player selection, password entry and tower loading. Loading
  scorecards cover normal solo, normal four-player and hard four-player runs.
- Enemy cadence includes 9,216 original clock-gate outcomes, continuous walker
  and flyer traces, anchored shooters, 36 ordinary-enemy initializers, 704 hit
  callback combinations, Willowisp decay and freeze/hurt palette timing.
- Pickup fixtures compare 24 independent 701-tick original traces for motion,
  animation, expiration and blinking. Shared collection and ownership are also
  checked in the world integration tests.
- Doorway entry compares subpixel motion, animation, background priority and
  removal against two original traces. During the countdown the other actors
  remain live, and team wipes cannot restart the completed level.
- Boss completion accounts for the original half-rate counter on the callback
  frame. Regular bosses have 508/509 remaining updates, the final flying arena
  138/139, and ordinary exits 254/255. The segmented boss uses its own delay and
  lift-out script; its fixture compares 334 player updates and the transition.
- Original outcome artwork is checked pixel by pixel for Game Over, normal
  ending, the four-player hard cinematic and the animated final score screen.

## What is tested

- Original numerical traces cover player movement, collision, jumping, feather
  and ghost motion, cooperative bumps, seeking, freefall and final-arena flight;
  projectiles; turret summons; lightning; and boss state transitions/motion.
  Boss movement fixtures compare 73,600 consecutive frames across both modes
  and one through four players. These are isolated behavioral comparisons,
  not a claim of whole-game frame equivalence.
- Interaction tests cover shared collectibles, ghost revival/freezing, friendly
  fire, shared attempts, checkpoint restoration, reinforced terrain, portals,
  swaps, boss entrances, stage transitions, equipment and the final flight arena.
- The stress test exercises all 108 stage/mode/player-count combinations using
  distinct controller inputs, finite-state assertions and rendered frames.
  It does not require stage completion.
- Saved completion recordings cover all 108 combinations: 13 normal stages and
  14 hard stages, each with one through four players. Each recording contains
  only legal controller masks and is independently replayed from a fresh game.
  The tests require Stage Clear, then wait up to 512 idle ticks for the
  source-timed transition and verify the next stage or ending. Continuous
  campaign recordings must reach Ending while carrying state.
- The packaged-runtime test copies the executable and assets outside the
  checkout, runs hard mode with four players, reads its PNG output, exercises
  replay metadata and checks rejection of malformed assets.

`replay_matrix --validate-all` checks every stored recording. The default matrix
command requires all 108 stage/mode/player combinations; `--campaign` requires
eight continuous campaigns. Missing coverage makes those stronger commands fail.
Neither command implies whole-game frame equivalence.

Continuous campaigns start through `Game::start(0, players, hard)` and carry
score, equipment, health, reserve lives and random state across every stage.
Input files contain no position, health or completion overrides. Their separate
integration test requires complete normal/hard campaigns with one/four players.

## Continuous campaign recordings

| Mode | Players | Ticks | Result |
| --- | --- | --- | --- |
| Normal | 1 | 60,049 | Ending; one reserve remaining |
| Normal | 4 | 56,319 | Ending; two reserves remaining |
| Hard | 1 | 66,801 | Ending; three reserves remaining |
| Hard | 4 | 61,399 | Ending; three reserves remaining |

## Desktop checks

The restored intro/title ran in the actual desktop window with audio. Window
keyboard events traversed Intro, Title, four-player hard selection, password
confirmation and the original loading scorecard into gameplay. Captures verified
the loading art, Escape opening/closing settings, and Numpad 0 jumping only the
fourth mage. The packaged executable test independently compares original intro
and title PNGs and drives the complete startup flow outside the checkout.

The release executable also replayed all four continuous campaigns to Ending.
Normal endings and the hard-mode animated final score screens were captured and
visually checked.

Controller mapping, disconnect and reconnect behavior have automated tests;
four physical gamepads were not available for a hardware test.
