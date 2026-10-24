# Third-party notices

Micro Mages is distributed with the Rust code linked into `app/bin/micromages`.
Platform libraries (Vulkan, X11/Wayland, ALSA, udev, glibc) come from the host and
are not shipped. Full license texts for the licenses used by the bundled crates
are in this directory:

- `MIT.txt`
- `Apache-2.0.txt`
- `ISC.txt`
- `Zlib.txt`

## Direct dependencies

| Crate | License | Upstream |
| --- | --- | --- |
| `quasar-window`, `quasar-input`, `quasar-core`, `quasar-storage`, `quasar-audio`, `quasar-render-2d`, `quasar-frame`, `vulkanic-gal` | MIT | https://github.com/HungLo2020/Quasar (tag `v0.2.5`) |
| `winit` | Apache-2.0 OR MIT | https://github.com/rust-windowing/winit |
| `x11rb` | Apache-2.0 OR MIT | https://github.com/psychon/x11rb |
| `raw-window-handle` | MIT OR Apache-2.0 OR Zlib | https://github.com/rust-windowing/raw-window-handle |
| `ash` | Apache-2.0 OR MIT | https://github.com/ash-rs/ash |
| `shaderc`, `shaderc-sys` | Apache-2.0 | https://github.com/google/shaderc-rs |
| `libloading` | ISC | https://github.com/nagisa/rust_libloading |
| `lewton` | MIT OR Apache-2.0 | https://github.com/RustAudio/lewton |
| `cpal` | Apache-2.0 | https://github.com/RustAudio/cpal |
| `hound` | Apache-2.0 | https://github.com/ruuda/hound |
| `crossbeam-channel` | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| `tempfile` | MIT OR Apache-2.0 | https://github.com/Stebalien/tempfile |
| `gilrs` | MIT OR Apache-2.0 | https://gitlab.com/gilrs-project/gilrs |
| `serde`, `serde_json` | MIT OR Apache-2.0 | https://github.com/serde-rs |
| `png`, `fdeflate`, `miniz_oxide`, `flate2` | MIT OR Apache-2.0 (Zlib for `miniz_oxide`) | https://github.com/image-rs |

## Transitive dependencies

Additional crates are pulled in through those dependencies (for example
`libc`, `hashbrown`, `zerocopy`, `once_cell`, `log`, `cfg-if`, `bitflags`,
`simd-adler32`, `bytemuck`, `libudev-sys`, `symphonia`-free Ogg decoding, and
`x11-dl`/`xkbcommon-dl`). The exact set and versions are recorded in
`Cargo.lock` at the corresponding source commit, and each crate carries its own
SPDX license expression. They are licensed under MIT, Apache-2.0, ISC, Zlib,
BSD-2-Clause, BSD-3-Clause or Unicode-3.0; the texts in this directory cover the
licenses that require the longest notices. No GPL or other copyleft dependency is
linked into the release binary.

## Assets

Artwork provenance is recorded in `app/basalt/sources.json`. Game graphics,
presentation and audio assets are original to the reconstruction and are covered
by the project's own licensing; see `README.md`.
