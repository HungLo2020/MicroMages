# Projectile behavior evidence

Source identity is recorded in `player-behavior.md`. Static observations below
use `analysis/game-disassembly.txt` and the original program's mode tables.

- Normal/charged spells: speed3 pixels/tick, lifetime37/41 ticks and damage2/6.
  The source decrements lifetime before movement. A charged shot consuming less
  than6 health converts into an ordinary shot instead of vanishing
  (`$c954..$c971`); its remaining lifetime is retained. Enemy callback status2
  ignores a shot without consuming it.
- Ghost freezing shot: type`$39`, lifetime20 (`$f6a5`), no ordinary damage.
  The world applies target-dependent freeze duration and shot-count limits.
- Goat bubble: type`$3b`, callback`$9922`. Initial velocity is horizontally
  ±152/256 and vertically+152/256 in normal mode; hard mode uses186/256.
  Upward acceleration is2/256 or3/256 respectively (`$d33b` parameter table).
  Thus a bubble initially falls before curving upward. When vertical integer
  velocity reaches−5, the fraction becomes255, limiting it to−1025/256.
- Troll spear: type`$38`, callback`$8f30`. Initial velocity±1,−3. Gravity37/256
  applies before integration. When vertical integer velocity reaches3, its
  fraction becomes3 after integration (771/256 for the next tick).
- Orb: type`$29`, callback`$f9ed`. Supplied velocity integrates linearly.

Bubble, spear and orb callbacks do not resolve terrain collisions. They perform
player interaction and screen-bound despawning instead. Their source lifetime is
not an arbitrary180-tick timeout. `advance_motion` and `age` preserve this division;
the world owns bounds, contacts, damage and object removal.

`tests/fixtures/projectile-motion.csv` records80 successive source callback ticks
for each of normal bubble, hard bubble, spear and orb, with no players in their
path. Tests compare all320 positions and velocities exactly. These controlled
reference-recompilation probes are numerical regressions, not shipped-executable
or full-campaign observations. Generic spark appearance is not evidence of a
separate recovered damaging projectile; original inhale particles type`$3c` are
cosmetic and must not be assigned damage merely from their appearance.

Freezing terrain interaction is intentionally different from a damaging spell:
`$f6bc` enters `$c7ce` with carry set; `$c829` then lets it pass an ordinary solid
wall. It still damages destructible terrain as an ordinary2-damage shot, despite
having zero actor damage. Destroying an ordinary block consumes it; opening a
chest lets it continue. A damaging spell is consumed by chest opening.

Reinforced metatile16 starts with6 durability (`$9484`). Ordinary/freezing shots
remove2, charged shots6. Partial hits return action2; destruction replaces the
whole metatile with0 and returns action3. The original tracks8 partially damaged
blocks in a circular cache (`$04e1/$04e9/$04f1`), so older partial damage can be
forgotten when a ninth distinct block is damaged. Chest17 has no durability:
it immediately becomes18, which cannot be opened again. World terrain tests own
these mutation and pickup consequences; projectile tests verify separate actor
and terrain damage values.

Bomb fragments (type `$34`) initialize both velocity fractions from `$0576`
(`$80` in both modes), with zero integer parts; the second fragment reverses X.
`$8c45` samples the center tile after each axis. Solid horizontal contact reverses
X and applies the reversed displacement; solid or downward one-way vertical
contact does the same for Y. Each axis contact counts, and the fourth contact
removes the fragment and spawns cosmetic debris. There is no gravity. The direct
sprite cycle is `$da,$db,$dc,$db`, four ticks each, with vertical flip on its final
phase. `bomb-fragment.csv` records 48 ticks of the original callback in a 16-pixel
room, including all four contacts and removal; the native motion test compares
every position, velocity, contact count and alive flag.
