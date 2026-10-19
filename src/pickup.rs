//! Block and chest rewards recovered from $be98..$bf22.
//! Enemy deaths award score; this table is not an enemy loot probability table.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickupKind {
    SmallGem,
    MediumGem,
    LargeGem,
    Fairy,
    Feather,
    Swap,
}
impl PickupKind {
    pub const fn source_id(self) -> u8 {
        match self {
            Self::SmallGem => 0x15,
            Self::MediumGem => 0x0f,
            Self::LargeGem => 0x17,
            Self::Fairy => 0x2b,
            Self::Feather => 0x20,
            Self::Swap => 0x11,
        }
    }
    /// Original decimal score units, before display scaling.
    pub const fn score_units(self) -> u32 {
        match self {
            Self::SmallGem => 1,
            Self::MediumGem => 2,
            Self::LargeGem => 5,
            _ => 0,
        }
    }
}

/// Native collectible state recovered from $9308/$9325 and $bd9c/$bdbd.
/// Phase is the low two bits of the original actor slot.
#[derive(Clone, Debug)]
pub struct Pickup {
    pub kind: PickupKind,
    pub phase: u8,
    pub remaining: u8,
    pub visible: bool,
    pub animation_ticks: u64,
    oscillator: u8,
    velocity: i16,
}
impl Pickup {
    pub fn new(kind: PickupKind, phase: u8) -> Self {
        Self {
            kind,
            phase: phase & 3,
            remaining: if kind == PickupKind::Fairy { 0 } else { 155 },
            visible: true,
            animation_ticks: 0,
            oscillator: 16,
            velocity: -6,
        }
    }
    pub fn animation(&self) -> &'static str {
        match self.kind {
            PickupKind::SmallGem | PickupKind::MediumGem => "pickup_gem",
            PickupKind::LargeGem => "pickup_large_gem",
            PickupKind::Fairy => "pickup_fairy",
            PickupKind::Feather => "pickup_feather",
            PickupKind::Swap => "pickup_swap",
        }
    }
    pub fn can_collect(&self, tick: u64) -> bool {
        (tick + u64::from(self.phase)).is_multiple_of(if self.kind == PickupKind::Fairy {
            2
        } else {
            4
        })
    }
    pub fn step(&mut self, tick: u64, body: &mut crate::physics::Body) -> bool {
        if self.kind == PickupKind::Fairy {
            if self.can_collect(tick) {
                self.oscillator = self.oscillator.wrapping_add(1);
                self.velocity += if self.oscillator & 63 < 32 { -6 } else { 6 };
            }
            body.velocity.y = f32::from(self.velocity) / 256.0;
            body.position.y += body.velocity.y;
        } else {
            if self.can_collect(tick) {
                self.remaining = self.remaining.saturating_sub(1);
            }
            if self.remaining == 0 {
                self.visible = false;
                return false;
            }
            let dispatcher = 2 - tick.saturating_sub(1) % 3;
            self.visible =
                self.remaining >= 30 || !(u64::from(self.phase) + dispatcher).is_multiple_of(3);
        }
        if self.visible {
            self.animation_ticks += 1;
        }
        true
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DropContext {
    pub active_players: u8,
    /// Height of the broken block below the camera top in original screen pixels.
    pub screen_y: u8,
    /// One random byte supplied by the world's deterministic generator.
    pub random: u8,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Rewards {
    /// Decimal count modulo 100; replaces the original packed-BCD counter.
    broken_blocks: u8,
}
impl Rewards {
    pub fn new(broken_blocks: u8) -> Self {
        Self {
            broken_blocks: broken_blocks % 100,
        }
    }
    pub fn broken_blocks(&self) -> u8 {
        self.broken_blocks
    }

    pub fn break_block(&mut self, context: DropContext) -> Option<PickupKind> {
        let units = self.broken_blocks % 10;
        let tens = self.broken_blocks / 10;
        let reward = if units == 0 || units == 5 {
            Some(PickupKind::SmallGem)
        } else if units == tens {
            Some(self.special_reward(units, context, false))
        } else {
            None
        };
        self.advance();
        reward
    }

    pub fn open_chest(&mut self, context: DropContext) -> PickupKind {
        let reward = self.special_reward(self.broken_blocks % 10, context, true);
        self.advance();
        reward
    }

    fn advance(&mut self) {
        self.broken_blocks = (self.broken_blocks + 1) % 100;
    }

    fn special_reward(&self, digit: u8, context: DropContext, chest: bool) -> PickupKind {
        use PickupKind::*;
        const TABLE: [PickupKind; 13] = [
            Fairy, Feather, Feather, LargeGem, Fairy, LargeGem, Fairy, Fairy, Feather, MediumGem,
            LargeGem, LargeGem, Swap,
        ];
        let mut index = usize::from(digit);
        if context.active_players < 4 {
            index += 1;
        }
        if context.active_players < 2 {
            index += 1;
        } else if index == 9 || (context.screen_y >= 128 && context.random < context.screen_y - 128)
        {
            index = 12;
        }
        let selected = TABLE[index];
        if !chest && selected == LargeGem {
            MediumGem
        } else {
            selected
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn motion_expiry_blink_and_animation_match_original_actor_traces() {
        #[derive(serde::Deserialize)]
        struct Case {
            kind: u8,
            slot: u8,
            frames: Vec<[i32; 7]>,
        }
        let cases: Vec<Case> = serde_json::from_str(include_str!(
            "../tests/fixtures/pickup_lifetime_reference.json"
        ))
        .unwrap();
        let assets = crate::assets::Assets::load(&crate::assets::default_root()).unwrap();
        for case in cases {
            let kind = [
                PickupKind::SmallGem,
                PickupKind::MediumGem,
                PickupKind::LargeGem,
                PickupKind::Fairy,
                PickupKind::Feather,
                PickupKind::Swap,
            ]
            .into_iter()
            .find(|k| k.source_id() == case.kind)
            .unwrap();
            let mut pickup = Pickup::new(kind, case.slot);
            let mut body = crate::physics::Body::new(
                crate::physics::Vec2::new(124.0, 68.0),
                crate::physics::Vec2::new(8.0, 8.0),
            );
            let animation = assets
                .animations
                .iter()
                .find(|a| a.id == pickup.animation())
                .unwrap();
            let mut active = true;
            for row in case.frames {
                let tick = row[0] as u64;
                if tick > 0 && active {
                    active = pickup.step(tick, &mut body);
                }
                let actual = [
                    if active { i32::from(case.kind) } else { 0 },
                    (body.center().x * 256.0) as i32,
                    (body.center().y * 256.0) as i32,
                    i32::from(pickup.remaining),
                    i32::from(animation.frame_at(pickup.animation_ticks).sprite),
                    if tick > 0 && active && pickup.visible {
                        4
                    } else {
                        0
                    },
                ];
                assert_eq!(
                    actual,
                    row[1..],
                    "kind {} slot {} tick {}",
                    case.kind,
                    case.slot,
                    tick
                );
            }
        }
    }
    fn context(players: u8) -> DropContext {
        DropContext {
            active_players: players,
            screen_y: 80,
            random: 255,
        }
    }
    #[test]
    fn ordinary_blocks_drop_every_fifth_and_on_matching_decimal_digits() {
        let mut rewards = Rewards::default();
        let observed: Vec<_> = (0..12)
            .filter_map(|index| rewards.break_block(context(1)).map(|drop| (index, drop)))
            .collect();
        assert_eq!(
            observed,
            vec![
                (0, PickupKind::SmallGem),
                (5, PickupKind::SmallGem),
                (10, PickupKind::SmallGem),
                (11, PickupKind::MediumGem)
            ]
        );
    }
    #[test]
    fn chest_rewards_follow_player_count_table_and_keep_large_gems() {
        assert_eq!(Rewards::default().open_chest(context(4)), PickupKind::Fairy);
        assert_eq!(
            Rewards::default().open_chest(context(2)),
            PickupKind::Feather
        );
        assert_eq!(Rewards::new(1).open_chest(context(1)), PickupKind::LargeGem);
        assert_eq!(
            Rewards::new(11).break_block(context(1)),
            Some(PickupKind::MediumGem)
        );
    }
    #[test]
    fn multiplayer_low_screen_rewards_can_be_swap_items_but_solo_cannot() {
        let near_bottom = DropContext {
            active_players: 2,
            screen_y: 200,
            random: 0,
        };
        assert_eq!(Rewards::default().open_chest(near_bottom), PickupKind::Swap);
        assert_eq!(
            Rewards::default().open_chest(DropContext {
                active_players: 1,
                ..near_bottom
            }),
            PickupKind::Feather
        );
        assert_eq!(Rewards::new(8).open_chest(context(2)), PickupKind::Swap);
    }
    #[test]
    fn decimal_counter_wraps_and_chests_advance_same_sequence() {
        let mut rewards = Rewards::new(99);
        rewards.open_chest(context(1));
        assert_eq!(rewards.broken_blocks(), 0);
        assert_eq!(rewards.break_block(context(1)), Some(PickupKind::SmallGem));
        assert_eq!(rewards.broken_blocks(), 1);
    }
}
