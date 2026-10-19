//! Player rules in pixels per fixed 60 Hz tick; independent of drawing and audio.
//! Recovered constants and deliberate reconstruction choices: docs/player-behavior.md.
use crate::input::InputFrame;
use crate::physics::{Body, Contact, Vec2};

pub const ACCELERATION: f32 = 44.0 / 256.0;
pub const DRAG: f32 = 31.0 / 256.0;
pub const GRAVITY: f32 = 37.0 / 256.0;
pub const MAX_RUN_SPEED: f32 = 336.0 / 256.0;
pub const JUMP_SPEED: f32 = -364.0 / 256.0;
pub const CHARGE_TICKS: u16 = 55;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Alive,
    Dying,
    Ghost,
    Dead,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayerEvent {
    Jumped,
    Shot { direction: Vec2, charged: bool },
    GhostShot { direction: Vec2 },
    Hurt,
    Died,
    Revived,
}

#[derive(Clone, Copy, Debug)]
enum Warp {
    Portal { target: Vec2, ticks: u8 },
    Swap { target: Vec2, ticks: u8 },
    Seeking { target: Vec2 },
}

#[derive(Clone)]
pub struct Player {
    pub id: usize,
    pub body: Body,
    pub facing: i8,
    pub state: PlayerState,
    pub health: u8,
    pub feather: bool,
    /// The extra final arena uses living, freely flying players (source state 2).
    pub flight_mode: bool,
    pub slippery: bool,
    pub climbing: bool,
    death_ticks: u16,
    input_lock: u8,
    warp: Option<Warp>,
    exit_target: Option<f32>,
    exited: bool,
    /// Remaining invulnerability ticks.
    pub invulnerability: u16,
    /// Named sprite frame in the extracted animation catalog.
    pub animation_frame: u16,
    jump_ticks: u8,
    float_timer: u8,
    wall_lock: u8,
    fire_ticks: u16,
    fire_animation: u8,
    fast_fall_ticks: u8,
    animation_ticks: u32,
    visual_animation: &'static str,
    visual_tick: u64,
}

impl Player {
    pub fn new(id: usize, position: Vec2) -> Self {
        Self {
            id,
            body: Body::new(position, Vec2::new(8.0, 8.0)),
            facing: 1,
            state: PlayerState::Alive,
            health: 1,
            feather: false,
            flight_mode: false,
            slippery: false,
            climbing: false,
            death_ticks: 0,
            input_lock: 0,
            warp: None,
            exit_target: None,
            exited: false,
            invulnerability: 0,
            animation_frame: 1,
            jump_ticks: 0,
            float_timer: 0,
            wall_lock: 0,
            fire_ticks: 0,
            fire_animation: 0,
            fast_fall_ticks: 0,
            animation_ticks: 0,
            visual_animation: "player_idle",
            visual_tick: 0,
        }
    }

    /// Source state 3 walks into the doorway at half a pixel per tick.
    pub fn begin_exit(&mut self, target_x: f32) {
        self.exit_target = Some(target_x);
        self.exited = false;
        self.facing = 1;
        self.invulnerability = 0;
        self.visual_animation = "player_run";
        self.visual_tick = 0;
    }
    pub fn update_exit(&mut self, map: &impl crate::physics::CollisionMap) {
        let Some(target) = self.exit_target else {
            return;
        };
        if self.exited {
            return;
        }
        self.body.velocity.x = 0.5;
        self.body.velocity.y = (self.body.velocity.y + GRAVITY).min(3.0);
        crate::physics::move_player(&mut self.body, map, false, false);
        self.visual_tick += 1;
        self.animation_frame = 2 + ((self.visual_tick / 4) % 4) as u16;
        self.exited = self.body.center().x.floor() > target;
    }
    pub fn is_exiting(&self) -> bool {
        self.exit_target.is_some()
    }
    pub fn has_exited(&self) -> bool {
        self.exited
    }
    pub fn behind_exit(&self) -> bool {
        self.exit_target
            .is_some_and(|target| target - self.body.center().x.floor() < 8.0)
    }

    /// Update intent and velocity once; the world then resolves movement/collisions.
    pub fn update(&mut self, input: InputFrame, touching_wall: i8) -> Vec<PlayerEvent> {
        self.update_with_environment(
            input,
            Contact {
                wall: touching_wall,
                wall_jump: touching_wall,
                jump_support: self.body.grounded,
                slippery: self.slippery,
                ..Contact::default()
            },
        )
    }

    /// Source viewport guards apply to the integer center and retain subpixels.
    pub fn prepare_viewport(&mut self, camera_y: f32) {
        if self.state == PlayerState::Ghost
            && self.body.center().y.floor() - camera_y.floor() >= 232.0
        {
            self.body.velocity.y = -772.0 / 256.0;
        }
    }
    pub fn clamp_to_viewport(&mut self, camera_y: f32) {
        let center = self.body.center();
        let x = center.x.floor();
        let screen_y = center.y.floor() - camera_y.floor();
        if x < 5.0 {
            self.body.position.x = 4.0 - self.body.size.x * 0.5 + center.x.rem_euclid(1.0);
        } else if x >= 252.0 {
            self.body.position.x = 252.0 - self.body.size.x * 0.5 + center.x.rem_euclid(1.0);
        }
        if screen_y < 7.0 {
            self.body.position.y =
                camera_y.floor() + 6.0 - self.body.size.y * 0.5 + center.y.rem_euclid(1.0);
            self.body.velocity.y = 0.0;
        } else if screen_y >= 240.0 {
            self.body.position.y =
                camera_y.floor() + 240.0 - self.body.size.y * 0.5 + center.y.rem_euclid(1.0);
            self.body.velocity.y = 0.0;
        }
    }
    pub fn lock_controls(&mut self, ticks: u8) {
        self.input_lock = ticks;
    }

