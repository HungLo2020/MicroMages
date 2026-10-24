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

These are the defaults. Open **Settings → Controls** to choose a player and
keyboard or gamepad, then select an action and press its new key or button.
Release the confirmation button before entering a binding. Conflicting bindings
are rejected; each player's device bindings can be reset independently.
The left stick continues to provide directional movement.

| Action | Player 1 | Player 2 | Player 3 | Player 4 | Gamepad |
| --- | --- | --- | --- | --- | --- |
| Move / aim | WASD | Arrow keys | IJKL | Numpad 8/4/6/2 | D-pad / left stick |
| Jump / confirm | G | Z or Enter | N | Numpad 0 | South |
| Shoot / charge | H | X | M | Numpad decimal | West / East |
| Settings / pause | Escape or Y | P | O | Numpad Enter | Start |

Confirm skips the opening and advances through the title, player selection,
and password prompt. Use arrows to select one to four players; the Shoot action
(H by default for Player 1) toggles normal/hard mode. Confirm an empty password to start, or enter an original tower
password with up/down and left/right. Up to four gamepads are supported. Hold jump for a higher jump;
jump against a wall to kick away. Hold shoot and release for a charged spell;
up/down aims vertically. A feather enables floating and absorbs a hit. Ghosts
can fly and fire freezing shots, then collect a fairy or feather to revive.
The hard-mode final arena uses directional flight and fires spells rightward.

Settings provide resume, separate music/effects volume, remappable controls,
display settings, restart confirmation and quit. Settings consume their own input
and freeze simulation and audio. Their navigation always uses arrows and Enter/Z
(X/Backspace to go back), or the standard gamepad D-pad/left stick and face buttons.
Escape always opens settings or goes back, regardless of the gameplay bindings.

**Settings → Display** offers window-size presets, fullscreen, and Sharp or Smooth
scaling; select **Apply** to save a change. **Alt+Enter** toggles fullscreen directly.
Fullscreen uses the desktop resolution and remembers your windowed size. Dragging
the window edges also changes and saves its size.
The game keeps its original 256×240 (16:15) image and proportions and presents it
through Quasar's viewport at the largest size that fits the window or screen.
Bars appear only on the left and right, or only on the top and bottom, never on
all four sides, and the image is never cropped or stretched. Sharp uses
nearest-neighbor sampling; Smooth interpolates pixels. Any window at least
256×240 is accepted, including square and portrait shapes. Existing `integer`
configurations load as Sharp scaling.

