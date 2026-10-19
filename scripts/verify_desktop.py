#!/usr/bin/env python3
"""Run under xvfb-run: launch with no arguments, change settings, quit normally."""
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from verify_package import extract


def key(window, name):
    subprocess.run(["xdotool", "windowfocus", "--sync", window], check=True)
    subprocess.run(["xdotool", "keydown", name], check=True)
    time.sleep(0.08)
    subprocess.run(["xdotool", "keyup", name], check=True)
    time.sleep(0.12)


def verify(archive):
    with tempfile.TemporaryDirectory(prefix="micromages-window-") as directory:
        root = Path(directory) / "Micro Mages"
        extract(archive, root)
        env = {k: v for k, v in os.environ.items() if not k.startswith("BASALT_") and k != "MICRO_MAGES_ASSETS"}
        process = subprocess.Popen([str(root / "app/bin/micromages")], cwd=directory, env=env,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            window = None
            for _ in range(100):
                result = subprocess.run(["xdotool", "search", "--name", "Micro Mages"], capture_output=True, text=True)
                if result.returncode == 0:
                    window = result.stdout.splitlines()[-1]
                    break
                if process.poll() is not None:
                    raise RuntimeError(process.communicate())
                time.sleep(0.1)
            assert window, "No game window appeared"
            time.sleep(0.3)
            key(window, "Escape")
            key(window, "Down")  # music
            key(window, "Left")
            for _ in range(4):
                key(window, "Down")
            key(window, "Return")  # quit
            stdout, stderr = process.communicate(timeout=15)
            assert process.returncode == 0, (stdout, stderr)
            preferences = json.loads((root / "data/config/shared/preferences.json").read_text())
            assert preferences["music"] == 90, preferences
            print("Desktop passed: no-argument launch, settings persistence and normal exit")
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate()


if __name__ == "__main__":
    verify(Path(sys.argv[1]).resolve())
