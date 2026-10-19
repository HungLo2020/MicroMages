# Micro Mages — native Rust

```sh
cargo run
```

A native Rust reconstruction with explicit players, enemies, projectiles, terrain,
and campaign state. Gameplay runs at a fixed 60 Hz; rendering, input devices and
audio are separate. Cargo builds ordinary Rust source. There is no instruction
translator, CPU state, original executable, emulator, or program-image asset in
the build or runtime.

## Controls

| Action | Player 1 | Player 2 | Player 3 | Player 4 | Gamepad |
| --- | --- | --- | --- | --- | --- |
| Move / aim | Arrow keys | WASD | IJKL | Numpad 8/4/6/2 | D-pad / left stick |
| Jump / confirm | Z or Enter | G | N | Numpad 0 | South |
| Shoot / charge | X | H | M | Numpad decimal | West / East |
| Settings / pause | Escape or P | Y | O | Numpad Enter | Start |

Confirm skips the opening and advances through the title, player selection,
and password prompt. Use arrows to select one to four players; the Shoot action
(X for Player 1) toggles normal/hard mode. Confirm an empty password to start, or enter an original tower
password with up/down and left/right. Up to four gamepads are supported. Hold jump for a higher jump;
jump against a wall to kick away. Hold shoot and release for a charged spell;
up/down aims vertically. A feather enables floating and absorbs a hit. Ghosts
can fly and fire freezing shots, then collect a fairy or feather to revive.
The hard-mode final arena uses directional flight and fires spells rightward.

Settings provide resume, separate music/effects volume, controls, restart
confirmation and quit. Settings consume their own input and freeze simulation
and audio. Music/effects volumes persist in `data/config/shared/preferences.json`.

## Editable assets

All runtime assets are in [`assets/`](assets/README.md):

- `graphics/tiles.png`: original 512-tile sprite/background atlas.
- `graphics/palette.json`: RGB colors.
- `graphics/sprites.json`: sprite parts, offsets, flips and palettes.
- `graphics/animations.json`: named animation frames and durations.
- `presentation/`: PNG artwork and JSON timelines for the original opening,
  title, tower scorecards, Game Over, and endings.
- `levels/levels.json`: 27 playable normal/hard stage definitions, named entity
  spawns, tile layers, collision materials, and destructible terrain definitions.
- `music/` and `sfx/`: all nine music tracks and 37 original Ogg sound files.

No `.bin` game assets remain. JSON and PNG edits are read on startup; no asset
compiler or Python installation is needed to build or run. The optional Python
migration tools document how the original assets were recovered and are not
invoked by Cargo. Source asset provenance is recorded in `assets/manifest.json`; its hashes describe
the extraction snapshot, rather than restricting later asset edits.

Releases use `app/bin/micromages` and `app/assets/`. Run the executable directly or
launch through Basalt; no arguments, Steam, network, or terminal interaction are
needed. During development the checkout's asset directory is used.
`MICRO_MAGES_ASSETS=/path/to/assets` explicitly selects another directory.
See [Basalt packaging and releases](docs/basalt.md) for paths, dependencies and CI.

## Code layout

| Module | Responsibility |
| --- | --- |
| `game.rs` | Intro, title, selection/password, campaign progression and transitions |
| `world.rs` | Entity interactions, checkpoints, lives, pickups and combat |
| `player.rs` | Movement, jump/float, wall jump, shooting, ghost and revival |
| `enemy.rs` | Enemy states, boss patterns, damage and freezing |
| `physics.rs` | Bodies, terrain materials and collision resolution |
| `projectile.rs`, `pickup.rs` | Spell lifetimes, reward rules and collectible motion/expiry |
| `camera.rs`, `video.rs` | Camera tracking and PNG tile/sprite drawing |
| `assets.rs` | JSON/PNG loading, validation and terrain edits |
| `scenes.rs`, `outcomes.rs` | Native intro/title/loading and outcome presentation assets |
| `events.rs`, `audio.rs` | Semantic game events and native Ogg playback |
| `input.rs`, `timing.rs` | Device inputs and fixed simulation timing |
| `menu.rs`, `window.rs` | Settings UI and native desktop window |

There is no address-based dispatch or shared emulated memory. Numeric sprite
catalog IDs identify artwork; level entity names select behavior. The simulation
has no window or audio-device dependency and can run headlessly.

## Development and verification

Rust plus the Linux X11/Wayland, ALSA and udev development libraries are required.

```sh
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
python3 -m unittest discover -s scripts  # optional migration checks; requires Pillow
cargo run -- --headless --stage 0 --frames 3600 --demo --screenshot /tmp/game.png
```

`--stage N`, `--hard` and `--players 1..4` select a scenario directly. `--mute`
disables audio, and `--frames N` bounds a run. PNG and legacy PPM screenshots are
supported. `--headless` disables devices and runs deterministic simulation ticks.
`--replay PATH.json` plays recorded controller inputs with native graphics and
sound; add `--headless` for deterministic offline playback. Replay metadata selects
the mode and player count; a supplied `stage` starts gameplay directly, while
omitting `stage` exercises the opening and menus. Playback stops after the
recorded inputs.

Tests cover gameplay interactions and every playable stage, plus reference-run
fixtures for player movement, enemy cadence, boss behavior and collectibles.
Startup and outcome image fixtures check presentation separately from gameplay.
Asset regressions verify every graphics pixel, the expanded terrain layers,
and all 27 stage-start palettes and tile maps. Rendering is tested independently
of simulation, and a packaged-runtime test uses a separate asset directory.

Validation distinguishes reference traces, interaction tests, stress scenarios,
and completed legal-input replays. The 108-combination stress matrix exercises
every stage, mode and player count; a bounded stress run is not a completion
claim. Check current recording coverage with
`cargo run --example replay_matrix -- --validate-all`.

This is a tested reconstruction, not a claim of complete original-game equivalence.
Known limits include intermediate loading fade colors, shared actor-slot allocation,
and the separate secret `7195` challenge mode, which is not implemented.

Evidence and the scope of each comparison are documented in
[`docs/validation.md`](docs/validation.md),
[`docs/architecture.md`](docs/architecture.md),
[`docs/world-behavior.md`](docs/world-behavior.md),
[`docs/player-behavior.md`](docs/player-behavior.md),
[`docs/enemy-behavior.md`](docs/enemy-behavior.md), and
[`docs/pickup-behavior.md`](docs/pickup-behavior.md), and
[`docs/startup-presentation.md`](docs/startup-presentation.md).
