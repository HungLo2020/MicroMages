# Intro, title, and tower scorecard

These are native image and animation assets. No instructions, memory image,
register state, or runtime extraction is used by the game.

- `cinematic.png`: RGB artwork atlas, 64 columns of 8-pixel tiles. Repeated
  artwork is stored once across both animations.
- `intro.json`: 948 ticks including the initial black frame. Each of its 960
  screen-cell tracks contains explicit `[tick, artwork_tile]` keys. The story,
  stars, flying mages, fortress reveal, and logo reveal are all preserved.
- `title.json`: a looping 512-tick title animation, using the same artwork.
  Original frames 948 and 1460 are pixel-identical, as are 960 and 1472.
- `loading.png`: original tower scorecard layout and all four colored player
  icons. Rust draws the current tower/floor, lives and individual scores over
  the template, and dims inactive player rows.

`python3 DevUtils/extract_presentation.py /path/to/reference-frames` rebuilds
intro/title assets from RGB PPM observations named `0001.ppm` through `1459.ppm`.
The observations came from the previously archived reference implementation
of executable SHA-256
`b7bf279c1b7ce6714aca5634719d300ea60d2a0b8e073918c17a3702ee0a6e9c`.
Only the converted PNG/JSON artwork is required to build or run this repository.

The native renderer supports seeking directly to any timeline tick. Rendering
never advances the animation or game state. The title, player selection,
password entry, and tower loading are explicit native screen states.
