# Basalt packaging and releases

Micro Mages follows [Basalt standard 1](https://github.com/HungLo2020/Basalt/blob/main/STANDARDS.md).
The catalog definition is maintained in Basalt's `resources/games/micromages.json`,
using schema 2, standard 1, no custom layout and no maintenance scripts. It is not
bundled with the game. The executable path stays `app/bin/micromages`.

## Installed layout

```text
MicroMages/
  app/
    bin/micromages
    assets/                  PNG, JSON and Ogg game assets
    licenses/                third-party notices
    build-info.json          source commit and platform
    basalt/cover.png         official Steam cover, adapted to 900×1200
    basalt/icon.png          transparent 512×512 purple mage
    basalt/sources.json      artwork attribution and provenance
    README.md
  data/
    save/                    reserved for user progress/content
    config/
      display.json           window dimensions, scaling and fullscreen (local)
      shared/
        preferences.json     music and effects volumes
        controls.json        four-player keyboard/gamepad bindings
    cache/
```

Runtime never writes to `app/` or Basalt's own files. Directory locations honor
`BASALT_GAME_DIR`, `BASALT_APP_DIR`, `BASALT_SAVE_DIR`, `BASALT_CONFIG_DIR`,
`BASALT_SHARED_CONFIG_DIR`, and `BASALT_CACHE_DIR`. Without those variables, packaged
executables locate the install root from their own `app/bin/` path, independent
of the working directory. Development uses the checkout's `data/` (gitignored).

Music and effects volumes and keyboard/gamepad bindings are portable player
preferences in separate version-1 shared configuration files. Display dimensions
and scaling live in `data/config/display.json`, outside the synced shared set.
They honor `BASALT_CONFIG_DIR` and `BASALT_SHARED_CONFIG_DIR` independently.
Defaults are compiled into the executable in `app/` and
written only when the configuration is missing. Changes are written to a sibling
temporary file, flushed, and atomically renamed. Existing valid files are retained;
malformed or unsupported newer versions produce a clear error instead of being
overwritten. Existing version-1 audio preferences keep their format and contents;
the controls and display files are created only when missing. Display Apply
changes the window immediately; manual resizing is saved after a short debounce
or on normal exit. Sharp (nearest-neighbor) and Smooth (linear) scaling fill the
entire height while preserving the native 256×240 image and proportions. Only side
bars are added; narrow windows become shorter to fit the whole image. Existing
`integer` settings load as Sharp. Fullscreen uses the desktop resolution, retaining
the windowed size for restoration; Alt+Enter toggles it. Portrait displays too
narrow for full-height output use windowed mode. Binding capture blocks menu and
gameplay actions until completion or cancellation, and Escape remains reserved
for settings navigation.

Progress currently uses the original tower passwords; this release does not add
saved mid-level sessions. Runtime caches are not currently generated. Their
directory is created for the standard layout. Explicit developer
CLI exports such as `--screenshot PATH` write to the user-specified destination.
No automatic writes occur outside the install root or launcher-provided paths.

## Linux release

The supported artifact is `MicroMages-latest-linux-x64.zip`, containing only `app/`
at its root. No wrapper, user data, build tools or original executable is shipped.
ZIP Unix permissions retain executable bits. The binary includes compiled Rust
code, while all platform shared libraries come from the host: [SDL2 **2.0.12 or newer**](https://wiki.libsdl.org/SDL2/SDL_SetTextureScaleMode), ALSA's `libasound.so.2` ABI, udev's `libudev.so.1` ABI, GCC unwind support,
and glibc. These ABI names do not pin distribution package versions. Compatible
newer releases are supported. ALSA uses the host's matching configuration and
plugins, including its PipeWire/PulseAudio integration. No private ALSA, udev, GCC,
or SDL library is shipped, and the executable has no RPATH/RUNPATH override.
Packaging checks direct library dependencies and rejects a private search path;
ZIP verification rejects shared libraries. Host X11/Wayland and graphics drivers
are used through SDL2. CI builds on Ubuntu 22.04 for a glibc **2.35 minimum**; a
release built locally on a newer distribution may require a newer glibc.

The game has no updater; Basalt owns updates, backup and sync.

To build the same package locally (Linux x64 with `readelf`/`ldd` and native build
dependencies installed; Xvfb, xdotool and Openbox for the desktop check):

```sh
cargo build --locked --release
python3 DevUtils/package_linux.py --revision "$(git rev-parse HEAD)"
python3 DevUtils/verify_package.py dist/MicroMages-latest-linux-x64.zip
python3 DevUtils/verify_audio.py dist/MicroMages-latest-linux-x64.zip
xvfb-run -a python3 DevUtils/verify_desktop.py dist/MicroMages-latest-linux-x64.zip
```

## Continuous validation and rolling latest

`.github/workflows/release.yml` runs on every branch push, pull request and manual
workflow dispatch. It checks formatting, strict Clippy, all Rust tests, asset-tool
and release-promotion tests, builds the optimized executable and verifies the ZIP.
The package test runs with read-only application files, outside the checkout, with
and without Basalt variables. It checks preferences, update preservation and moving
the entire installation to a path containing spaces. The graphical Xvfb test starts
with no arguments, changes volume, rebinds Jump, applies a new window size and
scaling mode, checks fullscreen and restoration, corrects a portrait resize, and
verifies the settings survive a relaunch.
Both runs quit through the UI with status 0. A separate audio check runs the packaged
game through host ALSA with a file-backed PCM and requires non-silent output. It
uses SDL’s dummy video driver, so CI needs neither speakers nor a sound card.

Successful pushes to `main` (and manual runs on `main`) publish the single mutable
GitHub release named **Latest**, tagged `latest`. PRs and other branches validate
and produce CI artifacts but cannot replace the public release. The tag moves to
the tested source commit; the ZIP and SHA-256 asset names stay stable. Release notes
and `app/build-info.json` identify the exact commit.

Publication is serialized without cancelling a publisher midway. Before publishing,
the job checks that its commit is still the head of `main`, preventing an older,
slower build from replacing a newer one. Uploads use temporary asset names that
Basalt does not match; old assets remain available until uploads complete. Asset
names are then promoted, the tag/notes updated, and the previous assets deleted.
Promotion failures restore the previous names. GitHub has no atomic multi-asset
swap, so a short rename window remains. A failed build/test never touches Latest.

GitHub release immutability must remain disabled for this intentional rolling-release
model. The publisher refuses to modify an immutable release. See GitHub's
[release upload API usage](https://cli.github.com/manual/gh_release_upload) and
[concurrency documentation](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency).

## Basalt integration

The matching standard-1 entry replaces the legacy schema-1 MicroMages entry in
Basalt's `resources/games/micromages.json`. Existing legacy installations should be
reinstalled once using the new catalog to adopt standard update/backup/sync actions;
older builds did not persist game settings or save files. No layout overrides are
needed. The catalog may also be tested as a local override under
`~/.config/basalt/games/` before distributing an updated Basalt build.

Compliance: release contains only app; app is read-only at runtime; all automatic
user files stay in data; preferences are portable and atomic; empty data initializes;
paths are relocatable; launch is offline and argument-free; normal quit returns 0;
controller support is retained; no custom updater; catalog uses schema 2/standard 1.

## Launcher artwork

The checked-in [artwork](../packaging/artwork/README.md) comes from the official
Steam release. Packaging copies it to `app/basalt/`, where Basalt discovers it
automatically after installation. The catalog has no `artwork.cover_url`: artwork
ships inside the game release and requires no separate download. Source URLs in
the bundled provenance file are attribution only. No changes to Basalt are needed.

The existing workflow now validates exact 3:4 cover geometry, minimum resolution,
opacity and the 2 MB limit, plus square transparent icons. Tests verify source
hashes and reproducible resizing; the ZIP check verifies that shipped PNGs match
the repository images byte for byte. No game runtime or release naming changes
are required. Artwork stays under `app/`, outside saves, backups and sync.