Following [Basalt standard 1](https://github.com/HungLo2020/Basalt/blob/main/STANDARDS.md),
controls and audio preferences follow the player; display settings remain local:

- `data/config/shared/controls.json`: keyboard and gamepad bindings for all four players.
- `data/config/shared/preferences.json`: music/effects volume (existing version-1 files are retained).
- `data/config/display.json`: remembered window width/height, scaling mode and fullscreen state.

Missing files initialize automatically. Changes are written atomically. The game
honors `BASALT_SHARED_CONFIG_DIR` and `BASALT_CONFIG_DIR` overrides and writes no
configuration into `app/` or the user's home configuration directories.

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
migration tools in [`DevUtils/`](DevUtils/README.md) document how the original
assets were recovered and are not invoked by Cargo. Source asset provenance is
recorded in `assets/manifest.json`; its hashes describe
the extraction snapshot, rather than restricting later asset edits.

Releases use `app/bin/micromages` and `app/assets/`. Run the executable directly or
launch through Basalt; no arguments, Steam, network, or terminal interaction are
needed. During development the checkout's asset directory is used.
`MICRO_MAGES_ASSETS=/path/to/assets` explicitly selects another directory.
Linux runtime requirements: a **Vulkan 1.3** driver and loader (dynamic
rendering, synchronization2 and timeline semaphores; Mesa's lavapipe is enough),
X11 or Wayland with `libxkbcommon`, host ALSA (`libasound.so.2`), udev
(`libudev.so.1`), the GCC C++ runtime (`libstdc++.so.6`), GCC unwind support
(`libgcc_s.so.1`), and glibc (release
baseline **2.35 or newer**). Compatible newer versions are supported. Audio uses
the host ALSA configuration and plugins, including its PipeWire/PulseAudio
integration. No private platform libraries are shipped. For development, install
`pkg-config`, `libasound2-dev`, `libudev-dev`, `libvulkan-dev`,
`mesa-vulkan-drivers`, `libxkbcommon-dev`, `libwayland-dev`, `libx11-dev`,
`libxrandr-dev`, `libxi-dev`, `libxcursor-dev`, `cmake`, `ninja-build` and a
C++ compiler (Debian/Ubuntu names), plus a Rust toolchain (1.94 or newer). The
first build compiles the shader compiler from source.
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
| `controls.rs`, `display.rs`, `storage.rs` | Portable bindings, display scaling, and Basalt configuration |
| `menu.rs`, `window.rs` | Settings UI, and Quasar window/Vulkan framebuffer presentation |

There is no address-based dispatch or shared emulated memory. Numeric sprite
catalog IDs identify artwork; level entity names select behavior. The simulation
has no window or audio-device dependency and can run headlessly.

## Development and verification

Rust plus the Linux X11/Wayland, ALSA and udev development libraries are required.

```sh
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
python3 -m unittest discover -s DevUtils  # optional migration checks; requires Pillow
cargo run -- --headless --stage 0 --frames 3600 --demo --screenshot /tmp/game.png
```

Quasar crates are consumed as pinned Git dependencies (`tag = "v0.2.5"`), so the
first build needs network access to GitHub. `quasar-window` and
`quasar-render-2d` are optional features: `cargo build --no-default-features`
produces a headless binary with no windowing or Vulkan dependency.

Development utilities (asset migration, packaging, release publication and
verification) live in [`DevUtils/`](DevUtils/README.md). This project follows
[CODING-STANDARDS.md](https://github.com/HungLo2020/LinuxScripts/blob/master/CODING-STANDARDS.md);
agent-specific instructions are in [`## AGENTS`](#agents) below.

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

## AGENTS

This section is the single source of truth for coding agents. `AGENTS.md` is a
symbolic link to this file, so every instruction here also applies to tools that
read `AGENTS.md`. The project follows
[CODING-STANDARDS.md](https://github.com/HungLo2020/LinuxScripts/blob/master/CODING-STANDARDS.md)
and the [Basalt Game Standard](https://github.com/HungLo2020/Basalt/blob/main/STANDARDS.md);
see [docs/basalt.md](docs/basalt.md) for how it is applied here.

Project skills live in [`.agents/skills/`](.agents/skills/); `.claude/skills` is
a symbolic link to that directory, so add or edit skills only once, there.

### Non-negotiables

- **No original game data.** Never add an instruction translator, CPU state, an
  original executable, a program image, or an emulator. Gameplay is the Rust
  simulation in `src/`; assets are editable JSON and PNG in `assets/`.
- **Basalt contract.** The game never writes to `app/`; user data goes only to
  `data/save/`, `data/config/` and `data/cache/`, with portable preferences in
  `data/config/shared/`. Honor `BASALT_*` and `MICRO_MAGES_ASSETS` overrides and
  keep the executable at `app/bin/micromages`. See [docs/basalt.md](docs/basalt.md).
- **Deterministic simulation.** Gameplay logic stays free of window and audio
  dependencies, runs at a fixed 60 Hz, and must remain replayable headlessly
  (`--headless`, `--replay`).
- **Assets stay editable.** JSON and PNG edits are read at startup; do not add an
  asset compiler or a build step that requires Python. `assets/manifest.json`
  records provenance and does not lock later edits.
- **Fixtures are evidence.** Do not regenerate the reference CSVs, `*_reference.json`
  traces, or the replay fixtures in `tests/fixtures/` just to make a test pass.
  Change them only with an intentional behavior change, and document why.

### Conventions

- Rust for game code, tests and examples. Python (standard library first, Pillow
  only where artwork requires it) for tooling in `DevUtils/`. Keep shell scripts
  limited to command orchestration.
- Tooling belongs in [`DevUtils/`](DevUtils/README.md); add a row to its README
  when adding a utility. Documentation belongs in `docs/`, linked from this README.
- Update the module table above when a module's responsibility changes, and keep
  this AGENTS section accurate in the same change.

### Verify like CI

`.github/workflows/release.yml` is the authority. Locally, with
`pkg-config libasound2-dev libudev-dev libvulkan-dev mesa-vulkan-drivers
libxkbcommon-dev libwayland-dev libx11-dev libxrandr-dev libxi-dev libxcursor-dev
cmake ninja-build build-essential python3-pil xvfb xdotool openbox` installed
(Debian/Ubuntu names), run:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked --no-default-features --release  # headless-only build
python3 -m unittest discover -s DevUtils
xvfb-run -a cargo test --test presentation -- --ignored --test-threads=1
cargo build --locked --release
python3 DevUtils/package_linux.py --revision "$(git rev-parse HEAD)"
python3 DevUtils/verify_package.py dist/MicroMages-latest-linux-x64.zip
xvfb-run -a python3 DevUtils/verify_audio.py dist/MicroMages-latest-linux-x64.zip
xvfb-run -a python3 DevUtils/verify_desktop.py dist/MicroMages-latest-linux-x64.zip
```

A push to any branch runs the `validate` job; a push to `main` also runs
`publish`, which replaces the rolling `latest` GitHub release and its
`MicroMages-latest-linux-x64.zip` asset. Use the
[commit-and-verify-ci](.agents/skills/commit-and-verify-ci/SKILL.md) skill to
commit, push and confirm the whole workflow, including that release.