    pub fn is_warping(&self) -> bool {
        self.warp.is_some()
    }
    pub fn begin_portal(&mut self, target: Vec2) {
        if self.state != PlayerState::Alive || self.is_warping() {
            return;
        }
        self.warp = Some(Warp::Portal { target, ticks: 16 });
        self.body.velocity.y = 0.5;
        self.fire_ticks = 0;
        self.body.grounded = false;
    }
    pub fn begin_seek(&mut self, target: Vec2) {
        if self.state != PlayerState::Alive {
            return;
        }
        self.warp = Some(Warp::Seeking { target });
        self.body.grounded = false;
        self.climbing = false;
        self.fire_ticks = 0;
    }
    pub fn begin_swap(&mut self, target: Vec2) {
        if self.state != PlayerState::Alive {
            return;
        }
        self.warp = Some(Warp::Swap { target, ticks: 36 });
        self.body.grounded = false;
        self.body.velocity = Vec2::new(0.0, -258.0 / 256.0);
        self.fire_ticks = 0;
    }
    pub fn spring_launch(&mut self) {
        self.body.velocity.y = -4.0;
        self.float_timer = 0;
        self.jump_ticks = 0;
        self.body.grounded = false;
        self.climbing = false;
    }
    fn update_warp(&mut self) {
        match self.warp.unwrap() {
            Warp::Portal { target, ticks } if ticks > 1 => {
                self.body.velocity.x = 0.0;
                self.warp = Some(Warp::Portal {
                    target,
                    ticks: ticks - 1,
                });
            }
            Warp::Portal { target, .. } => {
                // The world integrates the final velocity after this update.
                self.body.position = Vec2::new(target.x, target.y + 3.0);
                self.body.velocity = Vec2::new(0.0, -3.0);
                self.warp = None;
            }
            Warp::Swap { target, ticks } => {
                let previous = self.body.velocity;
                self.body.velocity.x = approach_zero(self.body.velocity.x, 8.0 / 256.0);
                self.body.velocity.y = approach_zero(self.body.velocity.y, 8.0 / 256.0);
                self.body.position.x += previous.x - self.body.velocity.x;
                self.body.position.y += previous.y - self.body.velocity.y;
                self.warp = Some(if ticks > 1 {
                    Warp::Swap {
                        target,
                        ticks: ticks - 1,
                    }
                } else {
                    Warp::Seeking { target }
                });
            }
            Warp::Seeking { target } => {
                let center = self.body.center();
                let target_center = Vec2::new(target.x + 4.0, target.y + 4.0);
                let quantized_speed = |delta: i32, seed: u8| {
                    let shift = if delta.abs() < 32 { 3 } else { 4 };
                    let magnitude =
                        ((delta.unsigned_abs() * 256 + u32::from(seed)) >> shift) as i32;
                    if delta < 0 { -magnitude } else { magnitude }
                };
                let dy = target_center.y.floor() as i32 - center.y.floor() as i32;
                let vy = quantized_speed(dy, target_center.y.floor() as i32 as u8);
                let dx = target_center.x.floor() as i32 - center.x.floor() as i32;
                let vx = quantized_speed(dx, vy as u8);
                self.body.velocity = Vec2::new(vx as f32 / 256.0, vy as f32 / 256.0);
                let next = Vec2::new(
                    center.x + self.body.velocity.x,
                    center.y + self.body.velocity.y,
                );
                if (target_center.x.floor() - next.x.floor()).abs()
                    + (target_center.y.floor() - next.y.floor()).abs()
                    < 2.0
                {
                    self.body.position =
                        Vec2::new(target.x + next.x.fract(), target.y + next.y.fract());
                    self.body.velocity = Vec2::default();
                    self.warp = None;
                }
            }
        }
    }

    pub fn flying(&self) -> bool {
        self.flight_mode && self.state == PlayerState::Alive
    }

    pub fn floating(&self) -> bool {
        self.float_timer != 0
    }

