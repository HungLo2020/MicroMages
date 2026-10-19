import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from PIL import Image
from prepare_artwork import prepare
from verify_artwork import verify_artwork

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "packaging/artwork"


class ArtworkTests(unittest.TestCase):
    def test_checked_in_artwork_meets_standard_and_source_hashes_match(self):
        verify_artwork(ART)
        for source in json.loads((ART / "sources.json").read_text())["sources"]:
            self.assertEqual(hashlib.sha256((ART / source["file"]).read_bytes()).hexdigest(), source["sha256"])

    def test_artwork_is_reproducible_from_preserved_steam_sources(self):
        with tempfile.TemporaryDirectory() as temporary:
            prepare(temporary)
            for name in ["cover.png", "icon.png"]:
                with Image.open(ART / name) as actual, Image.open(Path(temporary) / name) as regenerated:
                    self.assertEqual(actual.mode, regenerated.mode)
                    self.assertEqual(actual.size, regenerated.size)
                    self.assertEqual(actual.tobytes(), regenerated.tobytes())

    def test_rejects_incompatible_cover_and_icon(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            prepare(directory)
            for size, alpha in [((600, 900), 255), ((300, 400), 255), ((600, 800), 254)]:
                Image.new("RGBA", size, (0, 0, 0, alpha)).save(directory / "cover.png")
                with self.assertRaises(ValueError):
                    verify_artwork(directory)
            prepare(directory)
            Image.new("RGBA", (512, 256), (255, 0, 0, 255)).save(directory / "icon.png")
            with self.assertRaises(ValueError):
                verify_artwork(directory)
