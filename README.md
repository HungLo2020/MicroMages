# Micro Mages — native Rust reconstruction

```sh
cargo run
```

This opens the game in a native Linux window with keyboard/gamepad input and native
Ogg audio playback. Enter skips the intro and confirms menu choices. The repository's extracted
PC game logic is translated into Rust at **build time**, then compiled
by rustc. No NES emulator, CPU instruction interpreter, Wine, Proton, or original
executable is launched by this program.

## Controls

| Action | Player 1 | Player 2 | Player 3 | Gamepad |
| --- | --- | --- | --- | --- |
| Move / menu | Arrow keys | WASD | IJKL | D-pad / left stick |
| A action / confirm | Z or Enter | G | N | South button |
| B action | X | H | M | West / East button |
| Settings / pause | Escape or P | Y | O | Start |
| Select | Tab | T | U | Select |

Up to four gamepads are supported. Keyboard and gamepad input can be combined.
Player count is selected in the game's menu. Escape, P, or any gamepad's Start
opens the native settings menu and pauses gameplay and audio. The menu provides
Resume, separate music/effects volume, a Controls page, Restart (with confirmation),
and an explicit Quit Game option. Use up/down to select, left/right to adjust
volume, and Enter/Z or the gamepad's South button to confirm. Escape/Start or
X/the gamepad's B button goes back. Volume choices last for the current run and
survive a restart from this menu. Resize the window to change display size.

## What was reconstructed

The installed PC game consists of an embedded game program, a custom C++ execution
wrapper, a tile/sprite compositor, and external Ogg music/effects. Its embedded
program differs substantially from the separately distributed NES ROM.

`build.rs` runs `scripts/compile_game.py` against the verified data in `assets/`.
The compiler follows recovered control-flow tables and callbacks and emits about
9,857 original instructions as 3,952 Rust basic blocks. Rust/LLVM compiles these
blocks to host machine code. The game keeps the original memory layout and
register-shaped intermediate values, but **does not fetch or decode instructions
at runtime**. It is an ahead-of-time recompilation, not a hand-rewritten,
high-level gameplay engine.

The original frame-wait loops become native frame scheduling. The runtime calls
compiled game routines and composites complete frames; it does not simulate NES
CPU clocks, scanlines, or an audio chip. Original level, character, enemy, boss,
collision, and menu logic executes through the translated Rust blocks.

- `src/game.rs`: frame scheduler, game data/state and native service boundaries.
- `src/video.rs`: software tile/sprite compositor with the PC palette and scrolling.
- `src/audio.rs`: native playback of all 9 music tracks and 34 effect mappings.
- `src/input.rs`: native gamepad connections and four-player controls.
- `src/menu.rs`: native settings, pause, volume controls and restart/quit actions.
- `scripts/compile_game.py`: fixed-build static compiler, with original addresses
  retained in generated comments.
- `scripts/game_roots.json`: recovered callback and computed-transfer targets.

Generated Rust is inspectable at
`target/debug/build/micro-mages-re-*/out/native_game.rs`. Its `translation.json`
records the source digest and translation counts. Unknown control transfers fail
explicitly; there is no interpreter fallback.

## Assets and prerequisites

All assets for this PC-game reconstruction live in `assets/`:

- `game-data.bin`: original 32 KiB program and game data, including level and animation tables.
- `tiles.bin`: original 8 KiB graphics tile data used for sprites, backgrounds and text.
- `music/`: all 9 original Ogg music tracks.
- `sfx/`: all 37 original Ogg sound files, including files not currently referenced by playback mappings.
- `manifest.json`: SHA-256 checksums for every asset and the source executable digest.

The build verifies the asset checksums, translates the repository program data,
and embeds graphics and game data into the executable. Audio is read from this
checkout's `assets/` directory at runtime, independently of the working directory.
Steam and the original installation are no longer needed to build or run.
Moving the checkout requires rebuilding so the audio directory path is refreshed.
These are the original game assets, not newly authored replacements.

Build requirements: Rust, Python 3, and the Linux X11/Wayland, ALSA and udev
development libraries used by the native window/audio/input crates.

## Verification

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- --headless --frames 3600 --demo --screenshot /tmp/micro-mages.ppm
```

The automated suite covers rendering, four-player register behavior, controller
slots/disconnection, music looping/seeking, and 1,800 frames for each of 14 stage
selections in normal and hard modes with four-player scripted input. The stage
selection injection exists only in test builds and uses the recovered new-game
entry. Normal stage 13 exercises the ending; hard stage 13 exercises the final
boss selection. A separate 10,000-frame normal gameplay smoke run passed.

All 17 tests and strict Clippy checks pass, including decoding every bundled Ogg
file. A fresh package build and 3,600-frame gameplay run were traced: neither
accessed Steam or the original executable. A windowed run from outside the checkout
also verified local audio loading and native window initialization. These checks
establish runnable gameplay and broad callback coverage, not complete playthroughs,
physical testing of every controller, or frame-by-frame parity with the original.

A native settings overlay replaces the PC wrapper's settings dialogs. Configurable
bindings and persistent configuration have not been recreated. The separate Second Quest ROM is not this build's target.

## Reverse-engineering evidence

REA 6.7.0 and Ghidra 12.1.4 are installed and registered with Codex. The matching
skill is at `/home/matt/.agents/skills/reverse-engineer-anything`; Codex configuration
backups were retained. Native analysis can be run without restarting the client:

```sh
bash scripts/rea.sh doctor --provider ghidra --json
```

Official project: <https://github.com/morluto/rea>.
Ghidra lives under `tools/ghidra_12.1.4_PUBLIC`. Its official release ZIP digest was
verified as `ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db`.

Local, ignored analysis artifacts retain full evidence:

| Evidence | Finding |
| --- | --- |
| `analysis/rea-checksum.json` | REA decompilation of checksum function at `0x4014f0` |
| `analysis/rea-video-write.json` | REA decompilation of PC rendering-register writes |
| `analysis/audio-hooks-rea.json` | REA decompilation of native music/effect hooks |
| `analysis/game-flow.json` | Static game control-flow and callback discovery |
| `analysis/game-levels.json` | Stage selection, game modes and transition fields |
| `analysis/disassembly.txt` | GNU objdump disassembly of the PC wrapper |

REA could not identify the requested function in the raw 6502 import; that failed
attempt is recorded separately. The game-program control-flow recovery used static
disassembly, while REA/Ghidra established native-wrapper behavior. Decompiler output
is evidence about the shipped artifact, not its original source code.
