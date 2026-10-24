# Startup and tower presentation evidence

The earlier reconstruction replaced the original opening with an invented text
menu and started levels without the tower scorecard. Gameplay-only completion
recordings could not detect either omission. Ordinary launch now exercises
native Intro, Title, PlayerSelect, Password, Loading and Playing states.

## Original observations

The archived reference executes the source main path rooted at 8F72 and A091;
title interaction is E127–E222. Capturing every original update without input
established these milestones:

| Source update | Visible state |
| --- | --- |
| 1–947 | Opening story, starfield, flying mages, fortress and logo reveal |
| 915 | Logo reveal begins |
| 948 | Original title prompt is initialized (`a4=ff`) |
| 948 onward | Animated title waiting for confirmation |

The native global update count matches these startup frame numbers. Independent
PNG observations at updates 120, 600, 900 and 960 are in
`tests/fixtures/presentation`, separate from the PNG atlas and JSON tracks used
by the renderer. Title frames 948/1460 and 960/1472 are identical, establishing
the captured 512-tick loop boundary.

The source requires three confirmations: title to player count, player count
to password, then password to the tower. The native implementation preserves
those states and accepts the original normal/hard tower passwords decoded from E24D.
Empty password starts a new run. Invalid passwords clear the entry and play
the original error sound. The native player selection also exposes hard mode
with the Shoot action (X for Player 1), instead of requiring the original button
hold. Password digits range from 1 to 9; untouched positions remain blank.

For a fresh solo start confirmed on original update 1250, the scorecard is
fully visible by 1258, begins fading at 1510, and is black at 1512. Map setup
continues through 1544, followed by the level reveal at 1545–1547. The native
loading screen therefore occupies 298 ticks. It holds world simulation still,
shows the actual tower, reserve lives, and individual player scores, then
reveals the level. Native fades use RGB brightness; the source changes palette
indices, so intermediate fade shades are not claimed to be pixel-identical.
The initial outgoing-screen fade is replaced by a short black transition.
The fully visible loading layout is compared to solo/four-player source PNGs.

Direct stage starts used by tools and gameplay recordings intentionally bypass
presentation. Ordinary UI starts, continues and campaign transitions retain
presentation. This separation is tested explicitly rather than inferring a
complete launch from a gameplay replay.

## Asset format

Artwork is stored once in an RGB PNG atlas; editable JSON contains per-cell
animation keys. This is a native cinematic timeline, not emulated rendering:
there are no controller registers, CPU state, executable data or memory maps
in these assets or their playback code. See `assets/presentation/README.md`
and `DevUtils/extract_presentation.py` for the conversion format.

The separate secret challenge password `7195` selects an additional source
difficulty bit, not ordinary hard mode. It is rejected rather than silently
starting the wrong mode; that challenge variant is not implemented.
