# Turrets, summoned walkers and lightning

These findings come from the original PRG listing and isolated callbacks executed
in the saved reference recompilation. They do not establish a completed campaign
in the original executable. Each JSON fixture records the source PRG digest and
its probe setup.

## Turret (source type 5)

Initializer `$f60c` adds four to the spawn X coordinate. Callback `$f619` keeps
its original facing. Before activation it remains stationary. While the boss
activation flag is set, it rises one pixel on alternating update slots until its
integer Y coordinate is below 93. It then waits `ceil(center_x / 2) + 55`
alternating updates. Subsequent waiting periods come from the shared boss timer
at `$9a`, populated by boss health plus the difficulty timing value.

On firing, the turret creates an actor of type 16 at its center plus 16 pixels
in its facing direction. It spends 16 full updates in its firing pose before
starting the next alternating-update wait. The poses are `$96d8` while waiting
and `$9845` while firing. Source type 36 has a different descriptor: its update
is render-only, and it does not use this firing lifecycle.

`turret_reference.json` records 600 updates in each of two explicit timing and
facing configurations, including the initial inactive period, rise, first wait,
recovery and repeated actor spawns. Child actors are removed after recording so
that slot exhaustion does not change this isolated turret experiment.

## Summoned walker (source type 16)

This actor is not a straight projectile. Initializer `$d2c2` gives it one remaining
wall turn, with health six in normal mode or seven in hard mode. Callback `$d2cb`
first lets it fall under gravity. On its first grounded update it switches to its
walking phase and sets the integer Y velocity to -1. Horizontal walking starts
on the following update. Walking speed is 152/256 pixels per update in normal
mode and 168/256 in hard mode.

The actor reads the preceding update's wall contact only on odd update slots.
It also turns at the original horizontal screen limits. Turning changes facing
but does not reverse the horizontal velocity already selected for that update.
The second turn removes the actor before movement. Collision uses the source's
small inset character probes, preserves subpixels when snapping, and applies
gravity 37/256 every update. `physics::move_summoned_walker` provides that motion
and reports the contact to retain for the following update.

`turret_minion_reference.json` records the complete normal and hard lifetimes in
a synthetic corridor: 94 and 84 updates respectively. The regression test
compares every position, velocity and alive flag, including landing, the initial
hop, both turns and removal.

## Lightning (source type 27)

The initial dormant wait is `floor(center_x / 2)`. Callback `$c412` then advances
through a warning of 40 normal-mode updates or 27 hard-mode updates, followed by
16 active updates and a 68-update dormant wait. There is no damage during the
warning or the transition into the active phase.

The visual sprite occupies eight by 32 pixels, but damage uses one eight-by-eight
probe whose center moves vertically. Before decrementing the active timer,
`timer & 3` chooses the offset `[-12, 4, 12, -4]`. Thus the first four damaging
updates test offsets -12, -4, 12 and 4. A permanently tall damage rectangle would
not reproduce the source timing.

Boss lightning starts with a doubled warning, 80 or 54 updates. On activation it
creates two more active segments at Y+32 and Y+64. The segments share the same
16-update active lifetime and are removed when it finishes. The final active
update can still damage a player before removal. The middle and end poses come
from `$9705` and `$96eb`; the warning pose is `$96f4`.

`lightning_reference.json` covers normal and hard modes with ordinary and boss
lightning. Four fixed players have health and invulnerability reset before each
update, allowing the fixture to record the actual source contact mask on every
update independently of hurt protection. It also records every segment's
position, phase and timer, including chain creation and removal.

The lightning probe removes non-lightning actors spawned by synthetic damage
after each update. This matters because restoring health and invulnerability on
every update otherwise accumulates hurt fairies until the original actor-slot
allocator evicts the lightning itself. The recorded fixture therefore measures
the lightning lifecycle rather than an artifact of repeated forced damage.
