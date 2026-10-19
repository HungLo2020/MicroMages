//! Spells and enemy attacks share lifetime and terrain collision, not ownership.
use crate::{
    enemy::{EnemyAttack, ProjectileKind},
    physics::{Body, CollisionMap, Vec2},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Player(usize),
    Enemy,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    Spell,
    Freeze,
    Bubble,
    Spear,
    Orb,
    Spark,
    FallingHazard,
}
#[derive(Clone, Debug)]
pub struct Projectile {
    pub body: Body,
    pub appearance: Appearance,
    pub owner: Owner,
    pub charged: bool,
    pub freezes: bool,
    pub damage: u16,
    pub ticks_left: u16,
    rise_acceleration: f32,
    bounces: u8,
}
impl Projectile {
    pub fn spell(player: usize, position: Vec2, direction: Vec2, charged: bool) -> Self {
        let speed = 3.0;
        let size = Vec2::new(if charged { 8.0 } else { 4.0 }, 4.0);
        let mut body = Body::new(
            Vec2::new(position.x - size.x * 0.5, position.y - size.y * 0.5),
            size,
        );
        body.velocity = Vec2::new(direction.x * speed, direction.y * speed);
        Self {
            body,
            owner: Owner::Player(player),
            appearance: Appearance::Spell,
            charged,
            freezes: false,
            damage: if charged { 6 } else { 2 },
            ticks_left: if charged { 41 } else { 37 },
            rise_acceleration: 0.0,
            bounces: 0,
        }
    }
    pub fn freeze(player: usize, position: Vec2, direction: Vec2) -> Self {
        let mut shot = Self::spell(player, position, direction, false);
        shot.appearance = Appearance::Freeze;
        shot.freezes = true;
        shot.damage = 0;
        shot.ticks_left = 20;
        shot
    }
    /// Freezing shots have no actor damage but apply ordinary block damage.
    pub fn terrain_damage(&self) -> u8 {
        if self.charged { 6 } else { 2 }
    }

