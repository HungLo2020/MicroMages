//! Deterministic collision at a fixed 60 Hz. Positions are world pixels.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}
impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn distance_squared(self, other: Self) -> f32 {
        (self.x - other.x).powi(2) + (self.y - other.y).powi(2)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub position: Vec2,
    pub velocity: Vec2,
    pub size: Vec2,
    pub grounded: bool,
}
impl Body {
    pub fn new(position: Vec2, size: Vec2) -> Self {
        Self {
            position,
            size,
            velocity: Vec2::default(),
            grounded: false,
        }
    }
    pub fn center(&self) -> Vec2 {
        Vec2::new(
            self.position.x + self.size.x * 0.5,
            self.position.y + self.size.y * 0.5,
        )
    }
    pub fn overlaps(&self, other: &Self) -> bool {
        self.position.x < other.position.x + other.size.x
            && self.position.x + self.size.x > other.position.x
            && self.position.y < other.position.y + other.size.y
            && self.position.y + self.size.y > other.position.y
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Contact {
    /// -1 for a wall on the left, 1 for a wall on the right.
    pub wall: i8,
    pub wall_jump: i8,
    pub ceiling: bool,
    pub landed: bool,
    pub hazard: bool,
    pub ladder: bool,
    pub slippery: bool,
    pub jump_support: bool,
}

/// Material semantics recovered from the original collision table.
#[derive(Clone, Copy, Debug, Default)]
pub struct Material(pub u8);
impl Material {
    pub fn solid(self) -> bool {
        self.0 & 1 != 0
    }
    pub fn platform(self) -> bool {
        self.0 & 4 != 0
    }
    pub fn hazard(self) -> bool {
        self.0 & 64 != 0
    }
    pub fn ladder(self) -> bool {
        self.0 & 2 != 0
    }
    pub fn breakable(self) -> bool {
        self.0 & 16 != 0
    }
}

pub trait CollisionMap {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
    fn material(&self, column: i32, row: i32) -> Material;
}

/// Axis-separated, subpixel movement. Steps never cross an entire tile, so fast
/// projectiles and wall jumps cannot tunnel through thin walls.
pub fn move_body(body: &mut Body, map: &impl CollisionMap, drop_through: bool) -> Contact {
    let mut contact = Contact::default();
    let velocity = body.velocity;
    let steps = (velocity.x.abs().max(velocity.y.abs()) / 4.0)
        .ceil()
        .max(1.0) as usize;
    let dx = velocity.x / steps as f32;
    let dy = velocity.y / steps as f32;
    body.grounded = false;
    for _ in 0..steps {
        if contact.wall == 0 {
            body.position.x += dx;
            for (col, row) in covered_tiles(body) {
                if map.material(col, row).solid() {
                    if dx > 0.0 {
                        body.position.x = col as f32 * 8.0 - body.size.x;
                        contact.wall = 1;
                    } else if dx < 0.0 {
                        body.position.x = (col + 1) as f32 * 8.0;
                        contact.wall = -1;
                    }
                    body.velocity.x = 0.0;
                }
            }
        }
        let previous_bottom = body.position.y + body.size.y;
        if !contact.ceiling && !contact.landed {
            body.position.y += dy;
            for (col, row) in covered_tiles(body) {
                let material = map.material(col, row);
                let top = row as f32 * 8.0;
                let platform = material.platform()
                    && !drop_through
                    && dy >= 0.0
                    && previous_bottom <= top + 0.001;
                if material.solid() || platform {
                    if dy >= 0.0 && previous_bottom <= top + 0.01 {
                        body.position.y = top - body.size.y;
                        body.velocity.y = 0.0;
                        body.grounded = true;
                        contact.landed = true;
                    } else if dy < 0.0 {
                        body.position.y = (row + 1) as f32 * 8.0;
                        body.velocity.y = 0.0;
                        contact.ceiling = true;
                    }
                }
            }
        }
    }
    // A stationary body retains floor contact, without being pushed through it.
    if velocity.y >= 0.0 && !drop_through {
        let row = ((body.position.y + body.size.y + 0.01) / 8.0).floor() as i32;
        let aligned = (body.position.y + body.size.y - row as f32 * 8.0).abs() < 0.02;
        if aligned {
            for col in (body.position.x / 8.0).floor() as i32
                ..=((body.position.x + body.size.x - 0.01) / 8.0).floor() as i32
            {
                let material = map.material(col, row);
                body.grounded |= material.solid() || material.platform();
            }
        }
    }
    for (col, row) in covered_tiles(body) {
        let material = map.material(col, row);
        contact.hazard |= material.hazard();
        contact.ladder |= material.ladder();
    }
    contact
}

/// Diagonal flyers integrate before probing four pixels in each travel
/// direction. A solid surface reflects that component without removing speed.
/// Source a4bb / a50a; one-way platforms do not reflect these flying actors.
pub fn move_diagonal_flyer(body: &mut Body, map: &impl CollisionMap) {
    body.position.x += body.velocity.x;
    body.position.y += body.velocity.y;
    let center = body.center();
    let solid = |dx: f32, dy: f32| {
        map.material(
            ((center.x.floor() + dx) / 8.).floor() as i32,
            ((center.y.floor() + dy) / 8.).floor() as i32,
        )
        .solid()
    };
    if body.velocity.x != 0. && solid(if body.velocity.x < 0. { -4. } else { 4. }, 0.) {
        body.velocity.x = -body.velocity.x;
    }
    if body.velocity.y != 0. && solid(0., if body.velocity.y < 0. { -4. } else { 4. }) {
        body.velocity.y = -body.velocity.y;
    }
}

/// Original player collision samples use integer pixel centers. Fractional
/// position survives contact; unlike a full rectangle, the feet are inset.
pub fn player_environment(body: &Body, map: &impl CollisionMap) -> Contact {
    let center = body.center();
    let x = center.x.floor();
    let y = center.y.floor();
    let sample = |dx: f32, dy: f32| {
        map.material(
            ((x + dx) / 8.0).floor() as i32,
            ((y + dy) / 8.0).floor() as i32,
        )
    };
    let left = sample(-4.0, -2.0).solid() || sample(-4.0, 3.0).solid();
    let right = sample(4.0, -2.0).solid() || sample(4.0, 3.0).solid();
    let jump_left = sample(-7.0, -3.0).solid() || sample(-7.0, 3.0).solid();
    let jump_right = sample(7.0, -3.0).solid() || sample(7.0, 3.0).solid();
    Contact {
        wall: if left {
            -1
        } else if right {
            1
        } else {
            0
        },
        wall_jump: if jump_left {
            -1
        } else if jump_right {
            1
        } else {
            0
        },
        jump_support: [sample(-3.0, 6.0), sample(3.0, 6.0)]
            .iter()
            .any(|m| m.solid() || m.platform()),
        ladder: sample(0.0, -2.0).ladder(),
        hazard: sample(0.0, -2.0).hazard(),
        slippery: body.grounded
            && (sample(-4.0, 5.0).0 & 128 != 0 || sample(4.0, 5.0).0 & 128 != 0),
        ..Contact::default()
    }
}

pub fn move_player(
    body: &mut Body,
    map: &impl CollisionMap,
    _drop_through: bool,
    floating: bool,
) -> Contact {
    move_player_with_vertical_collision(body, map, floating, true)
}

/// The segmented-boss lift retains horizontal walls but disables vertical terrain.
pub fn move_player_during_ascent(body: &mut Body, map: &impl CollisionMap) -> Contact {
    move_player_with_vertical_collision(body, map, false, false)
}

fn move_player_with_vertical_collision(
    body: &mut Body,
    map: &impl CollisionMap,
    floating: bool,
    vertical_collision: bool,
) -> Contact {
    let mut contact = Contact::default();
    body.position.x += body.velocity.x;
    let mut center = body.center();
    let x = center.x.floor();
    let y = center.y.floor();
    let sample = |x: f32, y: f32| map.material((x / 8.0).floor() as i32, (y / 8.0).floor() as i32);
    let edge = if body.velocity.x < 0.0 { -4.0 } else { 4.0 };
    if body.velocity.x != 0.0
        && (sample(x + edge, y - 2.0).solid() || sample(x + edge, y + 3.0).solid())
    {
        let snapped = if body.velocity.x > 0.0 {
            (x / 8.0).floor() * 8.0 + 4.0
        } else {
            ((x + 8.0) / 8.0).floor() * 8.0 - 5.0
        };
        contact.wall = if body.velocity.x > 0.0 { 1 } else { -1 };
        body.position.x = snapped - body.size.x * 0.5 + center.x.fract();
        body.velocity.x = 0.0;
    }
    body.position.y += body.velocity.y;
    center = body.center();
    let x = center.x.floor();
    let y = center.y.floor();
    body.grounded = false;
    if vertical_collision && body.velocity.y > 0.0 {
        let blocks = |m: Material| m.solid() || (m.platform() && y.rem_euclid(8.0) >= 3.0);
        if blocks(sample(x - 3.0, y + 4.0)) || blocks(sample(x + 3.0, y + 4.0)) {
            let snapped = ((y - 4.0) / 8.0).floor() * 8.0 + 4.0;
            body.position.y = snapped - body.size.y * 0.5 + center.y.fract();
            body.velocity.y = 0.0;
            body.grounded = true;
            contact.landed = true;
        }
    } else if vertical_collision
        && body.velocity.y < 0.0
        && (sample(x - 3.0, y - 3.0).solid() || sample(x + 3.0, y - 3.0).solid())
    {
        let snapped = (y / 8.0).floor() * 8.0 + 3.0;
        body.position.y = snapped - body.size.y * 0.5 + center.y.fract();
        body.velocity.y = if floating {
            -18.0 / 256.0
        } else {
            -128.0 / 256.0
        };
        contact.ceiling = true;
    }
    let environment = player_environment(body, map);
    contact.ladder = environment.ladder;
    contact.hazard = environment.hazard;
    contact.slippery = environment.slippery;
    contact.jump_support = environment.jump_support;
    contact.wall_jump = environment.wall_jump;
    contact
}

/// Summoned walkers use the source character-sized inset terrain probes, with
/// gravity applied on every tick (including the tick after landing).
pub fn move_summoned_walker(body: &mut Body, map: &impl CollisionMap) -> Contact {
    body.velocity.y += 37.0 / 256.0;
    move_player(body, map, false, false)
}

fn covered_tiles(body: &Body) -> Vec<(i32, i32)> {
    let left = (body.position.x / 8.0).floor() as i32;
    let right = ((body.position.x + body.size.x - 0.001) / 8.0).floor() as i32;
    let top = (body.position.y / 8.0).floor() as i32;
    let bottom = ((body.position.y + body.size.y - 0.001) / 8.0).floor() as i32;
    (top..=bottom)
        .flat_map(|row| (left..=right).map(move |col| (col, row)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagonal_flyer_reflects_at_solid_surfaces_and_crosses_platforms() {
        struct Surface(u8);
        impl CollisionMap for Surface {
            fn width(&self) -> usize {
                32
            }
            fn height(&self) -> usize {
                32
            }
            fn material(&self, x: i32, y: i32) -> Material {
                Material(if x >= 8 || y >= 8 { self.0 } else { 0 })
            }
        }
        for material in [1, 4] {
            let mut body = Body::new(Vec2::new(56., 56.), Vec2::new(8., 8.));
            body.velocity = Vec2::new(0.25, 0.25);
            move_diagonal_flyer(&mut body, &Surface(material));
            assert_eq!(body.position, Vec2::new(56.25, 56.25));
            let speed = if material == 1 { -0.25 } else { 0.25 };
            assert_eq!(body.velocity, Vec2::new(speed, speed));
        }
    }

    #[test]
    fn summoned_walkers_match_full_source_lifetime_in_both_modes() {
        use crate::enemy::{Enemy, EnemyContext, EnemyKind, EnemyRng};
        struct Corridor;
        impl CollisionMap for Corridor {
            fn width(&self) -> usize {
                32
            }
            fn height(&self) -> usize {
                32
            }
            fn material(&self, x: i32, y: i32) -> Material {
                Material(
                    if y.div_euclid(2) == 5 || [4, 7].contains(&x.div_euclid(2)) {
                        1
                    } else {
                        0
                    },
                )
            }
        }
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/turret_minion_reference.json"
        ))
        .unwrap();
        for case in source["cases"].as_array().unwrap() {
            let hard = case["hard"].as_bool().unwrap();
            let mut enemy = Enemy::new(EnemyKind::SummonedWalker, Vec2::new(84., 60.), hard);
            enemy.facing = 1.;
            assert_eq!(enemy.health, if hard { 7 } else { 6 });
            let mut random = EnemyRng::new(1);
            for row in case["rows"].as_array().unwrap() {
                let tick = row["tick"].as_u64().unwrap();
                enemy.tick_with_context(
                    &EnemyContext::single(Some(Vec2::new(128., 64.)), tick),
                    &mut random,
                );
                if !enemy.motion_paused() && enemy.health > 0 {
                    let contact = move_summoned_walker(&mut enemy.body, &Corridor);
                    enemy.set_wall_contact(contact.wall);
                }
                for (key, value) in [
                    ("x256", enemy.body.center().x),
                    ("y256", enemy.body.center().y),
                    ("vx256", enemy.body.velocity.x),
                    ("vy256", enemy.body.velocity.y),
                ] {
                    assert_eq!(
                        (value * 256.) as i64,
                        row[key].as_i64().unwrap(),
                        "hard={hard} tick={tick} {key}"
                    );
                }
                assert_eq!(
                    enemy.health > 0,
                    row["alive"].as_bool().unwrap(),
                    "hard={hard} tick={tick}"
                );
            }
        }
    }
    struct Room {
        floor: u8,
    }
    impl CollisionMap for Room {
        fn width(&self) -> usize {
            32
        }
        fn height(&self) -> usize {
            30
        }
        fn material(&self, col: i32, row: i32) -> Material {
            Material(if col == 4 {
                1
            } else if row == 10 {
                self.floor
            } else {
                0
            })
        }
    }
    #[test]
    fn fast_body_cannot_tunnel_through_wall() {
        let mut body = Body::new(Vec2::new(0.0, 0.0), Vec2::new(8.0, 8.0));
        body.velocity.x = 100.0;
        let hit = move_body(&mut body, &Room { floor: 1 }, false);
        assert_eq!(body.position.x, 24.0);
        assert_eq!(hit.wall, 1);
    }
    #[test]
    fn one_way_platform_allows_upward_motion_and_stops_falling() {
        let map = Room { floor: 4 };
        let mut body = Body::new(Vec2::new(0.0, 90.0), Vec2::new(8.0, 8.0));
        body.velocity.y = -30.0;
        move_body(&mut body, &map, false);
        assert_eq!(body.position.y, 60.0);
        body.velocity.y = 30.0;
        move_body(&mut body, &map, false);
        assert_eq!(body.position.y, 72.0);
        assert!(body.grounded);
        body.velocity.y = 3.0;
        move_body(&mut body, &map, true);
        assert!(body.position.y > 72.0);
    }
    #[test]
    fn solid_floor_and_ceiling_resolve_without_overlap() {
        let map = Room { floor: 1 };
        let mut body = Body::new(Vec2::new(0.0, 60.0), Vec2::new(8.0, 8.0));
        body.velocity.y = 20.0;
        assert!(move_body(&mut body, &map, false).landed);
        assert_eq!(body.position.y, 72.0);
        body.position.y = 90.0;
        body.velocity.y = -20.0;
        assert!(move_body(&mut body, &map, false).ceiling);
        assert_eq!(body.position.y, 88.0);
    }
    #[test]
    fn player_contacts_match_original_subroutine_traces() {
        struct FixtureMap {
            wall: bool,
            platform: bool,
        }
        impl CollisionMap for FixtureMap {
            fn width(&self) -> usize {
                32
            }
            fn height(&self) -> usize {
                32
            }
            fn material(&self, col: i32, row: i32) -> Material {
                Material(if self.wall && col / 2 == 4 {
                    1
                } else if !self.wall && row / 2 == 5 {
                    if self.platform {
                        if row == 10 { 4 } else { 0 }
                    } else {
                        1
                    }
                } else {
                    0
                })
            }
        }
        for (name, x, y, vx, vy, floating) in [
            ("floor", 128., 70., 0., 128., false),
            ("ceiling", 128., 104., 0., -768., false),
            ("float_ceiling", 128., 104., 0., -768., true),
            ("right", 58., 100., 256., 0., false),
            ("left", 86., 100., -256., 0., false),
            ("platform_fall", 128., 70., 0., 128., false),
            ("platform_up", 128., 100., 0., -768., false),
        ] {
            let map = FixtureMap {
                wall: name == "right" || name == "left",
                platform: name.starts_with("platform"),
            };
            let mut body = Body::new(
                Vec2::new(x - 4. + 107. / 256., y - 4. + 193. / 256.),
                Vec2::new(8., 8.),
            );
            body.velocity = Vec2::new(vx / 256., vy / 256.);
            for row in include_str!("../tests/fixtures/player-collision.csv")
                .lines()
                .skip(1)
            {
                let f: Vec<_> = row.split(',').collect();
                if f[0] != name {
                    continue;
                }
                body.velocity.y += 37. / 256.;
                move_player(&mut body, &map, false, floating);
                let actual = [
                    body.center().x * 256.,
                    body.center().y * 256.,
                    body.velocity.x * 256.,
                    body.velocity.y * 256.,
                ];
                let expected = [
                    f[2].parse::<f32>().unwrap(),
                    f[3].parse().unwrap(),
                    f[4].parse().unwrap(),
                    f[5].parse().unwrap(),
                ];
                assert_eq!(actual, expected, "{name}, tick {}", f[1]);
            }
        }
    }
}
