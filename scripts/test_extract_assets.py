"""Independent migration regressions; run python3 -m unittest discover -s scripts."""

import hashlib
import json
from pathlib import Path
import unittest

from PIL import Image

from extract_assets import Bits, Source

ROOT = Path(__file__).resolve().parents[1]


def load(relative):
    return json.loads((ROOT / relative).read_text())


class AssetMigrationTests(unittest.TestCase):
    def test_png_preserves_every_original_pixel(self):
        image = Image.open(ROOT / "assets/graphics/tiles.png").convert("RGBA")
        self.assertEqual(image.size, (128, 256))
        planes = bytearray(8192)
        for tile in range(512):
            for y in range(8):
                for x in range(8):
                    r, g, b, alpha = image.getpixel(
                        (tile % 16 * 8 + x, tile // 16 * 8 + y)
                    )
                    self.assertEqual((r, g, b), (r, r, r))
                    self.assertIn(r, (0, 85, 170, 255))
                    value = r // 85
                    self.assertEqual(alpha, 255 if value else 0)
                    planes[tile * 16 + y] |= (value & 1) << (7 - x)
                    planes[tile * 16 + y + 8] |= ((value >> 1) & 1) << (7 - x)
        # Original source-region digest, retained independently of the PNG.
        self.assertEqual(
            hashlib.sha256(planes).hexdigest(),
            "f813fe56df2d6c50dc80829487dfb0dde39c93be3d90375ba0324a72c43cf3e4",
        )

    def test_all_stage_starts_match_independent_reference(self):
        levels = load("assets/levels/levels.json")["levels"]
        reference = load("tests/fixtures/reference_level_starts.json")["levels"]
        self.assertEqual(len(levels), 27)
        self.assertEqual(len(reference), 27)
        for expected in reference:
            with self.subTest(stage=expected["id"], hard=expected["hard"]):
                level = next(
                    level
                    for level in levels
                    if (level["id"], level["hard"])
                    == (expected["id"], expected["hard"])
                )
                self.assertEqual(level["colors"], expected["colors"])
                base = level["tiles"][-960:]
                for index, tile in enumerate(expected["tiles"]):
                    if index in expected["animated_overlay_indices"]:
                        self.assertEqual(base[index], 2)
                        self.assertIn(tile, range(211, 216))
                    else:
                        self.assertEqual(base[index], tile, f"tile {index}")

    def test_native_level_geometry_and_player_starts_are_valid(self):
        for level in load("assets/levels/levels.json")["levels"]:
            self.assertTrue(
                all(
                    isinstance(spawn["entity"], str) and spawn["entity"]
                    for spawn in level["spawns"]
                )
            )
            self.assertEqual(level["player_start"]["entity"], "player_start")
            count = level["width"] * level["height"]
            self.assertEqual(level["width"], 32)
            self.assertIn(level["height"], (32, 128))
            for field in ("tiles", "palettes", "collision"):
                self.assertEqual(len(level[field]), count)
            self.assertTrue(all(0 <= tile < 256 for tile in level["tiles"]))
            self.assertTrue(all(0 <= palette < 4 for palette in level["palettes"]))
            spawn = level["player_start"]
            self.assertEqual(spawn["kind"], 33)
            self.assertIn(spawn, level["spawns"])
            self.assertTrue(0 <= spawn["x"] < 256)
            self.assertTrue(0 <= spawn["y"] < level["height"] * 8)

    def test_terrain_catalog_reconstructs_all_tile_and_collision_layers(self):
        for level in load("assets/levels/levels.json")["levels"]:
            width = level["width"]
            self.assertEqual(len(level["metatiles"]), width * level["height"] // 4)
            definitions = {d["id"]: d for d in level["terrain"]}
            for index, metatile in enumerate(level["metatiles"]):
                definition = definitions[metatile]
                left = index % (width // 2) * 2
                top = index // (width // 2) * 2
                for quadrant in range(4):
                    cell = (top + quadrant // 2) * width + left + quadrant % 2
                    self.assertEqual(
                        level["tiles"][cell], definition["tiles"][quadrant]
                    )
                    self.assertEqual(
                        level["collision"][cell], definition["collision"][quadrant]
                    )
                    self.assertEqual(level["palettes"][cell], definition["palette"])
            # A chest opens visually while its bottom collision remains unchanged.
            self.assertNotEqual(definitions[17]["tiles"], definitions[18]["tiles"])
            self.assertTrue(any(m & 16 for m in definitions[17]["collision"]))
            self.assertEqual(definitions[17]["collision"], definitions[18]["collision"])

    def test_boss_rooms_include_the_flying_boss_encounter(self):
        for level in load("assets/levels/levels.json")["levels"]:
            self.assertEqual(level["boss"], level["id"] in (2, 5, 8, 11, 12, 13))

    def test_level_music_names_match_original_audio_table(self):
        expected = [
            "spooky",
            "spooky",
            "evil",
            "heroic",
            "heroic",
            "evil",
            "mysterious",
            "mysterious",
            "evil",
            "dreamy",
            "dreamy",
            "evil",
            "march",
            "boss",
        ]
        for level in load("assets/levels/levels.json")["levels"]:
            self.assertEqual(level["music"], expected[level["id"]])
            self.assertTrue(
                (ROOT / "assets/music" / (level["music"] + ".ogg")).is_file()
            )

    def test_bit_stream_crosses_byte_boundaries_most_significant_first(self):
        source = Source(bytes([0b10110010, 0b01101101]) + bytes(32766))
        bits = Bits(source, 0x8000)
        self.assertEqual(bits.read(3), 0b101)
        self.assertEqual(bits.read(7), 0b1001001)
        self.assertEqual(bits.read(6), 0b101101)

    def test_direct_projectile_art_and_invisible_portals_match_source(self):
        sprites = {
            sprite["kind"]: sprite
            for sprite in load("assets/graphics/sprites.json")["sprites"]
        }
        for kind, tiles in {
            58: [241, 242],
            53: [243, 244],
            57: [231],
            56: [221],
            59: [248, 247, 248, 249],
        }.items():
            self.assertEqual(
                [frame["parts"][0]["tile"] for frame in sprites[kind]["frames"]],
                [256 + tile for tile in tiles],
            )
            self.assertEqual(sprites[kind]["default_frame"], 0)
        for kind in (46, 48, 49, 51):
            self.assertEqual(sprites[kind]["frames"], [{"id": 0, "parts": []}])
        for kind, frame in {4: 0, 22: 2, 28: 0, 33: 1, 40: 1, 41: 1}.items():
            self.assertEqual(sprites[kind]["default_frame"], frame)
            self.assertIn(frame, [f["id"] for f in sprites[kind]["frames"]])

    def test_player_animation_timing_matches_recovered_records(self):
        animations = {
            a["id"]: a for a in load("assets/graphics/animations.json")["animations"]
        }
        run = animations["player_run"]
        self.assertEqual([f["sprite"] for f in run["frames"]], [2, 3, 4, 5])
        self.assertEqual([f["duration"] for f in run["frames"]], [4, 4, 4, 4])
        self.assertEqual(run["loop_start"], 0)
        self.assertEqual(animations["player_idle"]["frames"][0]["sprite"], 1)
        for animation in animations.values():
            self.assertLess(animation["loop_start"], len(animation["frames"]))
            self.assertTrue(all(f["duration"] > 0 for f in animation["frames"]))


if __name__ == "__main__":
    unittest.main()