    pub fn downgrade(&mut self) {
        self.charged = false;
        self.damage = 2;
        let center = self.body.center();
        self.body.size = Vec2::new(4.0, 4.0);
        self.body.position = Vec2::new(center.x - 2.0, center.y - 2.0);
    }
    pub fn hostile(attack: EnemyAttack) -> Self {
        Self::hostile_with_difficulty(attack, false)
    }
    pub fn hostile_with_difficulty(attack: EnemyAttack, hard: bool) -> Self {
        let mut body = Body::new(
            Vec2::new(attack.position.x - 2.0, attack.position.y - 2.0),
            Vec2::new(4.0, 4.0),
        );
        body.velocity = attack.velocity;
        if attack.kind == ProjectileKind::Bubble {
            let speed = if hard { 186.0 / 256.0 } else { 152.0 / 256.0 };
            body.velocity = Vec2::new(
                if attack.velocity.x < 0.0 {
                    -speed
                } else {
                    speed
                },
                speed,
            );
        }
        if attack.kind == ProjectileKind::Spear {
            body.velocity = Vec2::new(if attack.velocity.x < 0.0 { -1.0 } else { 1.0 }, -3.0);
        }
        Self {
            body,
            owner: Owner::Enemy,
            appearance: match attack.kind {
                ProjectileKind::Bubble => Appearance::Bubble,
                ProjectileKind::Spear => Appearance::Spear,
                ProjectileKind::Orb => Appearance::Orb,
                ProjectileKind::Spark => Appearance::Spark,
                ProjectileKind::FallingHazard => Appearance::FallingHazard,
                _ => Appearance::Spell,
            },
            charged: false,
            freezes: false,
            damage: 1,
            ticks_left: u16::MAX,
            bounces: 0,
            rise_acceleration: if hard { 3.0 / 256.0 } else { 2.0 / 256.0 },
        }
    }
    /// Hostile source callbacks integrate directly and never collide with terrain.
    pub fn advance_motion(&mut self) {
        match self.appearance {
            Appearance::Bubble => {
                self.body.velocity.y -= self.rise_acceleration;
                if self.body.velocity.y.floor() == -5.0 {
                    self.body.velocity.y = -1025.0 / 256.0;
                }
            }
            Appearance::Spear => {
                self.body.velocity.y += 37.0 / 256.0;
                // Original applies this fraction adjustment after integrating.
                self.body.position.x += self.body.velocity.x;
                self.body.position.y += self.body.velocity.y;
                if self.body.velocity.y.floor() == 3.0 {
                    self.body.velocity.y = 771.0 / 256.0;
                }
                return;
            }
            _ => {}
        }
        self.body.position.x += self.body.velocity.x;
        self.body.position.y += self.body.velocity.y;
    }
    /// Bomb fragments sample the tile at their center after each axis, reverse
    /// into their previous position on impact, and expire on the fourth contact.
    pub fn advance_bouncing(&mut self, map: &impl CollisionMap) {
        self.body.position.x += self.body.velocity.x;
        let center = self.body.center();
        if map
            .material(
                (center.x.floor() as i32).div_euclid(8),
                (center.y.floor() as i32).div_euclid(8),
            )
            .0
            & 1
            != 0
        {
            self.body.velocity.x = -self.body.velocity.x;
            self.body.position.x += self.body.velocity.x;
            self.bounces += 1;
        }
        self.body.position.y += self.body.velocity.y;
        let center = self.body.center();
        let flags = map
            .material(
                (center.x.floor() as i32).div_euclid(8),
                (center.y.floor() as i32).div_euclid(8),
            )
            .0;
        if flags & 1 != 0 || (flags & 4 != 0 && self.body.velocity.y >= 0.0) {
            self.body.velocity.y = -self.body.velocity.y;
            self.body.position.y += self.body.velocity.y;
            self.bounces += 1;
        }
        if self.bounces >= 4 {
            self.ticks_left = 0;
        }
    }
    pub fn age(&mut self) {
        if self.ticks_left != u16::MAX {
            self.ticks_left = self.ticks_left.saturating_sub(1);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bomb_fragment_matches_original_four_bounce_trace() {
        struct Room;
        impl CollisionMap for Room {
            fn width(&self) -> usize {
                32
            }
            fn height(&self) -> usize {
                32
            }
            fn material(&self, x: i32, y: i32) -> crate::physics::Material {
                crate::physics::Material(if (12..14).contains(&x) && (12..14).contains(&y) {
                    0
                } else {
                    1
                })
            }
        }
        let mut shot = Projectile::hostile(EnemyAttack {
            kind: ProjectileKind::Spark,
            position: Vec2::new(104., 104.),
            velocity: Vec2::new(0.5, 0.5),
        });
        for row in include_str!("../tests/fixtures/bomb-fragment.csv")
            .lines()
            .skip(1)
        {
            let f: Vec<i32> = row.split(',').map(|n| n.parse().unwrap()).collect();
            shot.advance_bouncing(&Room);
            assert_eq!(
                [
                    (shot.body.center().x * 256.) as i32,
                    (shot.body.center().y * 256.) as i32,
                    (shot.body.velocity.x * 256.) as i32,
                    (shot.body.velocity.y * 256.) as i32,
                    i32::from(shot.bounces),
                    i32::from(shot.ticks_left > 0)
                ],
                f[1..],
                "source tick {}",
                f[0]
            );
        }
    }
    #[test]
    fn charged_spell_is_stronger_and_projectiles_expire() {
        let ordinary = Projectile::spell(2, Vec2::default(), Vec2::new(0.0, -1.0), false);
        let mut charged = Projectile::spell(2, Vec2::default(), Vec2::new(0.0, -1.0), true);
        assert_eq!(ordinary.owner, Owner::Player(2));
        assert!(charged.damage > ordinary.damage);
        assert_eq!(charged.body.velocity.y, ordinary.body.velocity.y);
        for _ in 0..200 {
            charged.age();
        }
        assert_eq!(charged.ticks_left, 0);
    }
    #[test]
    fn hostile_arcs_match_eighty_ticks_of_original_callbacks() {
        for case in ["bubble", "hard_bubble", "spear", "orb"] {
            let kind = match case {
                "bubble" | "hard_bubble" => ProjectileKind::Bubble,
                "spear" => ProjectileKind::Spear,
                _ => ProjectileKind::Orb,
            };
            let mut shot = Projectile::hostile_with_difficulty(
                EnemyAttack {
                    kind,
                    position: Vec2::new(128., 120.),
                    velocity: Vec2::new(0.75, -0.5),
                },
                case == "hard_bubble",
            );
            for row in include_str!("../tests/fixtures/projectile-motion.csv")
                .lines()
                .skip(1)
            {
                let f: Vec<_> = row.split(',').collect();
                if f[0] != case {
                    continue;
                }
                shot.advance_motion();
                let actual = [
                    shot.body.center().x * 256.,
                    shot.body.center().y * 256.,
                    shot.body.velocity.x * 256.,
                    shot.body.velocity.y * 256.,
                ];
                let expected = [
                    f[2].parse::<f32>().unwrap(),
                    f[3].parse().unwrap(),
                    f[4].parse().unwrap(),
                    f[5].parse().unwrap(),
                ];
                assert_eq!(actual, expected, "{case} tick {}", f[1]);
            }
        }
    }
    #[test]
    fn freeze_lifetime_and_downgrade_keep_source_contract() {
        let freeze = Projectile::freeze(1, Vec2::new(20., 30.), Vec2::new(1., 0.));
        assert_eq!(freeze.ticks_left, 20);
        assert_eq!(freeze.damage, 0);
        assert_eq!(freeze.terrain_damage(), 2);
        assert!(freeze.freezes);
        let mut charged = Projectile::spell(0, Vec2::new(20., 30.), Vec2::new(1., 0.), true);
        charged.age();
        let lifetime = charged.ticks_left;
        charged.downgrade();
        assert_eq!(charged.body.center(), Vec2::new(20., 30.));
        assert_eq!(charged.ticks_left, lifetime);
        assert_eq!(charged.damage, 2);
        assert!(!charged.charged);
    }
}
