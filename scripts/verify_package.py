#!/usr/bin/env python3
"""Exercise the exact release ZIP with read-only app files and portable user data."""
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path, PurePosixPath
from verify_artwork import verify_artwork


def extract(archive, destination):
    with zipfile.ZipFile(archive) as zip_file:
        for info in zip_file.infolist():
            path = PurePosixPath(info.filename)
            assert path.parts[0] == "app" and not path.is_absolute() and ".." not in path.parts
            assert not info.is_dir(), "Package contains file entries only"
            assert info.external_attr >> 16 & 0o170000 != 0o120000, "No symlinks"
            target = destination.joinpath(*path.parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(zip_file.read(info))
            target.chmod(info.external_attr >> 16 & 0o777)
    assert os.access(destination / "app/bin/micromages", os.X_OK)


def snapshot(path):
    return {str(p.relative_to(path)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in path.rglob("*") if p.is_file()}


def launch(root, cwd, **overrides):
    env = {k: v for k, v in os.environ.items()
           if not k.startswith("BASALT_") and k != "MICRO_MAGES_ASSETS"}
    env.update(overrides)
    return subprocess.run([str(root / "app/bin/micromages"), "--headless", "--frames", "4"],
                          cwd=cwd, env=env, check=True, capture_output=True, text=True, timeout=30)


def verify(archive):
    with tempfile.TemporaryDirectory(prefix="micromages-release-") as directory:
        temporary = Path(directory)
        root = temporary / "library with spaces/MicroMages"
        extract(archive, root)
        verify_artwork(root / "app/basalt")
        source = Path(__file__).resolve().parents[1] / "packaging/artwork"
        for name in ["cover.png", "icon.png"]:
            assert (root / "app/basalt" / name).read_bytes() == (source / name).read_bytes()
        before = snapshot(root / "app")
        for p in (root / "app").rglob("*"):
            p.chmod((p.stat().st_mode & 0o555) if p.is_file() else 0o555)
        (root / "app").chmod(0o555)
        try:
            # Direct launch outside the install directory, without Basalt or a checkout.
            assert "Screen: Intro" in launch(root, temporary).stdout
            preferences = root / "data/config/shared/preferences.json"
            custom = '{"version":1,"music":30,"effects":70}\n'
            preferences.write_text(custom)
            save = root / "data/save/user-content.txt"
            save.write_text("user progress must survive updates")
            env = {"BASALT_GAME_DIR": str(root), "BASALT_APP_DIR": str(root / "app"),
                   "BASALT_SAVE_DIR": str(root / "data/save"),
                   "BASALT_CONFIG_DIR": str(root / "data/config"),
                   "BASALT_SHARED_CONFIG_DIR": str(preferences.parent),
                   "BASALT_CACHE_DIR": str(root / "data/cache")}
            launch(root, root, **env)
            assert preferences.read_text() == custom
            assert snapshot(root / "app") == before, "Runtime wrote into app"
        finally:
            for p in [root / "app", *(root / "app").rglob("*")]:
                if p.is_dir():
                    p.chmod(0o755)
        data_before = snapshot(root / "data")
        shutil.rmtree(root / "app")
        extract(archive, root)  # Basalt replaces app, retaining data.
        launch(root, temporary)
        assert snapshot(root / "data") == data_before
        moved = temporary / "moved install"
        root.rename(moved)
        launch(moved, temporary)
        assert snapshot(moved / "data") == data_before
        # Category overrides are honored independently of the executable location.
        shared = moved / "data/custom-shared"
        launch(moved, temporary, BASALT_SHARED_CONFIG_DIR=str(shared))
        assert json.loads((shared / "preferences.json").read_text())["music"] == 100
        assert snapshot(moved / "app") == before
        print("Package passed: portable startup, read-only app, preferences, update and relocation")


if __name__ == "__main__":
    verify(Path(sys.argv[1]).resolve())
