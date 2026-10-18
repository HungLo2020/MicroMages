# Micro Mages — native Rust reconstruction

```sh
cargo run
```

This opens the game in a native Linux window with keyboard/gamepad input and native
Ogg audio playback. Enter skips the intro and confirms menu choices. The installed
PC game's embedded logic is translated into Rust at **build time**, then compiled
by rustc. No NES emulator, CPU instruction interpreter, Wine, Proton, or original
executable is launched by this program.

## Controls

| Action | Player 1 | Player 2 | Player 3 | Gamepad |
| --- | --- | --- | --- | --- |
| Move / menu | Arrow keys | WASD | IJKL | D-pad / left stick |
| A action | Z | G | N | South button |
| B action | X | H | M | West / East button |
| Start / confirm | Enter | Y | O | Start |
| Select | Tab | T | U | Select |

Up to four gamepads are supported. Keyboard and gamepad input can be combined.
Player count is selected in the game's menu. P pauses the native host, R restarts,
and Escape closes the window. Resize the window to change display size.

## What was reconstructed

The installed PC game consists of an embedded game program, a custom C++ execution
wrapper, a tile/sprite compositor, and external Ogg music/effects. Its embedded
program differs substantially from the separately distributed NES ROM.

`build.rs` runs `scripts/compile_game.py` against the verified local executable.
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
- `scripts/compile_game.py`: fixed-build static compiler, with original addresses
  retained in generated comments.
- `scripts/game_roots.json`: recovered callback and computed-transfer targets.

Generated Rust is inspectable at
`target/debug/build/micro-mages-re-*/out/native_game.rs`. Its `translation.json`
records the source digest and translation counts. Unknown control transfers fail
explicitly; there is no interpreter fallback.

## Installed game and prerequisites

On this computer the build automatically finds:

`/run/media/matt/10TB Internal HDD/SteamLibrary/steamapps/common/Micro Mages`

For another installation:

```sh
MICRO_MAGES_DIR="/path/to/Micro Mages" cargo run
```

Build requirements: Rust, Python 3, and the Linux X11/Wayland, ALSA and udev
development libraries used by the native window/audio/input crates. They are
available on this computer. The exact supported executable SHA-256 is:

`b7bf279c1b7ce6714aca5634719d300ea60d2a0b8e073918c17a3702ee0a6e9c`

A different build is rejected rather than translated using guessed offsets.
The original installation is read without modification. Graphics and immutable
game data are embedded during compilation; Ogg files are read from the installation
at runtime. The original executable and `.nes` files are not opened at runtime.
No commercial assets are checked into the source tree.

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

All 13 tests and strict Clippy checks pass. The native window and audio initialization
have also been exercised. A traced windowed run opened 35 Ogg assets and launched
only the Rust executable; it opened no `.exe` or `.nes` files at runtime (recorded
in `analysis/native-run.strace`). These checks
establish runnable gameplay and broad callback coverage, not complete playthroughs,
physical testing of every controller, or frame-by-frame parity with the original.

The original PC wrapper's settings dialogs, configurable bindings and persistent
configuration have not been recreated. Native controls and pause/restart replace
those host features. The separate Second Quest ROM is not this build's target.

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
