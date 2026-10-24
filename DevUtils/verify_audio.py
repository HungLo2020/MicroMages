#!/usr/bin/env python3
"""Verify the shipped game's audio through host ALSA with a file-backed PCM.

Runs the windowed build, so provide a display (CI uses xvfb-run) and a Vulkan 1.3
driver (Mesa lavapipe is enough).
"""
import json
import os
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path
from verify_package import extract


def verify(archive):
    with tempfile.TemporaryDirectory(prefix="micromages-audio-") as directory:
        root = Path(directory)
        extract(archive, root)
        recording = root / "output.pcm"
        os.mkfifo(recording)
        descriptor = os.open(recording, os.O_RDWR | os.O_NONBLOCK)
        stopped = threading.Event()
        received = [0, False]

        def capture():
            while not stopped.is_set():
                try:
                    samples = os.read(descriptor, 8192)
                    received[0] += len(samples)
                    received[1] |= any(samples)
                except BlockingIOError:
                    pass
                # A null PCM has no hardware clock. Throttle the FIFO consumer
                # so CI cannot generate hundreds of MB of silence per second.
                time.sleep(0.002)

        consumer = threading.Thread(target=capture, daemon=True)
        consumer.start()
        config = root / "alsa.conf"
        # Host ALSA's file and null plugins exercise real device negotiation and
        # playback without using a developer's speakers or requiring a sound card.
        config.write_text('pcm.!default { type file slave.pcm { type null } '
                          f'file {json.dumps(str(recording))} format "raw" }}\n')
        env = {k: v for k, v in os.environ.items()
               if not k.startswith("BASALT_") and k != "MICRO_MAGES_ASSETS"}
        env.update(ALSA_CONFIG_PATH=str(config))
        try:
            result = subprocess.run([str(root / "app/bin/micromages"), "--stage", "0", "--frames", "120"],
                                    cwd=root, env=env, capture_output=True, text=True, timeout=30)
            assert result.returncode == 0, (result.stdout, result.stderr)
            assert "Audio unavailable" not in result.stderr, result.stderr
            assert received[0] > 4096 and received[1], "No non-silent samples reached host ALSA"
            print(f"Audio passed: host ALSA initialized and received {received[0]} bytes including non-silent PCM")
        finally:
            stopped.set()
            consumer.join(timeout=5)
            os.close(descriptor)


if __name__ == "__main__":
    verify(Path(sys.argv[1]).resolve())