    pub fn update_with_environment(
        &mut self,
        input: InputFrame,
        environment: Contact,
    ) -> Vec<PlayerEvent> {
        let events = self.update_rules(input, environment);
        let name = self.animation_name();
        if name != self.visual_animation {
            self.visual_animation = name;
            self.visual_tick = 0;
        } else {
            self.visual_tick = self.visual_tick.wrapping_add(1);
        }
        events
    }
    pub fn animation_pose(&self) -> (&'static str, u64) {
        let name = self.animation_name();
        (
            name,
            if name == self.visual_animation {
                self.visual_tick
            } else {
                0
            },
        )
    }
    pub fn display_palette(&self) -> u8 {
        let own = self.id as u8;
        if self.fire_ticks < CHARGE_TICKS {
            return own;
        }
        let raw = 73 + u32::from(self.fire_ticks);
        let counter = if raw < 256 {
            raw
        } else {
            146 + (raw - 256) % 110
        };
        if counter < 146 {
            (counter & 3) as u8
        } else {
            let color = (counter & 7) as u8;
            if color < 3 { color } else { own }
        }
    }
    fn animation_name(&self) -> &'static str {
        if self.exit_target.is_some() {
            return "player_run";
        }
        if self.state == PlayerState::Dying {
            return "player_death";
        }
        if self.state == PlayerState::Ghost {
            return "player_ghost";
        }
        if self.flying() {
            return "player_ghost_float";
        }
        if self.is_warping() {
            return "player_portal";
        }
        match self.animation_frame {
            16 => "player_shoot_horizontal",
            17 => "player_shoot_up",
            18 => "player_shoot_down",
            8 => "player_ladder_idle",
            9 => "player_ladder_move",
            2..=5 => "player_run",
            6 => {
                if self.float_timer > 0 {
                    "player_float"
                } else {
                    "player_jump"
                }
            }
            7 => {
                if self.float_timer >= 128 {
                    "player_float_spent"
                } else {
                    "player_fall"
                }
            }
            22 => "player_look_down",
            10 => "player_fast_fall",
            11 => "player_wall_slide",
            _ => "player_idle",
        }
    }
    fn update_rules(&mut self, mut input: InputFrame, environment: Contact) -> Vec<PlayerEvent> {
        let touching_wall = environment.wall;
        let jump_wall = environment.wall_jump;
        self.slippery = environment.slippery;

        let mut events = Vec::new();
        self.animation_ticks = self.animation_ticks.wrapping_add(1);
        self.invulnerability = self.invulnerability.saturating_sub(1);
        if self.is_warping() {
            self.update_warp();
            return events;
        }
        if self.state == PlayerState::Dead {
            return events;
        }
        if self.state == PlayerState::Dying {
            self.death_ticks = self.death_ticks.saturating_sub(1);
            if self.death_ticks == 0 {
                if self.flight_mode {
                    self.state = PlayerState::Dead;
                    self.body.velocity = Vec2::default();
                    return events;
                }
                self.state = PlayerState::Ghost;
                self.body.velocity.y = -1.0 / 256.0;
            }
            self.body.velocity.y += GRAVITY;
            if self.body.grounded {
                self.body.velocity.x = approach_zero(self.body.velocity.x, 7.0 / 256.0);
            }
            self.animation_frame = 15;
            return events;
        }
        if self.input_lock > 0 {
            self.input_lock -= 1;
            input = InputFrame::default();
        }
        let horizontal = i8::from(input.held.right) - i8::from(input.held.left);
        if horizontal != 0 {
            self.facing = horizontal;
        }
        if self.state == PlayerState::Ghost || self.flying() {
            if self.flying() {
                self.facing = 1;
            }
            self.body.velocity.x = approach_zero(self.body.velocity.x, 19.0 / 256.0)
                + f32::from(horizontal) * 38.0 / 256.0;
            if horizontal < 0 {
                self.body.velocity.x = self.body.velocity.x.max(-MAX_RUN_SPEED);
            }
            if horizontal > 0 {
                self.body.velocity.x = self.body.velocity.x.min(MAX_RUN_SPEED);
            }
            let vertical = i8::from(input.held.down) - i8::from(input.held.up);
            self.body.velocity.y = approach_zero(self.body.velocity.y, 19.0 / 256.0)
                + f32::from(vertical) * 38.0 / 256.0;
            if vertical < 0 {
                self.body.velocity.y = self.body.velocity.y.max(-1.25);
            }
            if vertical > 0 {
                self.body.velocity.y = self.body.velocity.y.min(1.25);
            }
            if self.flying() {
                if input.pressed.fire {
                    self.fire_ticks = 1;
                    events.push(self.shot(input, false));
                } else if input.held.fire && self.fire_ticks != 0 {
                    self.fire_ticks = self.fire_ticks.saturating_add(1);
                }
                if input.released.fire {
                    if self.fire_ticks >= CHARGE_TICKS {
                        events.push(self.shot(input, true));
                    }
                    self.fire_ticks = 0;
                }
            } else if input.pressed.fire {
                let direction = self.aim(input);
                events.push(PlayerEvent::GhostShot { direction });
            }
            self.animation_frame = 15;
            return events;
        }

        if !environment.ladder {
            self.climbing = false;
        }
        if environment.ladder && input.held.up && !self.climbing {
            self.climbing = true;
            let center = self.body.center().x.floor();
            self.body.position.x =
                (center / 8.0).floor() * 8.0 + 3.0 - 4.0 + self.body.position.x.fract();
        }
        if self.climbing && !input.pressed.jump {
            self.body.velocity = Vec2::new(0.0, 0.0);
            self.float_timer = 0;
            if self.fire_animation == 0 {
                self.body.velocity.y = if input.held.down {
                    0.75
                } else if input.held.up {
                    -0.75
                } else {
                    0.0
                };
                let center = self.body.center().x.floor() as i32;
                if horizontal < 0 && center.rem_euclid(8) != 0 {
                    self.body.velocity.x = -80.0 / 256.0;
                }
                if horizontal > 0 && center.rem_euclid(8) != 7 {
                    self.body.velocity.x = 80.0 / 256.0;
                }
            }
        } else {
            let was_climbing = self.climbing;
            self.climbing = false;
            let acceleration = if self.slippery {
                11.0 / 256.0
            } else {
                ACCELERATION
            };
            let drag = if self.slippery { 3.0 / 256.0 } else { DRAG };
            {
                self.body.velocity.x = (self.body.velocity.x
                    + f32::from(horizontal) * acceleration)
                    .clamp(-MAX_RUN_SPEED, MAX_RUN_SPEED);
            }
            self.wall_lock = self.wall_lock.saturating_sub(1);

            if self.body.grounded {
                self.jump_ticks = 0;
                self.float_timer = 0;
            }
            if input.pressed.jump && (environment.jump_support || jump_wall != 0 || was_climbing) {
                if !environment.jump_support && !was_climbing {
                    self.body.velocity.x = -f32::from(jump_wall) * 2.0;
                    self.facing = -jump_wall;
                    self.wall_lock = 6;
                }
                self.body.velocity.y = JUMP_SPEED;
                self.body.grounded = false;
                self.jump_ticks = 9;
                events.push(PlayerEvent::Jumped);
            } else if input.pressed.jump && self.feather && self.float_timer < 128 {
                self.float_timer = 48;
                self.body.velocity.y = 0.0;
                events.push(PlayerEvent::Jumped);
            } else if input.held.jump && self.float_timer > 0 && self.float_timer < 128 {
                self.body.velocity.y -= 38.0 / 256.0;
                self.float_timer += 1;
                self.jump_ticks = self.jump_ticks.saturating_sub(1);
            } else if input.held.jump && self.jump_ticks != 0 {
                self.body.velocity.y -= 44.0 / 256.0;
                self.jump_ticks -= 1;
            } else {
                self.jump_ticks = self.jump_ticks.saturating_sub(1);
            }
            if was_climbing {
                self.body.velocity.x = 0.0;
            } else {
                self.body.velocity.x = approach_zero(self.body.velocity.x, drag);
            }
            if touching_wall == 0 && self.body.velocity.y >= 2.0 {
                self.body.velocity.y = self.body.velocity.y.floor();
                self.fast_fall_ticks = self.fast_fall_ticks.saturating_add(1);
            } else {
                self.fast_fall_ticks = 0;
            }
            if touching_wall != 0 && self.body.velocity.y > 0.125 {
                self.body.velocity.y = 0.125;
            }
            if !was_climbing {
                self.body.velocity.y += GRAVITY;
            }
        }

        if input.pressed.fire {
            self.fire_ticks = 1;
            events.push(self.shot(input, false));
        } else if input.held.fire && self.fire_ticks != 0 {
            self.fire_ticks = self.fire_ticks.saturating_add(1);
        }
        if input.released.fire {
            if self.fire_ticks >= CHARGE_TICKS {
                events.push(self.shot(input, true));
            }
            self.fire_ticks = 0;
        }
        if self.fire_animation > 0 {
            self.fire_animation -= 1;
        } else {
            self.animation_frame = if self.climbing {
                if self.body.velocity.y == 0.0 { 8 } else { 9 }
            } else if !self.body.grounded {
                if self.float_timer > 0 && self.float_timer < 128 && input.held.jump {
                    6
                } else if self.float_timer >= 128 {
                    7
                } else if self.body.velocity.y < 0.0 {
                    6
                } else if touching_wall != 0 {
                    11
                } else if self.fast_fall_ticks >= 4 {
                    10
                } else {
                    7
                }
            } else if horizontal != 0 {
                2 + ((self.animation_ticks / 4) % 4) as u16
            } else if input.held.up && !input.held.down && !input.held.jump {
                17
            } else if input.held.down && !input.held.up && !input.held.jump {
                22
            } else if input.held.jump {
                16
            } else {
                1
            };
        }
        events
    }

    fn aim(&self, input: InputFrame) -> Vec2 {
        if self.flight_mode {
            return Vec2::new(1., 0.);
        }
        if input.held.up {
            Vec2::new(0.0, -1.0)
        } else if input.held.down {
            Vec2::new(0.0, 1.0)
        } else {
            Vec2::new(f32::from(self.facing), 0.0)
        }
    }

    fn shot(&mut self, input: InputFrame, charged: bool) -> PlayerEvent {
        let direction = self.aim(input);
        self.animation_frame = if direction.y < 0.0 {
            17
        } else if direction.y > 0.0 {
            18
        } else {
            16
        };
        self.fire_animation = 8;
        if charged && direction.y > 0.0 {
            self.body.velocity.y = -544.0 / 256.0;
        }
        PlayerEvent::Shot { direction, charged }
    }

    pub fn damage_from_hazard(&mut self) -> Vec<PlayerEvent> {
        if self.state != PlayerState::Alive
            || self.invulnerability > 0
            || self.is_warping()
            || self.is_exiting()
        {
            return Vec::new();
        }
        self.body.velocity.x = -f32::from(self.facing) * 1.5;
        self.damage()
    }

    pub fn damage(&mut self) -> Vec<PlayerEvent> {
        if self.state != PlayerState::Alive
            || self.invulnerability > 0
            || self.is_warping()
            || self.is_exiting()
        {
            return Vec::new();
        }
        self.invulnerability = 110;
        if self.feather {
            self.feather = false;
            self.float_timer = 0;
            return vec![PlayerEvent::Hurt];
        }
        self.health = self.health.saturating_sub(1);
        if self.health == 0 {
            self.state = PlayerState::Dying;
            self.invulnerability = 0;
            self.death_ticks = 128;
            self.body.velocity.y = -2.0;
            self.feather = false;
            self.climbing = false;
            self.fire_ticks = 0;
            vec![PlayerEvent::Hurt, PlayerEvent::Died]
        } else {
            vec![PlayerEvent::Hurt]
        }
    }

    pub fn revive(&mut self) -> Vec<PlayerEvent> {
        if self.state != PlayerState::Ghost {
            return Vec::new();
        }
        self.state = PlayerState::Alive;
        self.health = 1;
        self.invulnerability = 110;
        self.body.velocity = Vec2::new(0.0, -2.0);
        self.jump_ticks = 0;
        self.float_timer = 0;
        self.death_ticks = 0;
        self.climbing = false;
        vec![PlayerEvent::Revived]
    }
}

