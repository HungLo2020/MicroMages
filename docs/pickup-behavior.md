# Collectible behavior evidence

The original routines 9308/9325 and BD9C–BE32 initialize and update collectible
actors. `tests/fixtures/pickup_lifetime_reference.json` records 701 consecutive
updates for each of six collectible kinds in each actor cadence phase (slots
4–7), with an empty map and no players. Each row independently records active
kind, fractional center coordinates, lifetime counter, animation frame and
render visibility. The native test compares every field on every update.

Gems, feathers and swap items remain stationary. Their lifetime counter starts
at 155 and decrements once per four updates, giving 617–620 updates depending
on phase. They blink during the final 29 counter values; their animation also
pauses on hidden updates. Fairy movement uses signed 1/256-pixel velocity and a
half-rate oscillator, without gravity or the ordinary lifetime counter.
Collection is checked on the same quarter/half cadence. Offscreen items expire.

Block drops use the broken cell center (BF22); chest drops use the left-aligned
chest center six pixels into the 16-pixel cell, eight pixels above (BECB).
All five pickup animation tracks have named editable JSON entries and use the
packaged PNG sprite artwork. Default frame zero previously made these dropped
items invisible.

The native world assigns the first unused collectible cadence phase. It does
not yet reproduce the original shared actor-slot allocator and its replacement
priorities for all simultaneous enemies, projectiles and collectibles. The
controlled trace tests establish behavior for every phase; they do not prove
slot allocation parity under actor saturation.
