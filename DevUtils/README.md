# DevUtils

Development utilities: asset migration, packaging, release publication and
verification. Nothing here is invoked by `cargo build`, `cargo test` or the
running game; `cargo` builds and runs Micro Mages from `src/` and `assets/`
alone.

Run the unit tests for these tools with:

```sh
python3 -m unittest discover -s DevUtils
```

## Asset migration and artwork

| Utility | Purpose |
| --- | --- |
| `extract_assets.py` | One-time recovery of graphics and level JSON from archived original regions. |
| `extract_presentation.py` | Converts observed reference frames into `assets/presentation/` PNG artwork and JSON timelines. |
| `extract_outcomes.py` | Converts reference outcome frames into the Game Over and ending assets. |
| `prepare_artwork.py` | Resizes the checked-in official Steam cover and icon for Basalt (`packaging/artwork/`). |
| `verify_artwork.py` | Checks the Basalt artwork contract (sizes, transparency, provenance). |
| `test_artwork.py`, `test_extract_assets.py` | Regressions for the artwork contract and the migration output. |

## Packaging and releases

| Utility | Purpose |
| --- | --- |
| `package_linux.py` | Builds `dist/MicroMages-latest-linux-x64.zip` from an already-built release binary. |
| `verify_package.py` | Runs the shipped ZIP with read-only `app/` files and portable user data. |
| `verify_audio.py` | Verifies audio output through host ALSA with a file-backed PCM. |
| `verify_desktop.py` | Runs under Xvfb to exercise settings, remapping, resizing and persistence. |
| `publish_latest.py` | Replaces the mutable `latest` GitHub release after CI validation. |
| `test_publish_latest.py` | Unit tests for the release asset promotion/rollback logic. |

## Agent tooling

| Utility | Purpose |
| --- | --- |
| `rea.sh` | Launches `rea-agents` against a locally installed Ghidra (`tools/`, gitignored). |

Most utilities are Python, per the project
[CODING-STANDARDS.md](https://github.com/HungLo2020/LinuxScripts/blob/master/CODING-STANDARDS.md);
`rea.sh` is shell because it only orchestrates an external Node package.