/// Resolve the original cooperative body bumps once per unordered pair.
pub fn interact_players(
    players: &mut [Player],
    inputs: [InputFrame; 4],
) -> Vec<(usize, PlayerEvent)> {
    let mut events = Vec::new();
    for first in 0..players.len() {
        for second in first + 1..players.len() {
            let (head, tail) = players.split_at_mut(second);
            let a = &mut head[first];
            let b = &mut tail[0];
            if a.state != PlayerState::Alive
                || b.state != PlayerState::Alive
                || a.is_warping()
                || b.is_warping()
                || a.is_exiting()
                || b.is_exiting()
            {
                continue;
            }
            let dx = a.body.center().x.floor() - b.body.center().x.floor();
            let dy = a.body.center().y.floor() - b.body.center().y.floor();
            if dx.abs() >= 8.0 || dy.abs() >= 8.0 {
                continue;
            }
            if dy.abs() >= 4.0 {
                let (upper, lower) = if dy < 0.0 { (a, b) } else { (b, a) };
                if upper.body.velocity.y < 0.0 {
                    upper.body.velocity.y = -1.0;
                } else {
                    upper.body.velocity.y = -515.0 / 256.0;
                    if inputs[upper.id].held.jump {
                        if upper.feather {
                            upper.float_timer = 48;
                            upper.body.velocity.y = -259.0 / 256.0;
                        } else {
                            upper.body.velocity.y -= 1.0;
                        }
                    }
                    events.push((upper.id, PlayerEvent::Jumped));
                }
                upper.climbing = false;
                upper.body.grounded = false;
                lower.climbing = false;
                lower.input_lock = if lower.body.grounded { 3 } else { 8 };
                if !lower.body.grounded {
                    lower.body.velocity.y = 1.0;
                }
            } else {
                let (left, right) = if dx < 0.0 { (a, b) } else { (b, a) };
                std::mem::swap(&mut left.body.velocity.x, &mut right.body.velocity.x);
                left.body.velocity.x -= 112.0 / 256.0;
                right.body.velocity.x += 112.0 / 256.0;
                left.fire_animation = 6;
                right.fire_animation = 6;
            }
        }
    }
    events
}

