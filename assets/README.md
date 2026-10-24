# Editable game assets

The game reads these PNG, JSON, and Ogg files directly. No original executable,
program image, packed tile planes, or build-time extraction is required.

## Graphics

`graphics/tiles.png` is a standard 128 × 256 RGBA PNG atlas containing all 512
original 8 × 8 tiles, numbered left to right and then top to bottom. Background
tiles occupy IDs 0–255 and sprite tiles occupy IDs 256–511. Edit with a pixel art
editor with antialiasing disabled. Pixel values 0, 85, 170, and 255 select the four
colors in a palette; zero is transparent. This preserves palette swaps without
duplicating the same sprite four times. `graphics/palette.json` contains the 64
original RGB colors as integers.

`graphics/sprites.json` describes each recovered sprite's component tiles,
relative pixel offsets, flips, and default palette. Sprite frame IDs match the
animation `sprite` fields. The segmented boss has an explicit component
sprite definition despite using a separate drawing routine in the source.
Ordinary and charged spells have horizontal frame 0 and vertical frame 1;
freeze shots, spears, and bubbles also use recovered direct-drawing tiles.
An explicit empty sprite frame means intentionally invisible: portal triggers
and the boss entrance trigger use art already present in the terrain. Do not replace
empty frames with placeholder rectangles. Known `default_frame` fields specify
initial display frames; checkpoints use frame 0 before activation and frame 2
after activation. The normal exit's initial frame 0 is also intentionally empty.

`graphics/animations.json` contains durations in animation-clock ticks, sprite frame
IDs, drawing offsets, optional wand attachments, and loop-start frame indices.
Known player, enemy and collectible animations have semantic names. The remaining catalog
entries preserve recovered animations whose gameplay role has not been named;
`source_address` is extraction provenance, not executable code. Gameplay selects
when each clock advances: some source behaviors use half/quarter-rate updates,
and expiring collectibles pause their animation on hidden blink frames.

## Levels

`levels/levels.json` contains 27 playable stage/mode definitions: 13 normal stages
and 14 hard stages. Normal mode's stage 13 is an ending transition, not a room.
Each record has a stage ID, mode, tower/floor, music name, boss marker, dimensions,
and these editable layers:

- `metatiles`: row-major 16 × 16 terrain IDs, half the tile layer's width and
  height. This is the editable terrain-object layer.
- `terrain`: definitions keyed by ID, with four `tiles`, four `collision` values,
  and one `palette`. Quadrants are top-left, top-right, bottom-left, bottom-right.
  These definitions let gameplay replace terrain without guessing its graphics.
- `tiles`: row-major 8 × 8 atlas tile IDs, from top to bottom.
- `palettes`: one four-color palette selection per tile.
- `collision`: one material flag value per tile. The flag table below defines each recovered property.
- `colors`: the stage's 16 background and 16 sprite palette color indices.
- `spawns`: a descriptive `entity` name, world position, and facing flags.
  Positions are in pixels with positive Y downward. `player_start` identifies
  player entry. The numeric `kind` links to the original sprite catalog; it is
  retained separately from the editable gameplay identity. Distinct portal
  names identify linked portal groups; the color labels are native authoring
  names, not a claim about original source identifiers.
- `player_start`: the initial player spawn copied from the original spawn stream.

Regular rooms are 256 × 1024 pixels; the last hard-mode boss room is 256 × 256.
Hard-mode layouts and palette changes are expanded separately. Collision data
remains independent of visual substitutions such as invisible hard-mode blocks.

### Destructible terrain and chests

Terrain IDs 1–15 encode four independently breakable quadrants. A hit removes
one quadrant bit using masks 14, 13, 11, or 7 for top-left, top-right, bottom-left,
or bottom-right; replacing the terrain definition updates its graphics and
collision together. ID 16 is a reinforced block with a separate damage cycle.
ID 17 is a chest and changes to the distinct open-chest definition 18 after
opening. It should not be erased as ordinary background. These transitions are
recovered from source routine `9495` and its quadrant mask table `9572`.

### Collision flags

A tile combines flags by adding their values; for example, 6 means a ladder
with a one-way platform and 17 means a solid breakable tile.

| Value | Property | Source behavior |
| --- | --- | --- |
| 1 | Solid | Horizontal and vertical obstruction (`9b59`, `9b5f`). |
| 2 | Ladder | Climbable material; can combine with a platform. |
| 4 | One-way platform | Blocks downward movement at its upper surface (`9b5f`). |
| 16 | Breakable | Projectile collision enters block-damage handling (`c7df..c81e`). |
| 64 | Hazard | Player contact damage material. |
| 128 | Slippery candidate | Reduced acceleration/friction when the stage enables ice (`ca92`, `cc8f`). |

In the recovered game, ice is enabled in hard mode's second tower. Bit 128
alone does not enable ice in every stage; this is a stage rule as well as a
tile property. Background color and tile appearance do not determine collision.

## Presentation

`presentation/cinematic.png`, `intro.json` and `title.json` hold the opening and
looping title as deduplicated artwork and editable per-cell timeline keys.
`loading.png` supplies the tower scorecard artwork; Rust draws the current tower,
mode, players, lives, scores and password. `presentation/outcomes/` contains a
separate PNG atlas and JSON tracks for Game Over, the normal ending and the four
player-count variants of the hard ending. Scores, selection state and the final
starfield are drawn by native Rust code.

These are standard image and timeline assets, not program snapshots. The source
capture process and exact comparison scope are documented in
`docs/startup-presentation.md`, `docs/presentation-behavior.md` and
`presentation/README.md`. Intermediate loading fades use native RGB dimming,
which does not exactly match the original palette-index fade.

## Audio

`music/` contains nine original Ogg tracks. `sfx/` contains 37 original Ogg sound
files. These are ordinary editable audio files and have not been re-encoded.

## Recovery and verification

`DevUtils/extract_assets.py` documents the tile, sprite, animation and level
migration. `DevUtils/extract_presentation.py` converts reference image observations
to cinematic PNG/JSON assets.
The gameplay extractor takes explicit external paths for archived program and tile regions;
it is not invoked by Cargo and those source regions are not runtime assets.
Pillow is needed only for this development tool and its migration tests.

The PNG is verified pixel-for-pixel by reconstructing the original planar tile
encoding in a test and checking its independently retained SHA-256 digest.
All 27 stage palettes and 25,920 initial-screen tile cells were compared with
independent execution of the previous native recompilation. The fixture in
`tests/fixtures/reference_level_starts.json` preserves those observations.
Documented animated sparkle overlays are excluded from base-geometry comparisons.
This checks initial-screen extraction; it does not establish complete gameplay
or whole-level behavioral equivalence.

Run migration checks with `python3 -m unittest discover -s DevUtils`.
