# Basalt packaging and releases

Micro Mages follows [Basalt standard 1](https://github.com/HungLo2020/Basalt/blob/main/STANDARDS.md).
The catalog definition is [packaging/basalt.json](../packaging/basalt.json), using
schema 2, standard 1, no custom layout and no maintenance scripts. The executable
path stays `app/bin/micromages`, compatible with the previous catalog entry.

## Installed layout

```text
MicroMages/
  app/
    bin/micromages
    assets/                  PNG, JSON and Ogg game assets
    lib/                     ALSA, udev and GCC unwind libraries
    licenses/                bundled native-library notices
    build-info.json          source commit and platform
    basalt.json
    README.md
  data/
    save/                    reserved for user progress/content
    config/shared/preferences.json
    cache/
```

Runtime never writes to `app/` or Basalt's own files. Directory locations honor
`BASALT_GAME_DIR`, `BASALT_APP_DIR`, `BASALT_SAVE_DIR`, `BASALT_CONFIG_DIR`,
`BASALT_SHARED_CONFIG_DIR`, and `BASALT_CACHE_DIR`. Without those variables, packaged
executables locate the install root from their own `app/bin/` path, independent
of the working directory. Development uses the checkout's `data/` (gitignored).

Music and effects volumes are portable player preferences in the version-1
shared configuration. Defaults are compiled into the executable in `app/` and
written only when the configuration is missing. Changes are written to a sibling
temporary file, flushed, and atomically renamed. Existing valid files are retained;
malformed or unsupported newer versions produce a clear error instead of being
overwritten. There were no persisted settings in older releases to migrate.

Progress currently uses the original tower passwords; this release does not add
saved mid-level sessions. There are no machine-specific settings or runtime cache
files yet. Their directories are created for the standard layout. Explicit developer
CLI exports such as `--screenshot PATH` write to the user-specified destination.
No automatic writes occur outside the install root or launcher-provided paths.

## Linux release

The supported artifact is `MicroMages-latest-linux-x64.zip`, containing only `app/`
at its root. No wrapper, user data, build tools or original executable is shipped.
ZIP Unix permissions retain executable bits. The binary bundles its Rust runtime;
ALSA, udev and GCC unwind libraries are supplied under `app/lib` with an ELF
`$ORIGIN/../lib` search path and distribution license notices. Glibc and the desktop
X11/Wayland/display/audio drivers remain host facilities. CI builds on Ubuntu 22.04
for a glibc 2.35 baseline; newer compatible desktop Linux installations are supported.
The game has no updater; Basalt owns updates, backup and sync.

To build the same package locally (Linux x64 with `readelf`/`ldd` and native build
dependencies installed):

```sh
RUSTFLAGS='-C link-arg=-Wl,-rpath,$ORIGIN/../lib' cargo build --locked --release
python3 scripts/package_linux.py --revision "$(git rev-parse HEAD)"
python3 scripts/verify_package.py dist/MicroMages-latest-linux-x64.zip
xvfb-run -a python3 scripts/verify_desktop.py dist/MicroMages-latest-linux-x64.zip
```

## Continuous validation and rolling latest

`.github/workflows/release.yml` runs on every branch push, pull request and manual
workflow dispatch. It checks formatting, strict Clippy, all Rust tests, asset-tool
and release-promotion tests, builds the optimized executable and verifies the ZIP.
The package test runs with read-only application files, outside the checkout, with
and without Basalt variables. It checks preferences, update preservation and moving
the entire installation to a path containing spaces. The graphical Xvfb test starts
with no arguments, changes volume via the real settings UI and quits with status 0.

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
