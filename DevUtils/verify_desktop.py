#!/usr/bin/env python3
"""Run under Xvfb: exercise settings, remapping, window resizing and persistence.

Requires a Vulkan 1.3 driver (Mesa lavapipe is enough under Xvfb).
"""
import argparse
from contextlib import contextmanager
import json
import os
import shutil
import subprocess
import tempfile
import time
from pathlib import Path
from verify_package import extract


@contextmanager
def desktop_session():
    # Borderless fullscreen needs a window manager (Xvfb alone has none).
    manager = subprocess.Popen(["openbox", "--sm-disable"], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    try:
        time.sleep(0.5)
        if manager.poll() is not None:
            raise RuntimeError(f"Openbox failed: {manager.stderr.read()}")
        yield
    finally:
        if manager.poll() is None:
            manager.terminate()
        manager.wait(timeout=5)
        manager.stderr.close()


def key(window, name):
    subprocess.run(["xdotool", "windowfocus", "--sync", window], check=True)
    subprocess.run(["xdotool", "keydown", name], check=True)
    # Software rendering on shared CI runners can take longer than 80 ms per
    # frame. Hold and release long enough for both input states to be sampled.
    time.sleep(0.25)
    subprocess.run(["xdotool", "keyup", name], check=True)
    time.sleep(0.25)


def find_window(process):
    for _ in range(100):
        result = subprocess.run(["xdotool", "search", "--name", "Micro Mages"], capture_output=True, text=True)
        if result.returncode == 0:
            return result.stdout.splitlines()[-1]
        if process.poll() is not None:
            raise RuntimeError(process.communicate())
        time.sleep(0.1)
    raise AssertionError("No game window appeared")


def verify(archive=None, binary=None, screenshot=None):
    with tempfile.TemporaryDirectory(prefix="micromages-window-") as directory:
        root = Path(directory) / "Micro Mages"
        if binary:
            (root / "app/bin").mkdir(parents=True)
            shutil.copy2(binary, root / "app/bin/micromages")
            shutil.copytree(Path(__file__).resolve().parents[1] / "assets", root / "app/assets")
        else:
            extract(archive, root)
        env = {k: v for k, v in os.environ.items() if not k.startswith("BASALT_") and k != "MICRO_MAGES_ASSETS"}
        # Always use the isolated X server, even from a Wayland desktop session.
        env["WAYLAND_DISPLAY"] = str(Path(directory) / "no-wayland-display")
        def start():
            return subprocess.Popen([str(root / "app/bin/micromages")], cwd=directory, env=env,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        process = start()
        try:
            window = find_window(process)
            time.sleep(1.0)  # Allow the first software-rendered frame to finish.
            key(window, "Escape")
            key(window, "Down")  # music
            key(window, "Left")
            for _ in range(2):
                key(window, "Down")
            key(window, "Return")  # controls
            for _ in range(6):
                key(window, "Down")  # player 1 jump
            key(window, "Return")
            key(window, "space")
            controls_path = root / "data/config/shared/controls.json"
            controls = json.loads(controls_path.read_text())
            assert controls["keyboard"][0]["jump"] == ["space"], controls
            assert controls["keyboard"][1]["jump"] == ["z", "enter"], controls
            key(window, "Escape")  # settings, controls selected
            key(window, "Down")
            key(window, "Return")  # display
            key(window, "Right")  # 1280x720
            key(window, "Down")
            key(window, "Right")  # smooth scaling
            key(window, "Down")
            key(window, "Down")  # skip fullscreen
            key(window, "Return")  # apply in place
            time.sleep(0.3)
            window = find_window(process)
            display_path = root / "data/config/display.json"
            display = json.loads(display_path.read_text())
            assert (display["width"], display["height"], display["scaling"]) == (1280, 720, "smooth"), display
            subprocess.run(["xdotool", "windowsize", "--sync", window, "1000", "600"], check=True)
            time.sleep(0.8)  # resize persistence is debounced
            display = json.loads(display_path.read_text())
            assert (display["width"], display["height"]) == (1000, 600), display
            # Any window shape is accepted; portrait sizes letterbox vertically.
            subprocess.run(["xdotool", "windowsize", window, "600", "1000"], check=True)
            time.sleep(0.8)
            display = json.loads(display_path.read_text())
            assert (display["width"], display["height"]) == (600, 1000), display
            portrait_geometry = subprocess.check_output(["xdotool", "getwindowgeometry", "--shell", window], text=True)
            assert "WIDTH=600\n" in portrait_geometry and "HEIGHT=1000\n" in portrait_geometry, portrait_geometry
            subprocess.run(["xdotool", "windowsize", window, "1000", "600"], check=True)
            time.sleep(0.8)
            key(window, "alt+Return")
            time.sleep(0.8)
            display = json.loads(display_path.read_text())
            assert display["fullscreen"] and (display["width"], display["height"]) == (1000, 600), display
            fullscreen_geometry = subprocess.check_output(["xdotool", "getwindowgeometry", "--shell", window], text=True)
            assert "WIDTH=1000\n" not in fullscreen_geometry, fullscreen_geometry
            if screenshot:
                subprocess.run(["import", "-window", window, str(screenshot.with_stem(screenshot.stem + "-fullscreen"))], check=True)
            key(window, "alt+Return")
            time.sleep(0.8)
            display = json.loads(display_path.read_text())
            assert not display["fullscreen"], display
            if screenshot:
                # Optional local visual check; ImageMagick is not needed in CI.
                subprocess.run(["import", "-window", window, str(screenshot)], check=True)
            key(window, "Escape")  # settings, display selected
            for _ in range(2):
                key(window, "Down")
            key(window, "Return")  # quit
            stdout, stderr = process.communicate(timeout=15)
            assert process.returncode == 0, (stdout, stderr)
            preferences = root / "data/config/shared/preferences.json"
            assert json.loads(preferences.read_text())["music"] == 90
            before = {p: p.read_bytes() for p in (controls_path, display_path, preferences)}

            process = start()
            window = find_window(process)
            geometry = subprocess.check_output(["xdotool", "getwindowgeometry", "--shell", window], text=True)
            assert "WIDTH=1000\n" in geometry and "HEIGHT=600\n" in geometry, geometry
            assert all(p.read_bytes() == data for p, data in before.items())
            key(window, "alt+Return")
            time.sleep(0.8)
            assert json.loads(display_path.read_text())["fullscreen"]
            key(window, "Escape")
            key(window, "Up")  # wrap to quit
            key(window, "Return")
            stdout, stderr = process.communicate(timeout=15)
            assert process.returncode == 0, (stdout, stderr)

            process = start()
            window = find_window(process)
            time.sleep(0.5)
            geometry = subprocess.check_output(["xdotool", "getwindowgeometry", "--shell", window], text=True)
            expected = {line for line in fullscreen_geometry.splitlines() if line.startswith(("WIDTH=", "HEIGHT="))}
            assert expected <= set(geometry.splitlines()), geometry
            key(window, "alt+Return")
            time.sleep(0.8)
            geometry = subprocess.check_output(["xdotool", "getwindowgeometry", "--shell", window], text=True)
            assert "WIDTH=1000\n" in geometry and "HEIGHT=600\n" in geometry, geometry
            key(window, "Escape")
            key(window, "Up")
            key(window, "Return")
            stdout, stderr = process.communicate(timeout=15)
            assert process.returncode == 0, (stdout, stderr)
            assert all(p.read_bytes() == data for p, data in before.items())
            print("Desktop passed: keyboard remap, display apply, fullscreen toggle/restore, portrait resize, settings persistence and relaunch")
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", nargs="?", type=Path)
    parser.add_argument("--binary", type=Path, help="Exercise a development binary with the checkout's assets")
    parser.add_argument("--screenshot", type=Path, help="Optional ImageMagick window capture")
    args = parser.parse_args()
    if bool(args.archive) == bool(args.binary):
        parser.error("provide either an archive or --binary")
    with desktop_session():
        verify(args.archive.resolve() if args.archive else None,
               args.binary.resolve() if args.binary else None,
               args.screenshot.resolve() if args.screenshot else None)