fn approach_zero(value: f32, delta: f32) -> f32 {
    if value > 0.0 {
        (value - delta).max(0.0)
    } else {
        (value + delta).min(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::PlayerInput;
    #[test]
    fn doorway_walk_matches_original_execution() {
        struct Floor;
        impl crate::physics::CollisionMap for Floor {
            fn width(&self) -> usize {
                32
            }
            fn height(&self) -> usize {
                30
            }
            fn material(&self, _column: i32, row: i32) -> crate::physics::Material {
                crate::physics::Material(u8::from(row >= 10))
            }
        }
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/exit_motion_reference.json"))
                .unwrap();
        for case in cases.as_array().unwrap() {
            let mut player = Player::new(
                0,
                Vec2::new(124.0, case["start_y"].as_f64().unwrap() as f32 - 4.0),
            );
            player.begin_exit(139.0);
            for row in case["frames"].as_array().unwrap().iter().skip(1) {
                player.update_exit(&Floor);
                let tick = row[0].as_u64().unwrap();
                assert_eq!(
                    (player.body.center().x * 256.0) as u64,
                    row[2].as_u64().unwrap(),
                    "x tick {tick}"
                );
                assert_eq!(
                    (player.body.center().y * 256.0) as u64,
                    row[3].as_u64().unwrap(),
                    "y tick {tick}"
                );
                assert_eq!(player.has_exited(), row[1] == 0, "visibility tick {tick}");
                assert_eq!(
                    player.behind_exit(),
                    row[4].as_u64().unwrap() & 32 != 0,
                    "priority tick {tick}"
                );
                if !player.has_exited() {
                    assert_eq!(
                        u64::from(player.animation_frame),
                        row[5].as_u64().unwrap(),
                        "pose tick {tick}"
                    );
                }
            }
        }
    }

    fn frame(held: PlayerInput, pressed: PlayerInput, released: PlayerInput) -> InputFrame {
        InputFrame {
            held,
            pressed,
            released,
        }
    }
    fn empty() -> InputFrame {
        frame(
            PlayerInput::default(),
            PlayerInput::default(),
            PlayerInput::default(),
        )
    }
    #[test]
    fn final_arena_flight_matches_original_direction_charge_and_drag_trace() {
        let mut player = Player::new(0, Vec2::new(44., 184.));
        player.flight_mode = true;
        let mut previous_fire = false;
        let mut shots = 0;
        for row in include_str!("../tests/fixtures/player-flight.csv")
            .lines()
            .skip(1)
        {
            let f: Vec<i32> = row.split(',').map(|n| n.parse().unwrap()).collect();
            let bits = f[1];
            let mut input = empty();
            input.held = PlayerInput {
                right: bits & 1 != 0,
                left: bits & 2 != 0,
                down: bits & 4 != 0,
                up: bits & 8 != 0,
                fire: bits & 64 != 0,
                ..PlayerInput::default()
            };
            input.pressed.fire = input.held.fire && !previous_fire;
            input.released.fire = !input.held.fire && previous_fire;
            previous_fire = input.held.fire;
            for event in player.update(input, 0) {
                if let PlayerEvent::Shot { direction, charged } = event {
                    assert_eq!(direction, Vec2::new(1., 0.));
                    assert_eq!(charged, f[0] == 60);
                    shots += 1;
                } else {
                    panic!("unexpected flight event {event:?}");
                }
            }
            player.body.position.x += player.body.velocity.x;
            player.body.position.y += player.body.velocity.y;
            assert_eq!(
                [
                    (player.body.center().x * 256.) as i32,
                    (player.body.center().y * 256.) as i32,
                    (player.body.velocity.x * 256.) as i32,
                    (player.body.velocity.y * 256.) as i32
                ],
                f[2..6],
                "source flight tick {}",
                f[0]
            );
            assert_eq!(shots, f[7]);
            assert_eq!([f[8], f[9]], [1024, 0]);
            assert_eq!(player.animation_pose().0, "player_ghost_float");
        }
        assert!(player.flying());
        player.health = 1;
        player.damage();
        assert!(!player.flying());
        for _ in 0..128 {
            player.update(empty(), 0);
        }
        assert_eq!(player.state, PlayerState::Dead);
    }
    #[test]
    fn sixty_tick_freefall_matches_original_velocity_quantization() {
        let mut player = Player::new(0, Vec2::new(124., 16.));
        for row in include_str!("../tests/fixtures/player-freefall.csv")
            .lines()
            .skip(1)
        {
            let f: Vec<i32> = row.split(',').map(|n| n.parse().unwrap()).collect();
            player.update(empty(), 0);
            player.body.position.x += player.body.velocity.x;
            player.body.position.y += player.body.velocity.y;
            assert_eq!(
                [
                    (player.body.center().x * 256.) as i32,
                    (player.body.center().y * 256.) as i32,
                    (player.body.velocity.x * 256.) as i32,
                    (player.body.velocity.y * 256.) as i32,
                    i32::from(player.fast_fall_ticks)
                ],
                f[1..],
                "source tick {}",
                f[0]
            );
        }
    }
    #[test]
    fn charge_colors_and_named_pose_timing_follow_source_counters() {
        let mut player = Player::new(3, Vec2::new(124., 96.));
        for (held_ticks, palette) in [
            (54, 3),
            (55, 0),
            (56, 1),
            (57, 2),
            (58, 3),
            (73, 2),
            (74, 3),
            (79, 0),
            (183, 2),
        ] {
            player.fire_ticks = held_ticks;
            assert_eq!(player.display_palette(), palette, "held {held_ticks}");
        }
        player.body.grounded = true;
        let mut input = empty();
        input.held.down = true;
        player.update(input, 0);
        assert_eq!(player.animation_pose(), ("player_look_down", 0));
        player.update(input, 0);
        assert_eq!(player.animation_pose(), ("player_look_down", 1));
        input.held.down = false;
        input.held.up = true;
        player.update(input, 0);
        assert_eq!(player.animation_pose(), ("player_shoot_up", 0));
        player.damage();
        assert_eq!(player.animation_pose(), ("player_death", 0));
        assert_eq!(player.invulnerability, 0);
    }
    #[test]
    fn ghost_bottom_rebound_precedes_drag_and_control() {
        let mut player = Player::new(0, Vec2::new(124., 228.));
        player.state = PlayerState::Ghost;
        player.prepare_viewport(0.);
        assert_eq!(player.body.velocity.y, -772. / 256.);
        player.update(empty(), 0);
        assert_eq!(player.body.velocity.y, -753. / 256.);
    }
    #[test]
    fn movement_accelerates_then_stops_without_reversing() {
        let mut player = Player::new(0, Vec2::new(10.0, 10.0));
        let mut input = empty();
        input.held.right = true;
        for _ in 0..120 {
            player.update(input, 0);
        }
        assert!((player.body.velocity.x - (MAX_RUN_SPEED - DRAG)).abs() < 0.0001);
        for _ in 0..30 {
            player.update(empty(), 0);
        }
        assert_eq!(player.body.velocity.x, 0.0);
    }
    #[test]
    fn jumping_requires_support_and_holding_extends_ascent() {
        let mut short = Player::new(0, Vec2::new(0.0, 0.0));
        let mut held = Player::new(0, Vec2::new(0.0, 0.0));
        short.body.grounded = true;
        held.body.grounded = true;
        let mut input = empty();
        input.held.jump = true;
        input.pressed.jump = true;
        assert_eq!(short.update(input, 0), vec![PlayerEvent::Jumped]);
        held.update(input, 0);
        assert!(!short.update(input, 0).contains(&PlayerEvent::Jumped));
        input.pressed.jump = false;
        for _ in 0..8 {
            short.update(empty(), 0);
            held.update(input, 0);
        }
        assert!(held.body.velocity.y < short.body.velocity.y);
    }
    #[test]
    fn wall_jump_moves_away_and_wall_slide_limits_fall() {
        let mut player = Player::new(0, Vec2::new(0.0, 0.0));
        player.body.velocity.y = 3.0;
        player.update(empty(), 1);
        assert_eq!(player.body.velocity.y, 0.125 + GRAVITY);
        let mut input = empty();
        input.pressed.jump = true;
        input.held.jump = true;
        assert_eq!(player.update(input, 1), vec![PlayerEvent::Jumped]);
        assert_eq!(player.body.velocity.x, -2.0 + DRAG);
        assert!(player.body.velocity.y < 0.0);
    }
    #[test]
    fn shot_on_press_then_charge_on_release() {
        let mut player = Player::new(0, Vec2::new(0.0, 0.0));
        let mut input = empty();
        input.held.fire = true;
        input.pressed.fire = true;
        assert!(matches!(
            player.update(input, 0).as_slice(),
            [PlayerEvent::Shot { charged: false, .. }]
        ));
        input.pressed.fire = false;
        for _ in 0..CHARGE_TICKS {
            assert!(player.update(input, 0).is_empty());
        }
        input.held.fire = false;
        input.released.fire = true;
        input.held.up = true;
        assert_eq!(
            player.update(input, 0),
            vec![PlayerEvent::Shot {
                direction: Vec2::new(0.0, -1.0),
                charged: true
            }]
        );
        assert!(player.update(input, 0).is_empty());
    }
    #[test]
    fn damage_transitions_to_ghost_and_revive_protects_player() {
        let mut player = Player::new(0, Vec2::new(0.0, 0.0));
        assert_eq!(player.damage(), vec![PlayerEvent::Hurt, PlayerEvent::Died]);
        assert_eq!(player.state, PlayerState::Dying);
        assert!(player.damage().is_empty());
        for _ in 0..128 {
            player.update(empty(), 0);
        }
        assert_eq!(player.state, PlayerState::Ghost);
        assert_eq!(player.revive(), vec![PlayerEvent::Revived]);
        assert_eq!(player.state, PlayerState::Alive);
        assert!(player.damage().is_empty());
        for _ in 0..120 {
            player.update(empty(), 0);
        }
        assert!(!player.damage().is_empty());
    }
    #[test]
    fn running_matches_original_recorded_velocities_and_positions() {
        let mut player = Player::new(0, Vec2::new(128.0, 184.0));
        for row in include_str!("../tests/fixtures/player-run.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = row.split(',').collect();
            let mut input = empty();
            input.held.right = fields[2] == "128";
            player.body.grounded = true;
            player.update(input, 0);
            player.body.position.x += player.body.velocity.x;
            let expected_velocity: f32 = fields[7].parse::<f32>().unwrap() / 256.0;
            let expected_x =
                fields[3].parse::<f32>().unwrap() + fields[4].parse::<f32>().unwrap() / 256.0;
            assert_eq!(
                player.body.velocity.x, expected_velocity,
                "frame {}",
                fields[1]
            );
            assert_eq!(player.body.position.x, expected_x, "frame {}", fields[1]);
        }
    }
    #[test]
    fn short_and_held_jumps_match_original_recorded_velocity() {
        for hold in [false, true] {
            let mut player = Player::new(0, Vec2::new(163.0, 184.0));
            player.body.grounded = true;
            for row in include_str!("../tests/fixtures/player-jump.csv")
                .lines()
                .skip(1)
            {
                let fields: Vec<_> = row.split(',').collect();
                if (fields[0] == "true") != hold {
                    continue;
                }
                let mut input = empty();
                input.held.jump = fields[2] == "1";
                input.pressed.jump = fields[1] == "1720";
                player.update(input, 0);
                // Stop the short-jump comparison at landing: world collision is tested separately.
                if fields[10].parse::<u8>().unwrap() & 16 != 0 {
                    break;
                }
                let expected: f32 = fields[8].parse::<f32>().unwrap() / 256.0;
                assert_eq!(
                    player.body.velocity.y, expected,
                    "hold={hold}, frame {}",
                    fields[1]
                );
            }
        }
    }
    #[test]
    fn feather_allows_airborne_float_and_absorbs_one_hit() {
        let mut player = Player::new(0, Vec2::default());
        player.feather = true;
        player.body.velocity.y = 2.0;
        let mut input = empty();
        input.held.jump = true;
        input.pressed.jump = true;
        assert_eq!(player.update(input, 0), vec![PlayerEvent::Jumped]);
        assert_eq!(player.body.velocity.y, GRAVITY);
        input.pressed.jump = false;
        player.update(input, 0);
        assert_eq!(player.body.velocity.y, GRAVITY - 1.0 / 256.0);
        assert_eq!(player.damage(), vec![PlayerEvent::Hurt]);
        assert_eq!(player.health, 1);
        assert!(!player.feather);
        assert_eq!(player.state, PlayerState::Alive);
    }
    #[test]
    fn ghost_flight_has_inertia_and_fires_freeze_shot_once_per_press() {
        let mut player = Player::new(0, Vec2::default());
        player.damage();
        for _ in 0..128 {
            player.update(empty(), 0);
        }
        let mut input = empty();
        input.held.right = true;
        input.held.fire = true;
        input.pressed.fire = true;
        assert_eq!(
            player.update(input, 0),
            vec![PlayerEvent::GhostShot {
                direction: Vec2::new(1.0, 0.0)
            }]
        );
        assert_eq!(player.body.velocity.x, 38.0 / 256.0);
        input.pressed.fire = false;
        assert!(player.update(input, 0).is_empty());
        assert_eq!(player.body.velocity.x, 57.0 / 256.0);
        player.update(empty(), 0);
        assert_eq!(player.body.velocity.x, 38.0 / 256.0);
        player.revive();
        assert_eq!(player.body.velocity.y, -2.0);
        assert_eq!(player.invulnerability, 110);
    }
    #[test]
    fn feather_and_ghost_match_recorded_reference_velocities() {
        for ghost in [false, true] {
            let mut player = Player::new(0, Vec2::default());
            player.body.grounded = !ghost;
            player.feather = !ghost;
            if ghost {
                player.state = PlayerState::Ghost;
            }
            for row in include_str!("../tests/fixtures/player-feather-ghost.csv")
                .lines()
                .skip(1)
            {
                let fields: Vec<_> = row.split(',').collect();
                if (fields[0] == "true") != ghost {
                    continue;
                }
                let mut input = empty();
                input.held.right = ghost;
                input.held.jump = !ghost && fields[2] == "1";
                input.pressed.jump = !ghost && (fields[1] == "1650" || fields[1] == "1665");
                player.update(input, 0);
                assert_eq!(
                    player.body.velocity.x,
                    fields[3].parse::<f32>().unwrap() / 256.0,
                    "ghost={ghost}, frame {}",
                    fields[1]
                );
                assert_eq!(
                    player.body.velocity.y,
                    fields[4].parse::<f32>().unwrap() / 256.0,
                    "ghost={ghost}, frame {}",
                    fields[1]
                );
            }
        }
    }
    #[test]
    fn slippery_floor_accelerates_and_brakes_with_recovered_ice_constants() {
        let mut player = Player::new(0, Vec2::default());
        player.slippery = true;
        player.body.grounded = true;
        let mut input = empty();
        input.held.right = true;
        player.update(input, 0);
        assert_eq!(player.body.velocity.x, 8.0 / 256.0);
        for _ in 0..100 {
            player.update(input, 0);
        }
        assert_eq!(player.body.velocity.x, MAX_RUN_SPEED - 3.0 / 256.0);
        let previous = player.body.velocity.x;
        player.update(empty(), 0);
        assert_eq!(player.body.velocity.x, previous - 3.0 / 256.0);
    }
    #[test]
    fn feather_pickup_revives_ghost_with_health_one_and_retains_feather() {
        let mut player = Player::new(0, Vec2::default());
        player.damage();
        for _ in 0..128 {
            player.update(empty(), 0);
        }
        player.feather = true;
        assert_eq!(player.revive(), vec![PlayerEvent::Revived]);
        assert_eq!(player.state, PlayerState::Alive);
        assert_eq!(player.health, 1);
        assert!(player.feather);
        assert_eq!(player.invulnerability, 110);
        assert_eq!(player.body.velocity.y, -2.0);
        assert!(player.revive().is_empty());
    }
    #[test]
    fn ladder_warps_and_death_match_original_subroutine_traces() {
        for case in ["ladder", "portal", "swap", "death"] {
            let mut player = Player::new(0, Vec2::new(127.0, 96.0));
            match case {
                "ladder" => player.climbing = true,
                "portal" => player.begin_portal(Vec2::new(156., 116.)),
                "swap" => player.begin_swap(Vec2::new(156., 116.)),
                _ => {
                    player.damage();
                }
            }
            for row in include_str!("../tests/fixtures/player-transitions.csv")
                .lines()
                .skip(1)
            {
                let f: Vec<_> = row.split(',').collect();
                if f[0] != case {
                    continue;
                }
                let input_bits: u8 = f[2].parse().unwrap();
                let mut input = empty();
                input.held.up = input_bits & 8 != 0;
                input.held.down = input_bits & 4 != 0;
                input.held.right = input_bits & 1 != 0;
                input.held.jump = input_bits & 128 != 0;
                input.pressed.jump = input.held.jump;
                player.update_with_environment(
                    input,
                    Contact {
                        ladder: case == "ladder",
                        ..Contact::default()
                    },
                );
                player.body.position.x += player.body.velocity.x;
                player.body.position.y += player.body.velocity.y;
                let actual = [
                    (player.body.center().x * 256.).rem_euclid(65536.),
                    (player.body.center().y * 256.).rem_euclid(65536.),
                    player.body.velocity.x * 256.,
                    player.body.velocity.y * 256.,
                ];
                let expected = [
                    f[3].parse::<f32>().unwrap(),
                    f[4].parse().unwrap(),
                    f[5].parse().unwrap(),
                    f[6].parse().unwrap(),
                ];
                assert_eq!(actual, expected, "{case}, tick {}", f[1]);
                if case == "death" {
                    assert_eq!(
                        player.state,
                        if f[7] == "9" {
                            PlayerState::Ghost
                        } else {
                            PlayerState::Dying
                        }
                    );
                }
            }
        }
    }
    #[test]
    fn cooperative_bumps_match_original_subroutine_outcomes() {
        for row in include_str!("../tests/fixtures/player-coop.csv")
            .lines()
            .skip(1)
        {
            let f: Vec<_> = row.split(',').collect();
            let scenario: u8 = f[0].parse().unwrap();
            let mut players = [
                Player::new(0, Vec2::new(127., 96.)),
                Player::new(1, Vec2::new(127., 90.)),
            ];
            players[1].body.velocity.y = 1.;
            let mut inputs = [empty(); 4];
            if scenario == 1 || scenario == 2 {
                inputs[1].held.jump = true;
            }
            if scenario == 2 {
                players[1].feather = true;
            }
            if scenario == 3 {
                players[1].body.velocity.y = -0.5;
            }
            if scenario == 4 {
                players[0].body.position.x = 125.;
                players[1].body.position = Vec2::new(130., 96.);
                players[0].body.velocity.x = 1.;
                players[1].body.velocity.x = -1.;
            }
            interact_players(&mut players, inputs);
            let actual = [
                players[1].body.velocity.y * 256.,
                players[0].body.velocity.y * 256.,
                players[0].input_lock as f32,
                players[0].body.velocity.x * 256.,
                players[1].body.velocity.x * 256.,
            ];
            let expected = [
                f[1].parse::<f32>().unwrap(),
                f[2].parse().unwrap(),
                f[3].parse().unwrap(),
                f[4].parse().unwrap(),
                f[5].parse().unwrap(),
            ];
            assert_eq!(actual, expected, "scenario {scenario}");
        }
    }
    #[test]
    fn seeking_matches_source_subpixel_easing_and_arrival() {
        let mut player = Player::new(0, Vec2::new(127. + 107. / 256., 96. + 193. / 256.));
        player.begin_seek(Vec2::new(156., 116.));
        for row in include_str!("../tests/fixtures/player-seeking.csv")
            .lines()
            .skip(1)
        {
            let f: Vec<_> = row.split(',').collect();
            player.update(empty(), 0);
            player.body.position.x += player.body.velocity.x;
            player.body.position.y += player.body.velocity.y;
            let actual = [
                player.body.center().x * 256.,
                player.body.center().y * 256.,
                player.body.velocity.x * 256.,
                player.body.velocity.y * 256.,
            ];
            let expected = [
                f[1].parse::<f32>().unwrap(),
                f[2].parse().unwrap(),
                f[3].parse().unwrap(),
                f[4].parse().unwrap(),
            ];
            assert_eq!(actual, expected, "tick {}", f[0]);
            assert_eq!(player.is_warping(), f[5] == "5");
        }
    }
    #[test]
    fn viewport_guards_stop_top_escape_and_preserve_subpixels() {
        let mut player = Player::new(0, Vec2::new(-10.25, -1700.75));
        player.body.velocity.y = -2.0;
        player.clamp_to_viewport(80.0);
        assert_eq!(player.body.center(), Vec2::new(4.75, 86.25));
        assert_eq!(player.body.velocity.y, 0.0);
        player.body.position = Vec2::new(280.5, 400.5);
        player.body.velocity.y = 4.0;
        player.clamp_to_viewport(80.0);
        assert_eq!(player.body.center(), Vec2::new(252.5, 320.5));
        assert_eq!(player.body.velocity.y, 0.0);
    }
}
