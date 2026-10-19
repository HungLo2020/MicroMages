//! Named enemy behavior, independent of rendering and collision resolution.
//!
//! Positions and velocities are pixels and pixels per 60 Hz simulation tick.
//! See `docs/enemy-behavior.md` for recovered facts and reconstruction limits.
use crate::physics::{Body, Vec2};
#[path = "enemy_motion.rs"]
mod motion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyKind {
    Goat,
    Troll,
    Willowisp,
    Snake,
    Walker,
    Hopper,
    Leaper,
    SummoningPlatform,
    SummonedWalker,
    Chaser,
    Minion,
    Spiderweb,
    Turret,
    DiagonalFlyer,
    Bomb,
    Lightning,
    Spike,
    Boss(BossKind),
}

/// Original projectile callback result (f9 at c91f): damage, absorb, or pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitResponse {
    Apply,
    Absorb,
    Pass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossKind {
    Inhaling,
    Summoning,
    Flying,
    Final,
    Sorcerer,
    Segmented,
}

impl EnemyKind {
    pub fn source_id(self) -> u8 {
        match self {
            Self::Boss(BossKind::Inhaling) => 1,
            Self::Boss(BossKind::Summoning) => 2,
            Self::Boss(BossKind::Flying) => 3,
            Self::Boss(BossKind::Final) => 6,
            Self::Boss(BossKind::Sorcerer) => 8,
            Self::Boss(BossKind::Segmented) => 0x2f,
            Self::DiagonalFlyer => 0x12,
            Self::Bomb => 0x0a,
            Self::Lightning => 0x1b,
            Self::Spike => 0x19,
            Self::Goat => 7,
            Self::Willowisp => 9,
            Self::Troll => 0x0b,
            Self::Snake => 0x0c,
            Self::Walker => 0x0e,
            Self::Hopper => 0x14,
            Self::Leaper => 0x18,
            Self::SummoningPlatform => 0x24,
            Self::SummonedWalker => 0x10,
            Self::Chaser => 0x0d,
            Self::Minion => 0x16,
            Self::Spiderweb => 0x1d,
            Self::Turret => 5,
        }
    }

    pub fn uses_gravity(self) -> bool {
        matches!(
            self,
            Self::Hopper | Self::Leaper | Self::SummonedWalker | Self::Snake
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossPhase {
    Waiting,
    Inhale,
    Charge,
    Dash,
    Recover,
    Summon,
    Windup,
    Volley,
    Rain,
    Transform,
    Chase,
    Approach,
    Flash,
    Patrol,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyState {
    Idle,
    Moving,
    Attacking,
    Fuse,
    Boss(BossPhase),
    Defeated,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileKind {
    Bubble,
    Spear,
    Spark,
    Spell,
    FallingHazard,
    Lightning,
    Orb,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnemyAttack {
    pub kind: ProjectileKind,
    pub position: Vec2,
    pub velocity: Vec2,
}

/// Shared original random stream. Each feedback step supplies high then low byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnemyRng {
    state: u16,
    low_pending: bool,
}
impl EnemyRng {
    pub fn new(seed: u16) -> Self {
        Self {
            state: seed,
            low_pending: false,
        }
    }
    pub fn byte(&mut self) -> u8 {
        if self.low_pending {
            self.low_pending = false;
            return self.state as u8;
        }
        let feedback = self.state & 1 != 0;
        self.state >>= 1;
        if feedback {
            self.state ^= 0xb400;
        }
        self.low_pending = true;
        (self.state >> 8) as u8
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EnemyContext {
    /// Centers of active, vulnerable players; unavailable/dead players are None.
    pub players: [Option<Vec2>; 4],
    pub camera_y: f32,
    pub frame_index: u64,
    pub active_minions: usize,
    pub active_chasers: usize,
    pub extra_mode: bool,
    pub nearby_shots: [Option<Vec2>; 12],
    pub at_ledge: bool,
    pub turret_active: bool,
    pub turret_period: u16,
}
impl EnemyContext {
    pub fn single(target: Option<Vec2>, frame_index: u64) -> Self {
        Self {
            players: [target, None, None, None],
            camera_y: 0.,
            frame_index,
            active_minions: 0,
            active_chasers: 0,
            extra_mode: false,
            nearby_shots: [None; 12],
            at_ledge: false,
            turret_active: false,
            turret_period: 216,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WorldAction {
    PullPlayersBelow {
        y: f32,
        velocity_y: f32,
    },
    BreakTile {
        x: usize,
        y: usize,
    },
    SpawnEnemy {
        kind: EnemyKind,
        position: Vec2,
        velocity: Vec2,
        target: Option<usize>,
    },
    Particle {
        position: Vec2,
        velocity: Vec2,
    },
    LockPlayerControls {
        ticks: u8,
    },
}

#[derive(Debug, Clone, Default)]
pub struct EnemyFrame {
    pub attacks: Vec<EnemyAttack>,
    pub actions: Vec<WorldAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinalAction {
    MoveCenter,
    CenterPause,
    Aim,
    TripleShot,
    AfterShots,
    MoveLower,
    LowerPause,
    MoveMiddle,
    MiddlePause,
    UpperPause,
    DashCharge,
    Dash,
    DashRecovery,
}

#[derive(Debug, Clone)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub source_kind: u8,
    pub update_phase: u8,
    pub body: Body,
    pub health: u16,
    pub animation_frame: u16,
    pub segments: Vec<Body>,
    pub(crate) segments_hit: u8,
    pub state: EnemyState,
    pub facing: f32,
    pub invulnerable_ticks: u16,
    pub frozen_ticks: u16,
    palette_effect_ticks: u16,
    palette_remapped: bool,
    thaw_velocity: Vec2,
    clock_ticks: u64,
    stunned_this_tick: bool,
    timer: u16,
    hard: bool,
    age: u64,
    burst_remaining: u8,
    movement_step: u8,
    movement_wait: u16,
    pub boss_health_phase: u8,
    pattern: u8,
    shots_remaining: u8,
    boss_cycles: u16,
    final_action: FinalAction,
    context: EnemyContext,
    actions: Vec<WorldAction>,
    selected_player: Option<usize>,
    random: EnemyRng,
    navigation_target: Option<Vec2>,
    hazard_x: u8,
    bobbing_down: bool,
    motion_paused: bool,
    arena_waiting: bool,
    activation_ticks: u8,
    attack_protected: bool,
    chase_updates_left: u16,
    patrol_steps_remaining: u8,
    fleeing: bool,
    animation_started: u64,
    wall_turns: u8,
    previous_wall: i8,
    lightning_boss: bool,
    lightning_probe: Option<f32>,
}

impl Enemy {
    pub fn new(kind: EnemyKind, position: Vec2, hard: bool) -> Self {
        let boss = matches!(kind, EnemyKind::Boss(_));
        // Source initial hit-point table at b7c1; these are raw damage units.
        let health = match kind {
            EnemyKind::Boss(BossKind::Inhaling | BossKind::Summoning) => 76,
            EnemyKind::Boss(BossKind::Flying) => 90,
            EnemyKind::Boss(BossKind::Final) => 74,
            EnemyKind::Boss(BossKind::Sorcerer) => 110,
            EnemyKind::Boss(BossKind::Segmented) => 30,
            EnemyKind::DiagonalFlyer => 4,
            EnemyKind::Bomb => 8,
            EnemyKind::Goat => 10 + u16::from(hard),
            EnemyKind::Troll => 12,
            EnemyKind::Willowisp => 88,
            EnemyKind::Snake => 4 + u16::from(hard),
            EnemyKind::Walker => 4,
            EnemyKind::Hopper => 6,
            EnemyKind::Leaper => 4 + u16::from(hard),
            EnemyKind::SummoningPlatform => 33,
            EnemyKind::SummonedWalker => {
                if hard {
                    7
                } else {
                    6
                }
            }
            EnemyKind::Chaser => 2,
            EnemyKind::Minion => 32,
            EnemyKind::Turret => 20,
            _ => 1,
        };
        Self {
            kind,
            source_kind: kind.source_id(),
            update_phase: 0,
            body: Body {
                position,
                velocity: Vec2 { x: 0., y: 0. },
                size: Vec2 {
                    x: if boss { 32. } else { 8. },
                    y: if boss { 32. } else { 8. },
                },
                grounded: false,
            },
            health,
            segments_hit: 0,
            segments: if kind == EnemyKind::Boss(BossKind::Segmented) {
                (0..30)
                    .map(|i| Body {
                        position: Vec2 {
                            x: 40. + (i % 6) as f32 * 29.,
                            y: 36. + (i / 6) as f32 * 20.,
                        },
                        velocity: Vec2 { x: 0., y: 0. },
                        size: Vec2 { x: 8., y: 8. },
                        grounded: false,
                    })
                    .collect()
            } else {
                Vec::new()
            },
            animation_frame: match kind {
                EnemyKind::Troll => 12,
                EnemyKind::Snake
                | EnemyKind::Walker
                | EnemyKind::Hopper
                | EnemyKind::Leaper
                | EnemyKind::Chaser
                | EnemyKind::DiagonalFlyer => 1,
                _ => 0,
            },
            state: if boss {
                EnemyState::Boss(BossPhase::Waiting)
            } else {
                EnemyState::Idle
            },
            facing: -1.,
            invulnerable_ticks: 0,
            frozen_ticks: 0,
            palette_effect_ticks: 0,
            palette_remapped: false,
            clock_ticks: 0,
            stunned_this_tick: false,
            thaw_velocity: Vec2 { x: 0., y: 0. },
            timer: match kind {
                EnemyKind::Goat => {
                    if hard {
                        45
                    } else {
                        130
                    }
                }
                EnemyKind::Hopper | EnemyKind::Turret => 0,
                EnemyKind::Leaper => 20,
                EnemyKind::Lightning => ((position.x + 4.) / 2.) as u16,
                EnemyKind::Troll => {
                    if hard {
                        112
                    } else {
                        144
                    }
                }
                EnemyKind::Boss(BossKind::Sorcerer) => 211,
                EnemyKind::Boss(BossKind::Summoning) => 62,
                EnemyKind::Boss(BossKind::Flying) => 82,
                _ => 60,
            },
            hard,
            age: 0,
            burst_remaining: 0,
            movement_step: 0,
            movement_wait: 40,
            boss_health_phase: 0,
            pattern: 0,
            shots_remaining: 0,
            boss_cycles: 0,
            final_action: FinalAction::MoveCenter,
            context: EnemyContext::single(None, 0),
            actions: Vec::new(),
            selected_player: None,
            random: EnemyRng::new(0),
            navigation_target: None,
            hazard_x: 128,
            bobbing_down: true,
            motion_paused: false,
            arena_waiting: false,
            activation_ticks: 0,
            attack_protected: false,
            chase_updates_left: if kind == EnemyKind::Minion { 102 } else { 256 },
            patrol_steps_remaining: 35,
            fleeing: false,
            animation_started: 0,
            wall_turns: 0,
            previous_wall: 0,
            lightning_boss: false,
            lightning_probe: None,
        }
    }

    /// A depleted willowisp remains allocated until its next successful hit.
    /// Its gradual health decay alone does not run the destruction callback.
    pub fn is_alive(&self) -> bool {
        self.health != 0
            || (self.kind == EnemyKind::Willowisp && self.state != EnemyState::Defeated)
    }

    /// Bosses reject freezing; repeated hits extend rather than erase the hold.
    pub fn wait_for_arena(&mut self) {
        self.arena_waiting = true;
    }
    pub fn activate_arena(&mut self) {
        self.arena_waiting = false;
        self.activation_ticks = if matches!(
            self.kind,
            EnemyKind::Boss(
                BossKind::Inhaling | BossKind::Summoning | BossKind::Flying | BossKind::Segmented
            )
        ) {
            47
        } else {
            0
        };
    }
    pub fn start_lightning_warning(&mut self, doubled: bool) {
        self.lightning_boss |= doubled;
        self.animation_started = self.age;
        self.state = EnemyState::Moving;
        self.timer = (if self.hard { 27 } else { 40 }) * if doubled { 2 } else { 1 };
    }

    pub fn arena_active(&self) -> bool {
        !self.arena_waiting && self.activation_ticks == 0 && self.health != 0
    }

    pub fn can_accept_hit(&self) -> bool {
        self.is_alive()
            && !matches!(
                self.kind,
                EnemyKind::SummoningPlatform
                    | EnemyKind::Lightning
                    | EnemyKind::Spike
                    | EnemyKind::Spiderweb
            )
            && self.invulnerable_ticks == 0
            && !self.arena_waiting
            && self.activation_ticks == 0
            && !self.attack_protected
            && self.state != EnemyState::Boss(BossPhase::Transform)
    }

    pub fn hit_response(&self, damage: u16) -> HitResponse {
        if !self.is_alive()
            || matches!(
                self.kind,
                EnemyKind::SummoningPlatform
                    | EnemyKind::Lightning
                    | EnemyKind::Spike
                    | EnemyKind::Spiderweb
            )
        {
            return HitResponse::Pass;
        }
        if matches!(self.kind, EnemyKind::Boss(_)) {
            if self.arena_waiting || self.activation_ticks != 0 {
                return HitResponse::Pass;
            }
            if self.invulnerable_ticks != 0
                || self.attack_protected
                || self.state == EnemyState::Boss(BossPhase::Transform)
            {
                return HitResponse::Absorb;
            }
        }
        if self.kind == EnemyKind::Bomb && damage >= self.health {
            return if self.state == EnemyState::Fuse {
                HitResponse::Pass
            } else {
                HitResponse::Absorb
            };
        }
        HitResponse::Apply
    }

    /// Units awarded by the original hit reward table (84af/ec84).
    pub fn score_units(&self) -> u8 {
        match self.source_kind {
            1 | 2 | 3 | 6 | 8 => 10,
            5 | 7 | 9 | 10 | 11 => 5,
            12 | 20 | 24 => 2,
            14 | 18 | 19 => 1,
            _ => 0,
        }
    }

    fn troll_animation_tick(&self) -> u64 {
        let delay = if self.hard { 96 } else { 128 };
        if self.age < delay {
            self.age
        } else {
            (self.age - delay) % (delay + 16) + 1
        }
    }

    pub fn visible(&self) -> bool {
        self.is_alive() && !(self.kind == EnemyKind::Lightning && self.state == EnemyState::Idle)
    }

    pub fn animation_for_part(&self, part: usize) -> Option<(&'static str, u64)> {
        if self.kind == EnemyKind::Lightning {
            let name = if self.state != EnemyState::Attacking {
                "lightning_warning"
            } else if self.lightning_boss && part < 2 {
                "lightning_middle"
            } else {
                "lightning_end"
            };
            return Some((name, self.age - self.animation_started));
        }
        self.animation()
    }

    /// Named editable animation and its local playback clock.
    pub fn animation(&self) -> Option<(&'static str, u64)> {
        let name = match self.kind {
            EnemyKind::Troll => return Some(("troll_attack", self.troll_animation_tick())),
            EnemyKind::Goat if self.burst_remaining != 0 => "goat_attack",
            EnemyKind::Goat => "goat_idle",
            EnemyKind::Walker if self.state == EnemyState::Attacking => {
                return Some(("walker_attack", self.age - self.animation_started + 1));
            }
            EnemyKind::Walker => return Some(("walker_walk", self.age - self.animation_started)),
            EnemyKind::DiagonalFlyer if self.state == EnemyState::Idle => return None,
            EnemyKind::DiagonalFlyer => {
                return Some((
                    if self.state == EnemyState::Moving {
                        "minion_fly"
                    } else {
                        "diagonal_flyer_awaken"
                    },
                    self.age - self.animation_started,
                ));
            }
            EnemyKind::Snake if self.state == EnemyState::Idle => "snake_idle",
            EnemyKind::Snake => return Some(("snake_attack", self.age - self.animation_started)),
            EnemyKind::Willowisp => {
                return Some((
                    if self.health < 88 {
                        "willowisp_hurt"
                    } else {
                        "willowisp_idle"
                    },
                    self.age - self.animation_started,
                ));
            }
            EnemyKind::Bomb => "bomb_walk",
            EnemyKind::Chaser => "chaser_fly",
            EnemyKind::Minion => "minion_fly",
            EnemyKind::SummonedWalker => "turret_minion_walk",
            EnemyKind::Turret if self.state == EnemyState::Attacking => "turret_fire",
            EnemyKind::Turret | EnemyKind::SummoningPlatform => "boss_idle",
            EnemyKind::Leaper if self.body.grounded => {
                return Some(("hopper_grounded", self.age - self.animation_started));
            }
            EnemyKind::Boss(BossKind::Sorcerer)
                if self.boss_health_phase >= 2
                    && (self.state != EnemyState::Boss(BossPhase::Transform)
                        || self.timer <= 129) =>
            {
                "sorcerer_boss_transformed"
            }
            EnemyKind::Boss(BossKind::Sorcerer) if self.boss_health_phase >= 1 => {
                "sorcerer_boss_wounded"
            }
            EnemyKind::Boss(BossKind::Sorcerer) => "sorcerer_boss_idle",
            _ => return None,
        };
        Some((name, self.age))
    }

    pub fn set_target(&mut self, player: Option<usize>) {
        self.selected_player = player.filter(|&index| index < 4);
    }

    /// Source f6e7 uses the same hit callback as spells. c545 rejects both.
    pub fn can_freeze(&self) -> bool {
        self.is_alive()
            && !matches!(
                self.kind,
                EnemyKind::Boss(_)
                    | EnemyKind::SummoningPlatform
                    | EnemyKind::Lightning
                    | EnemyKind::Spike
                    | EnemyKind::Spiderweb
            )
    }

    pub fn freeze(&mut self, ticks: u16) {
        if !self.can_freeze() || ticks == 0 {
            return;
        }
        if self.frozen_ticks == 0 {
            self.thaw_velocity = self.body.velocity;
        }
        self.frozen_ticks = self.frozen_ticks.max(ticks);
        self.palette_effect_ticks = self.frozen_ticks.saturating_sub(52);
        self.palette_remapped = true;
        self.stunned_this_tick = true;
        self.body.velocity = Vec2 { x: 0., y: 0. };
    }

    /// Dynamically summoned actors remain scheduled until their source bounds
    /// cleanup runs, including the final frozen ticks outside the viewport.
    pub fn needs_viewport_update(&self) -> bool {
        self.kind == EnemyKind::Minion || (self.kind == EnemyKind::Chaser && self.age != 0)
    }

    /// Source 823e/8247 clears the actor slot after movement, not on freezing.
    pub fn remove_if_outside_viewport(&mut self, camera_y: f32) -> bool {
        if self.health == 0 || self.is_stunned() {
            return false;
        }
        let checks_bounds = self.kind == EnemyKind::Minion
            || (self.kind == EnemyKind::Chaser
                && self.context.frame_index % 3 != u64::from(self.update_phase) % 3);
        let center = self.body.center();
        if checks_bounds
            && (!(7. ..249.).contains(&center.x.floor())
                || !(0. ..240.).contains(&(center.y.floor() - camera_y.floor())))
        {
            self.health = 0;
            self.state = EnemyState::Defeated;
            return true;
        }
        false
    }

    /// Original a3bc/a3e7 palette flash, shared by freezing and nonfatal hits.
    pub fn display_palette(&self, base: u8) -> u8 {
        if self.palette_remapped {
            [2, 2, 3, 2][usize::from(base & 3)]
        } else {
            base
        }
    }

    fn update_palette_effect(&mut self) {
        if self.palette_effect_ticks == 0 {
            return;
        }
        self.palette_remapped =
            self.frozen_ticks == 0 || self.frozen_ticks >= 105 || self.frozen_ticks & 4 != 0;
        if self.palette_remapped {
            self.palette_effect_ticks -= 1;
            if self.palette_effect_ticks == 0 {
                self.palette_remapped = false;
            }
        }
    }

    pub fn motion_paused(&self) -> bool {
        self.motion_paused
    }

    pub fn is_stunned(&self) -> bool {
        self.stunned_this_tick
    }
    pub fn contact_enabled(&self) -> bool {
        if self.kind == EnemyKind::Lightning {
            return self.lightning_probe.is_some() && !self.stunned_this_tick;
        }
        self.is_alive()
            && !self.stunned_this_tick
            && self.kind != EnemyKind::SummoningPlatform
            && (self.kind != EnemyKind::Lightning || self.state == EnemyState::Attacking)
    }

    pub fn overlaps(&self, body: &Body) -> bool {
        if self.kind == EnemyKind::Lightning {
            return self.lightning_probe.is_some_and(|offset| {
                let parts = if self.segments.is_empty() {
                    vec![self.body]
                } else {
                    self.segments.clone()
                };
                parts.into_iter().any(|mut part| {
                    part.position.y += offset;
                    part.overlaps(body)
                })
            });
        }
        if self.kind == EnemyKind::Boss(BossKind::Segmented) {
            self.segments.iter().any(|segment| segment.overlaps(body))
        } else {
            self.body.overlaps(body)
        }
    }

    /// Collision shapes also provide the renderer with each visible boss segment.
    pub fn bodies(&self) -> Vec<Body> {
        if self.kind == EnemyKind::Lightning {
            if self.health == 0 {
                return Vec::new();
            }
            if !self.segments.is_empty() {
                return self.segments.clone();
            }
        }
        if self.kind == EnemyKind::Boss(BossKind::Segmented) {
            self.segments.clone()
        } else {
            vec![self.body]
        }
    }

    pub fn hit_at(&mut self, point: Vec2, damage: u16) -> bool {
        if self.kind != EnemyKind::Boss(BossKind::Segmented) {
            return self.hit(damage);
        }
        if damage == 0 || self.health == 0 {
            return false;
        }
        if let Some(index) = self.segments.iter().position(|b| {
            point.x >= b.position.x - 4.
                && point.x <= b.position.x + b.size.x + 4.
                && point.y >= b.position.y - 4.
                && point.y <= b.position.y + b.size.y + 4.
        }) {
            self.segments.remove(index);
            self.segments_hit += 1;
            self.health = self.segments.len() as u16;
            if self.health == 0 {
                self.state = EnemyState::Defeated;
                return true;
            }
        }
        false
    }

    fn tick_segments(&mut self, random: &mut EnemyRng) -> Vec<EnemyAttack> {
        self.movement_wait = self.movement_wait.saturating_sub(1);
        if self.movement_wait == 0 {
            let step = self.movement_step;
            self.movement_step = (step + 1) % 32;
            let (dx, dy) = match step {
                15 | 31 => (0., 8.),
                0..=14 => (2., 0.),
                _ => (-2., 0.),
            };
            for segment in &mut self.segments {
                segment.position.x += dx;
                segment.position.y += dy;
            }
            self.segments.retain(|s| s.position.y + 4. < 156.);
            self.health = self.segments.len() as u16;
            self.movement_wait = match self.segments.len() {
                0..=5 => 2,
                6..=14 => 4,
                15..=20 => 8,
                _ => 16,
            };
        }
        self.animation_frame = u16::from(self.hard) + u16::from(self.movement_step & 1) * 2;
        if self.segments.is_empty() {
            self.health = 0;
            self.state = EnemyState::Defeated;
        }
        // Source chooses a random live segment every64ticks after the first kill.
        if self.age.is_multiple_of(64) && self.segments.len() < 30 && !self.segments.is_empty() {
            let index = usize::from((random.byte() >> 1) & 15);
            if index >= self.segments.len() {
                return Vec::new();
            }
            let b = self.segments[index];
            vec![EnemyAttack {
                kind: ProjectileKind::Spell,
                position: b.center(),
                velocity: Vec2 { x: 0., y: 2. },
            }]
        } else {
            Vec::new()
        }
    }

    /// Newly defeated returns true exactly once. Boss hit protection is 20 ticks.
    pub fn hit(&mut self, damage: u16) -> bool {
        if damage == 0 || self.hit_response(damage) == HitResponse::Pass {
            return false;
        }
        if self.kind == EnemyKind::Bomb && damage >= self.health {
            // fa66: lethal damage arms a 60-tick fuse instead of immediate death.
            self.state = EnemyState::Fuse;
            self.timer = 60;
            self.body.velocity.x = 0.;
            return false;
        }
        if self.hit_response(damage) == HitResponse::Absorb {
            return false;
        }
        if self.kind == EnemyKind::DiagonalFlyer && self.state == EnemyState::Idle {
            self.state = EnemyState::Attacking;
            self.animation_started = self.age;
            self.timer = if self.hard { 18 } else { 26 };
        }
        if self.kind == EnemyKind::Willowisp {
            // bce9 selects the repeating 9659 hurt stream on every nonzero hit.
            self.animation_started = self.age;
        }
        self.health = self.health.saturating_sub(damage);
        if self.kind == EnemyKind::Boss(BossKind::Sorcerer) {
            if self.health < 65 {
                self.boss_health_phase = self.boss_health_phase.max(1);
            }
            if self.health < 41 && self.health != 0 {
                if !self.hard {
                    self.health = 0;
                } else if self.boss_health_phase < 2 {
                    self.boss_health_phase = 2;
                    self.state = EnemyState::Boss(BossPhase::Transform);
                    self.timer = 120 + 60 + 13 * 3 + 31;
                }
            }
        }
        if self.health == 0 {
            self.state = EnemyState::Defeated;
            self.body.velocity = Vec2 { x: 0., y: 0. };
            return true;
        }
        self.palette_effect_ticks = 10;
        self.frozen_ticks = 0;
        self.invulnerable_ticks = if matches!(self.kind, EnemyKind::Boss(_)) {
            20
        } else {
            0
        };
        false
    }

    pub fn set_wall_contact(&mut self, wall: i8) {
        self.previous_wall = wall;
    }

    pub fn reverse(&mut self) {
        if self.kind == EnemyKind::SummonedWalker {
            self.wall_turns += 1;
            if self.wall_turns >= 2 {
                self.health = 0;
                self.state = EnemyState::Defeated;
                self.motion_paused = true;
            }
            self.facing = -self.facing;
            return;
        }
        self.facing = -self.facing;
        self.body.velocity.x = -self.body.velocity.x;
    }

    /// Choose movement and attacks. The world owns integration and terrain collision.
    pub fn tick(&mut self, target: Option<Vec2>) -> Vec<EnemyAttack> {
        let context = EnemyContext::single(target, self.context.frame_index + 1);
        let mut random = self.random;
        let output = self.tick_with_context(&context, &mut random);
        self.random = random;
        output.attacks
    }

    pub fn tick_with_context(
        &mut self,
        context: &EnemyContext,
        random: &mut EnemyRng,
    ) -> EnemyFrame {
        self.context = *context;
        self.motion_paused = false;
        self.actions.clear();
        let target = self
            .selected_player
            .and_then(|index| context.players[index])
            .or_else(|| context.players.into_iter().flatten().next());
        let attacks = self.tick_internal(target, random);
        EnemyFrame {
            attacks,
            actions: std::mem::take(&mut self.actions),
        }
    }

    fn choose_player(&mut self, random: &mut EnemyRng) -> Option<Vec2> {
        let start = usize::from((random.byte() >> 1) & 3);
        for offset in 0..4 {
            let index = (start + offset) % 4;
            if let Some(target) = self.context.players[index] {
                self.selected_player = Some(index);
                return Some(target);
            }
        }
        self.selected_player = None;
        None
    }

    fn tick_internal(&mut self, target: Option<Vec2>, random: &mut EnemyRng) -> Vec<EnemyAttack> {
        if !self.is_alive() {
            return Vec::new();
        }
        if self.arena_waiting || self.activation_ticks != 0 {
            self.activation_ticks = self.activation_ticks.saturating_sub(1);
            self.motion_paused = true;
            return Vec::new();
        }
        self.clock_ticks = self.context.frame_index;
        if self.frozen_ticks == 0 {
            self.update_palette_effect();
        }
        if self.frozen_ticks != 0 {
            let previous = self.frozen_ticks;
            self.frozen_ticks -= 1;
            self.update_palette_effect();
            let skip = previous >= 105 || !self.clock_ticks.is_multiple_of(2);
            if skip {
                if !self.stunned_this_tick {
                    self.thaw_velocity = self.body.velocity;
                }
                self.stunned_this_tick = true;
                self.body.velocity = Vec2 { x: 0., y: 0. };
                return Vec::new();
            }
            // The source dispatcher halves its local frame counter on the
            // alternate thaw updates; each actor's cadence uses that counter.
            self.clock_ticks /= 2;
        }
        if self.stunned_this_tick {
            self.body.velocity = self.thaw_velocity;
        }
        self.stunned_this_tick = false;
        self.age = self.age.wrapping_add(1);
        self.animation_frame = match self.kind {
            EnemyKind::Walker => [1, 2, 3, 2][(self.age / 10 % 4) as usize],
            EnemyKind::Goat => [0, 4, 8, 12, 16][(self.age / 8 % 5) as usize],
            EnemyKind::Snake => [1, 2][(self.age / 12 % 2) as usize],
            EnemyKind::Chaser => [1, 2, 3, 4][(self.age / 3 % 4) as usize],
            EnemyKind::Troll => {
                let tick = self.troll_animation_tick();
                if tick < 16 {
                    12
                } else if tick < 48 {
                    16
                } else {
                    [0, 4, 8, 4][((tick - 48) / 8 % 4) as usize]
                }
            }
            EnemyKind::Bomb => match self.age % 60 {
                0..=19 => 0,
                20..=29 | 50..=59 => 2,
                _ => 4,
            },
            _ => self.animation_frame,
        };
        if self.kind == EnemyKind::Boss(BossKind::Segmented) {
            return self.tick_segments(random);
        }
        self.invulnerable_ticks = self.invulnerable_ticks.saturating_sub(1);
        self.timer = self.timer.saturating_sub(1);
        if self.state == EnemyState::Fuse {
            if self.timer == 0 {
                self.health = 0;
                self.state = EnemyState::Defeated;
                return [-1., 1.]
                    .into_iter()
                    .map(|x| self.attack(ProjectileKind::Spark, Vec2 { x: x * 0.5, y: 0.5 }))
                    .collect();
            }
            return Vec::new();
        }
        let Some(target) = target else {
            self.body.velocity.x = 0.;
            return Vec::new();
        };
        let delta = Vec2 {
            x: target.x - self.body.center().x,
            y: target.y - self.body.center().y,
        };
        let toward = if delta.x < 0. { -1. } else { 1. };
        let mut attacks = Vec::new();
        match self.kind {
            EnemyKind::Goat => {
                self.body.velocity.x = 0.;
                if self.burst_remaining == 0 && self.timer == 0 {
                    self.burst_remaining = if self.hard { 3 } else { 2 };
                    self.timer = if self.hard { 79 } else { 99 };
                    self.state = EnemyState::Attacking;
                } else if self.burst_remaining != 0 {
                    if self.timer == 20 {
                        attacks.push(self.attack(
                            ProjectileKind::Bubble,
                            Vec2 {
                                x: self.facing,
                                y: -0.5,
                            },
                        ));
                    }
                    if self.timer == 0 {
                        self.burst_remaining -= 1;
                        self.timer = if self.burst_remaining == 0 {
                            self.state = EnemyState::Idle;
                            if self.hard { 45 } else { 130 }
                        } else if self.hard {
                            79
                        } else {
                            99
                        };
                    }
                }
            }
            EnemyKind::Troll => {
                self.body.velocity.x = 0.;
                if self.timer == 0 {
                    self.state = EnemyState::Attacking;
                    let mut shot = self.attack(ProjectileKind::Spear, Vec2::new(self.facing, -3.));
                    shot.position.x += self.facing * 4.;
                    attacks.push(shot);
                    self.timer = if self.hard { 112 } else { 144 };
                } else {
                    self.state = EnemyState::Idle;
                }
            }
            EnemyKind::Turret => {
                self.body.velocity = Vec2 { x: 0., y: 0. };
                let even = (self.clock_ticks + u64::from(self.update_phase)).is_multiple_of(2);
                if self.state == EnemyState::Idle {
                    if even && self.context.turret_active {
                        self.body.position.y -= 1.;
                        if self.body.center().y - self.context.camera_y < 93. {
                            self.state = EnemyState::Moving;
                            self.timer = self.body.center().x.ceil() as u16 / 2
                                + (self.body.center().x as u16 % 2)
                                + 55;
                        }
                    }
                } else if self.state == EnemyState::Moving {
                    // The shared decrement above runs every tick; restore on odd frames.
                    if !even {
                        self.timer = self.timer.saturating_add(1);
                    }
                    if self.timer == 0 {
                        self.actions.push(WorldAction::SpawnEnemy {
                            kind: EnemyKind::SummonedWalker,
                            position: Vec2 {
                                x: self.body.center().x + self.facing * 16.,
                                y: self.body.center().y,
                            },
                            velocity: Vec2 {
                                x: self.facing * if self.hard { 168. / 256. } else { 152. / 256. },
                                y: 0.,
                            },
                            target: None,
                        });
                        self.timer = 16;
                        self.state = EnemyState::Attacking;
                    }
                } else if self.timer == 0 {
                    self.state = EnemyState::Moving;
                    self.timer = self.context.turret_period;
                }
            }
            EnemyKind::SummonedWalker => {
                if self.state == EnemyState::Idle && self.body.grounded {
                    self.state = EnemyState::Moving;
                    self.body.velocity.y = -1.;
                    self.body.grounded = false;
                } else if self.state == EnemyState::Moving {
                    self.body.velocity.x =
                        self.facing * if self.hard { 168. / 256. } else { 152. / 256. };
                    if !(self.clock_ticks + u64::from(self.update_phase)).is_multiple_of(2)
                        && (self.previous_wall != 0
                            || (self.facing > 0. && self.body.center().x >= 246.)
                            || (self.facing < 0. && self.body.center().x < 17.))
                    {
                        self.reverse();
                    }
                } else {
                    self.body.velocity.x = 0.;
                }
            }
            EnemyKind::DiagonalFlyer => {
                if self.state == EnemyState::Idle {
                    if (self.clock_ticks + u64::from(self.update_phase)).is_multiple_of(4)
                        && self
                            .context
                            .players
                            .into_iter()
                            .flatten()
                            .any(|player| motion::manhattan(self.body.center(), player) < 64)
                    {
                        self.state = EnemyState::Attacking;
                        self.animation_started = self.age;
                        self.timer = if self.hard { 18 } else { 26 };
                    }
                } else if self.state == EnemyState::Attacking && self.timer == 0 {
                    self.state = EnemyState::Moving;
                    self.animation_started = self.age;
                    let closest = self
                        .context
                        .players
                        .into_iter()
                        .flatten()
                        .min_by_key(|player| motion::manhattan(self.body.center(), *player))
                        .unwrap_or(target);
                    self.facing = if closest.x < self.body.center().x {
                        -1.
                    } else {
                        1.
                    };
                    let speed = if self.hard { 96. / 256. } else { 64. / 256. };
                    self.body.velocity = Vec2::new(self.facing * speed, speed);
                    // Activation assigns velocity; integration starts next tick.
                    self.motion_paused = true;
                }
            }
            EnemyKind::Lightning => {
                self.lightning_probe = if self.state == EnemyState::Attacking {
                    Some([-12., 4., 12., -4.][usize::from((self.timer + 1) & 3)])
                } else {
                    None
                };
                if self.timer == 0 {
                    match self.state {
                        EnemyState::Idle => self.start_lightning_warning(false),
                        EnemyState::Moving => {
                            self.state = EnemyState::Attacking;
                            self.timer = 16;
                            self.animation_started = self.age;
                            if self.lightning_boss {
                                self.segments = (0..3)
                                    .map(|n| {
                                        let mut b = self.body;
                                        b.position.y += n as f32 * 32.;
                                        b
                                    })
                                    .collect();
                            }
                        }
                        _ if self.lightning_boss => {
                            self.health = 0;
                            self.state = EnemyState::Defeated;
                        }
                        _ => {
                            self.state = EnemyState::Idle;
                            self.timer = 68;
                        }
                    }
                }
            }
            EnemyKind::Spike => {
                self.body.velocity = Vec2 { x: 0., y: 0. };
            }
            EnemyKind::Walker | EnemyKind::Bomb => {
                self.body.velocity.x = 0.;
                if self.kind == EnemyKind::Walker && self.state == EnemyState::Attacking {
                    if self.timer == 17 {
                        attacks.push(
                            self.attack(ProjectileKind::Spark, Vec2::new(self.facing * 0.5, 0.5)),
                        );
                    }
                    if self.timer == 0 {
                        self.state = EnemyState::Moving;
                        self.patrol_steps_remaining = if self.context.extra_mode { 17 } else { 35 };
                        self.animation_started = self.age;
                    }
                } else {
                    self.state = EnemyState::Moving;
                    // A step is one pixel, every four (Walker) or eight (Bomb)
                    // ticks. Extra mode halves these intervals; hard does not.
                    let interval = match (self.kind, self.context.extra_mode) {
                        (EnemyKind::Walker, false) | (EnemyKind::Bomb, true) => 4,
                        (EnemyKind::Walker, true) => 2,
                        _ => 8,
                    };
                    if self.age == 1 && self.context.extra_mode {
                        self.patrol_steps_remaining = 17;
                    }
                    if (self.clock_ticks + u64::from(self.update_phase)).is_multiple_of(interval) {
                        self.body.velocity.x = self.facing;
                        if self.kind == EnemyKind::Walker && self.hard {
                            self.patrol_steps_remaining -= 1;
                            if self.patrol_steps_remaining == 0 {
                                self.state = EnemyState::Attacking;
                                self.timer = 52;
                                self.animation_started = self.age;
                            }
                        }
                    }
                }
            }
            EnemyKind::Hopper => {
                if self.body.grounded {
                    if self.state == EnemyState::Attacking {
                        self.state = EnemyState::Idle;
                        self.timer = if self.hard { 0 } else { 25 };
                    }
                    if self.timer == 0 {
                        let threat = self.context.nearby_shots.into_iter().flatten().any(|shot| {
                            (shot.x - self.body.center().x).abs() <= 32.
                                && (shot.y - self.body.center().y).abs() <= 8.
                        });
                        if (threat || self.context.at_ledge)
                            && self.body.center().y - self.context.camera_y >= 20.
                        {
                            self.body.velocity.y = -2.5;
                            self.body.grounded = false;
                            self.state = EnemyState::Attacking;
                        }
                        if self.context.extra_mode
                            || self.context.frame_index % 3 == u64::from(self.update_phase) % 3
                        {
                            self.body.position.x += self.facing;
                        }
                        self.body.velocity.x = 0.;
                    } else {
                        self.body.velocity.x = 0.;
                    }
                }
            }
            EnemyKind::Leaper => {
                if self.body.center().y - self.context.camera_y >= 23. {
                    if self.body.grounded {
                        self.body.velocity.x = 0.;
                        if self.state != EnemyState::Idle {
                            self.state = EnemyState::Idle;
                            self.timer = 20;
                            self.animation_started = self.age;
                        }
                        if self.timer == 0 {
                            self.body.velocity.y = -2.5;
                            self.body.grounded = false;
                            self.state = EnemyState::Moving;
                        }
                    } else {
                        self.state = EnemyState::Moving;
                        self.body.velocity.x = self.facing * 160. / 256.;
                    }
                }
            }
            EnemyKind::SummoningPlatform => {
                self.body.velocity = Vec2 { x: 0., y: 0. };
            }
            EnemyKind::Chaser | EnemyKind::Minion | EnemyKind::Willowisp => {
                if self.kind == EnemyKind::Willowisp
                    && self.health < 88
                    && self.health & 15 != 0
                    && !self.context.frame_index.is_multiple_of(2)
                {
                    self.health -= 1;
                }
                let after_movement = self.kind == EnemyKind::Minion;
                if after_movement {
                    self.body.position.x += self.body.velocity.x;
                    self.body.position.y += self.body.velocity.y;
                }
                self.facing = toward;
                self.state = EnemyState::Moving;
                let (acceleration, maximum) = match self.kind {
                    EnemyKind::Chaser => (12, if self.hard { 116. } else { 88. }),
                    EnemyKind::Minion => (
                        if self.hard { 32 } else { 28 },
                        if self.hard { 116. } else { 112. },
                    ),
                    _ if self.health < 88 => (
                        if self.hard { 16 } else { 12 },
                        if self.hard { 124. } else { 100. },
                    ),
                    _ => (
                        if self.hard { 8 } else { 4 },
                        if self.hard { 52. } else { 30. },
                    ),
                };
                if self.selected_player.is_none() {
                    self.choose_player(random);
                }
                let target = self
                    .selected_player
                    .and_then(|index| self.context.players[index])
                    .unwrap_or(target);
                if (self.clock_ticks + u64::from(self.update_phase)).is_multiple_of(4) {
                    if self.fleeing {
                        self.body.velocity.y += 6. / 256.;
                        self.body.velocity.x +=
                            if self.body.center().x.floor().rem_euclid(256.) >= 128. {
                                12. / 256.
                            } else {
                                -12. / 256.
                            };
                    } else {
                        self.chase_updates_left = self.chase_updates_left.saturating_sub(1);
                        let missing_target = self
                            .selected_player
                            .is_some_and(|p| self.context.players[p].is_none());
                        if (self.chase_updates_left == 0 || missing_target)
                            && self.kind != EnemyKind::Willowisp
                        {
                            self.fleeing = true;
                        } else {
                            motion::steer(
                                self.body.center(),
                                &mut self.body.velocity,
                                target,
                                acceleration,
                                maximum / 256.,
                            );
                        }
                    }
                }
                if after_movement {
                    self.body.position.x -= self.body.velocity.x;
                    self.body.position.y -= self.body.velocity.y;
                }
            }
            EnemyKind::Snake => {
                if self.state == EnemyState::Idle {
                    let aligned = self.context.players.into_iter().flatten().any(|player| {
                        let dy = self.body.center().y.floor() - player.y.floor();
                        (-4. ..=8.).contains(&dy)
                            && (player.x - self.body.center().x) * self.facing > 0.
                    });
                    if (self.clock_ticks + u64::from(self.update_phase)).is_multiple_of(4)
                        && aligned
                    {
                        self.state = EnemyState::Attacking;
                        self.animation_started = self.age;
                        self.timer = if self.context.extra_mode { 2 } else { 30 };
                    }
                } else if self.timer == 0 {
                    self.body.velocity.x =
                        self.facing * if self.hard { 255. / 256. } else { 176. / 256. };
                }
            }
            EnemyKind::Boss(BossKind::Summoning) => {
                self.body.velocity = Vec2 { x: 0., y: 0. };
                if self.timer == 0 {
                    let target = self.choose_player(random).unwrap_or(target);
                    attacks.push(EnemyAttack {
                        kind: ProjectileKind::Lightning,
                        position: Vec2 {
                            x: target.x.clamp(76., 180.),
                            y: self.context.camera_y + 112.,
                        },
                        velocity: Vec2 { x: 0., y: 0. },
                    });
                    self.timer = if self.hard { 115 } else { 180 };
                    self.state = EnemyState::Boss(BossPhase::Summon);
                }
            }
            EnemyKind::Boss(BossKind::Flying) => self.tick_flying(delta, random, &mut attacks),
            EnemyKind::Boss(BossKind::Sorcerer) => {
                self.tick_sorcerer(toward, delta, random, &mut attacks)
            }
            EnemyKind::Boss(BossKind::Inhaling) => self.tick_inhaling(random),
            EnemyKind::Boss(BossKind::Final) => self.tick_final(random, &mut attacks),
            EnemyKind::Boss(BossKind::Segmented) => {
                unreachable!("segmented boss ticks before targeting")
            }
            EnemyKind::Spiderweb => {
                self.body.velocity = Vec2 { x: 0., y: 0. };
            }
        }
        attacks
    }

    fn attack(&self, kind: ProjectileKind, velocity: Vec2) -> EnemyAttack {
        EnemyAttack {
            kind,
            position: Vec2 {
                x: self.body.position.x + self.body.size.x * 0.5,
                y: self.body.position.y + self.body.size.y * 0.5,
            },
            velocity,
        }
    }

    fn tick_flying(&mut self, delta: Vec2, random: &mut EnemyRng, attacks: &mut Vec<EnemyAttack>) {
        if self.state == EnemyState::Boss(BossPhase::Waiting) && self.timer == 80 {
            self.choose_player(random);
            self.attack_protected = true;
        }
        if self.timer != 0 {
            return;
        }
        match self.state {
            EnemyState::Boss(BossPhase::Waiting) => {
                self.shots_remaining = 3;
                self.state = EnemyState::Boss(BossPhase::Volley);
                self.emit_flying_attack(delta, random, attacks);
            }
            EnemyState::Boss(BossPhase::Volley) => {
                if self.shots_remaining != 0 {
                    self.emit_flying_attack(delta, random, attacks);
                } else {
                    self.attack_protected = false;
                    self.state = EnemyState::Boss(BossPhase::Windup);
                    self.timer = 110;
                }
            }
            EnemyState::Boss(BossPhase::Windup) => {
                self.shots_remaining = 8;
                self.state = EnemyState::Boss(BossPhase::Rain);
                self.emit_flying_attack(delta, random, attacks);
            }
            EnemyState::Boss(BossPhase::Rain) => {
                if self.shots_remaining != 0 {
                    self.emit_flying_attack(delta, random, attacks);
                } else {
                    self.state = EnemyState::Boss(BossPhase::Waiting);
                    self.timer = 240;
                }
            }
            _ => {}
        }
    }

    fn emit_flying_attack(
        &mut self,
        delta: Vec2,
        random: &mut EnemyRng,
        attacks: &mut Vec<EnemyAttack>,
    ) {
        self.shots_remaining -= 1;
        if self.state == EnemyState::Boss(BossPhase::Volley) {
            let target = self
                .selected_player
                .and_then(|index| self.context.players[index])
                .unwrap_or(Vec2 {
                    x: self.body.center().x + delta.x,
                    y: self.body.center().y + delta.y,
                });
            attacks.push(self.attack(
                ProjectileKind::Orb,
                motion::proportional_aim(self.body.center(), target, false),
            ));
            self.timer = 20;
        } else {
            let coordinate = random.byte();
            attacks.push(EnemyAttack {
                kind: ProjectileKind::Orb,
                position: Vec2 {
                    x: if coordinate < 100 {
                        248.
                    } else {
                        f32::from(coordinate)
                    },
                    y: self.context.camera_y
                        + if coordinate < 100 {
                            f32::from(coordinate)
                        } else {
                            8.
                        },
                },
                velocity: Vec2 { x: -1., y: 2. },
            });
            self.timer = 10;
        }
    }

    fn tick_sorcerer(
        &mut self,
        _toward: f32,
        _delta: Vec2,
        random: &mut EnemyRng,
        attacks: &mut Vec<EnemyAttack>,
    ) {
        let EnemyState::Boss(phase) = self.state else {
            return;
        };
        if phase != BossPhase::Chase {
            self.body.velocity = Vec2 { x: 0., y: 0. };
        }
        if phase == BossPhase::Transform && self.timer == 30 {
            self.body.position.y -= 1.;
        }
        match phase {
            BossPhase::Transform if self.timer == 0 => {
                self.state = EnemyState::Boss(BossPhase::Chase);
                self.timer = 32;
            }
            BossPhase::Chase => {
                let target = self
                    .selected_player
                    .and_then(|p| self.context.players[p])
                    .or_else(|| self.choose_player(random));
                if let Some(target) = target
                    .filter(|_| (self.clock_ticks + u64::from(self.update_phase)).is_multiple_of(4))
                {
                    motion::steer(
                        self.body.center(),
                        &mut self.body.velocity,
                        target,
                        14,
                        92. / 256.,
                    );
                }
                if self.timer == 0 {
                    if self.context.active_minions < 2 {
                        let boss_target = self.selected_player;
                        self.choose_player(random);
                        let minion_target = self.selected_player;
                        self.selected_player = boss_target;
                        self.actions.push(WorldAction::SpawnEnemy {
                            kind: EnemyKind::Minion,
                            position: self.body.center(),
                            velocity: Vec2 { x: 0., y: -2. },
                            target: minion_target,
                        });
                    }
                    self.timer = 128;
                }
            }
            BossPhase::Waiting | BossPhase::Recover if self.timer == 0 => {
                self.state = EnemyState::Boss(BossPhase::Windup);
                self.timer = 40;
            }
            BossPhase::Windup if self.timer == 0 => {
                self.shots_remaining = if self.pattern == 0 { 4 } else { 10 };
                self.state = EnemyState::Boss(if self.pattern == 0 {
                    BossPhase::Volley
                } else {
                    BossPhase::Rain
                });
                self.emit_sorcerer_attack(random, attacks);
            }
            BossPhase::Volley | BossPhase::Rain if self.timer == 0 => {
                if self.shots_remaining == 0 {
                    self.state = EnemyState::Boss(BossPhase::Recover);
                    self.timer = 30 + if self.hard { 82 } else { 130 };
                    self.pattern ^= 1;
                } else {
                    self.emit_sorcerer_attack(random, attacks);
                }
            }
            _ => {}
        }
    }

    fn emit_sorcerer_attack(&mut self, random: &mut EnemyRng, attacks: &mut Vec<EnemyAttack>) {
        self.shots_remaining -= 1;
        if self.pattern == 0 {
            self.actions.push(WorldAction::SpawnEnemy {
                kind: EnemyKind::Minion,
                position: self.body.center(),
                velocity: Vec2 { x: 0., y: -2. },
                target: Some(usize::from(self.shots_remaining)),
            });
            self.timer = 10;
        } else {
            // c62b uses the shared random byte, including LSR carry on addition.
            if self.shots_remaining == 9 {
                self.hazard_x = 128;
            }
            attacks.push(EnemyAttack {
                kind: ProjectileKind::FallingHazard,
                position: Vec2 {
                    x: f32::from(self.hazard_x),
                    y: self.context.camera_y + 208.,
                },
                velocity: Vec2 { x: 0., y: -1. },
            });
            self.hazard_x = ((u16::from(random.byte()).div_ceil(2) + u16::from(self.hazard_x))
                & 127) as u8
                + 64;
            self.timer = 16;
        }
    }

    fn approach(&mut self, point: Vec2, _speed: f32, tolerance: f32) -> bool {
        let center = self.body.center();
        if motion::manhattan(center, point) < tolerance as u16 {
            self.body.velocity = Vec2 { x: 0., y: 0. };
            true
        } else {
            self.body.velocity = motion::proportional_aim(center, point, false);
            false
        }
    }

    /// Health-gated inhale/dash pattern; movement integration belongs to the world.
    fn tick_inhaling(&mut self, random: &mut EnemyRng) {
        if self.age == 1 {
            self.actions
                .push(WorldAction::LockPlayerControls { ticks: 255 });
            return;
        }
        if self.age == 2 {
            return;
        }
        let phase = match self.health {
            0..=17 => 3,
            18..=35 => 2,
            36..=53 => 1,
            _ => 0,
        };
        if self.state == EnemyState::Boss(BossPhase::Waiting) {
            self.state = EnemyState::Boss(BossPhase::Approach);
        }
        match self.state {
            EnemyState::Boss(BossPhase::Approach) => {
                if self.approach(
                    Vec2 {
                        x: 32.,
                        y: self.context.camera_y + 64.,
                    },
                    1.,
                    2.,
                ) {
                    self.state = EnemyState::Boss(BossPhase::Flash);
                    self.timer = if self.boss_cycles == 0 {
                        80
                    } else {
                        [80, 98, 58, 48][usize::from(phase)] - if self.hard { 31 } else { 0 }
                    };
                }
            }
            EnemyState::Boss(BossPhase::Flash) if self.timer == 0 => {
                self.state = EnemyState::Boss(BossPhase::Inhale);
                if self.boss_cycles == 0 {
                    self.actions
                        .push(WorldAction::LockPlayerControls { ticks: 215 });
                }
                self.body.velocity = Vec2 { x: 1.5, y: 0. };
            }
            EnemyState::Boss(BossPhase::Recover) if self.timer == 0 => {
                self.state = EnemyState::Boss(BossPhase::Approach);
            }
            EnemyState::Boss(BossPhase::Inhale) if self.body.center().x >= 240. => {
                self.state = EnemyState::Boss(BossPhase::Dash);
                self.body.velocity = Vec2 { x: -5.5, y: 1. };
            }
            EnemyState::Boss(BossPhase::Dash) if self.body.center().x >= 17. => {
                self.body.velocity.x = (self.body.velocity.x + 8. / 256.).min(0.);
            }
            EnemyState::Boss(BossPhase::Dash) if self.body.center().x < 17. => {
                self.state = EnemyState::Boss(BossPhase::Patrol);
                self.body.velocity = Vec2 {
                    x: (f32::from(
                        [64u8, 100, 136, 176][usize::from(
                            (self.boss_health_phase + u8::from(self.boss_cycles != 0)).min(3),
                        )],
                    ) + if self.hard { 32. } else { 0. })
                        / 256.,
                    y: 0.,
                };
                self.motion_paused = true;
                self.bobbing_down = true;
                self.facing = 1.;
                if self.boss_cycles != 0 {
                    self.boss_health_phase += 1;
                }
                self.boss_cycles += 1;
                self.attack_protected = false;
                if self.boss_cycles == 1 {
                    self.break_inhale_platform(5);
                    self.break_inhale_platform(6);
                } else {
                    self.break_inhale_platform(self.boss_health_phase as usize);
                }
            }
            EnemyState::Boss(BossPhase::Patrol) => {
                if phase != self.boss_health_phase {
                    self.state = EnemyState::Boss(BossPhase::Recover);
                    self.timer = 30;
                    self.body.velocity = Vec2 { x: 0., y: 0. };
                    self.attack_protected = true;
                } else {
                    let speed = f32::from(
                        [64u8, 100, 136, 176][usize::from(self.boss_health_phase.min(3))],
                    ) + if self.hard { 32. } else { 0. };
                    self.body.velocity.x = self.facing * speed / 256.;
                    self.body.velocity.y += if self.bobbing_down {
                        8. / 256.
                    } else {
                        -8. / 256.
                    };
                    if self.body.velocity.y >= 336. / 256. {
                        self.bobbing_down = false;
                    }
                    if self.body.velocity.y < -336. / 256. {
                        self.bobbing_down = true;
                    }
                    if self.body.center().y.floor() < self.context.camera_y + 104. {
                        self.body.position.y += 1.;
                    }
                    if !self.clock_ticks.is_multiple_of(2) && self.body.center().x.floor() >= 246. {
                        self.facing = -1.;
                    }
                    if !self.clock_ticks.is_multiple_of(2) && self.body.center().x.floor() < 17. {
                        self.facing = 1.;
                    }
                }
            }
            _ => {}
        }
        if self.state == EnemyState::Boss(BossPhase::Inhale) {
            self.actions.push(WorldAction::PullPlayersBelow {
                y: self.body.center().y,
                velocity_y: -1.0078125,
            });
            if self.clock_ticks.is_multiple_of(4) {
                self.actions.push(WorldAction::Particle {
                    position: Vec2 {
                        x: f32::from(random.byte()),
                        y: self.context.camera_y + 166.,
                    },
                    velocity: Vec2 { x: 0., y: -3. },
                });
            }
        }
        if self.state == EnemyState::Boss(BossPhase::Patrol)
            && self.hard
            && self.clock_ticks.is_multiple_of(128)
        {
            let maximum = if self.context.extra_mode { 3 } else { 1 };
            if self.context.active_chasers < maximum {
                self.actions.push(WorldAction::SpawnEnemy {
                    kind: EnemyKind::Chaser,
                    position: self.body.center(),
                    velocity: Vec2 { x: 0., y: 0. },
                    target: self.selected_player,
                });
            }
        }
    }

    fn break_inhale_platform(&mut self, index: usize) {
        const CELLS: [usize; 7] = [184, 137, 140, 134, 131, 135, 136];
        let cell = CELLS[index.min(6)];
        let x = (cell % 16) * 2;
        let y = (self.context.camera_y / 8.).floor().max(0.) as usize + (cell / 16) * 2;
        for dy in 0..2 {
            for dx in 0..2 {
                self.actions.push(WorldAction::BreakTile {
                    x: x + dx,
                    y: y + dy,
                });
            }
        }
    }

    fn begin_final_aim(&mut self, random: &mut EnemyRng) {
        self.choose_player(random);
        self.attack_protected = true;
        self.final_action = FinalAction::Aim;
        self.timer = 80;
        self.state = EnemyState::Boss(BossPhase::Windup);
    }

    fn tick_final(&mut self, random: &mut EnemyRng, attacks: &mut Vec<EnemyAttack>) {
        let target_point = |y| Vec2 {
            x: 200.,
            y: self.context.camera_y + y,
        };
        match self.final_action {
            FinalAction::MoveCenter => {
                self.navigation_target = Some(target_point(128.));
                self.state = EnemyState::Boss(BossPhase::Approach);
                if motion::manhattan(self.body.center(), target_point(128.)) < 24 {
                    self.final_action = FinalAction::CenterPause;
                    self.timer = 80;
                }
            }
            FinalAction::CenterPause if self.timer == 0 => {
                if self.boss_cycles >= 2 {
                    self.begin_final_dash();
                } else {
                    self.begin_final_aim(random);
                }
            }
            FinalAction::Aim if self.timer == 0 => {
                self.final_action = FinalAction::TripleShot;
                self.shots_remaining = 3;
                self.emit_final_aimed(attacks);
            }
            FinalAction::TripleShot if self.timer == 0 => {
                if self.shots_remaining == 0 {
                    self.attack_protected = false;
                    self.final_action = FinalAction::AfterShots;
                    self.timer = 60;
                } else {
                    self.emit_final_aimed(attacks);
                }
            }
            FinalAction::AfterShots if self.timer == 0 => {
                self.final_action = FinalAction::MoveLower;
                self.navigation_target = Some(target_point(176.));
            }
            FinalAction::MoveLower => {
                if motion::manhattan(self.body.center(), target_point(176.)) < 24 {
                    self.final_action = FinalAction::LowerPause;
                    self.timer = 60;
                }
            }
            FinalAction::LowerPause if self.timer == 0 => {
                self.radial_orbs(attacks);
                if self.boss_cycles >= 2 {
                    self.final_action = FinalAction::MoveMiddle;
                    self.navigation_target = Some(target_point(128.));
                } else {
                    self.final_action = FinalAction::UpperPause;
                    self.navigation_target = Some(target_point(80.));
                    self.timer = 70;
                }
            }
            FinalAction::MoveMiddle => {
                if motion::manhattan(self.body.center(), target_point(128.)) < 24 {
                    self.final_action = FinalAction::MiddlePause;
                    self.timer = 60;
                }
            }
            FinalAction::MiddlePause if self.timer == 0 => {
                self.radial_orbs(attacks);
                self.final_action = FinalAction::UpperPause;
                self.navigation_target = Some(target_point(80.));
                self.timer = 70;
            }
            FinalAction::UpperPause if self.timer == 0 => {
                self.radial_orbs(attacks);
                self.boss_cycles += 1;
                if self.boss_cycles == 1 {
                    self.begin_final_aim(random);
                } else if self.boss_cycles == 2 {
                    self.final_action = FinalAction::MoveCenter;
                } else {
                    self.begin_final_dash();
                }
            }
            FinalAction::DashCharge if self.timer == 0 => {
                self.final_action = FinalAction::Dash;
                self.timer = 50;
                self.state = EnemyState::Boss(BossPhase::Dash);
                self.body.velocity.x = -5.023_437_5;
            }
            FinalAction::Dash if self.timer != 0 => {
                self.body.velocity.x = (self.body.velocity.x + 12. / 256.).min(0.);
            }
            FinalAction::Dash if self.timer == 0 => {
                self.body.velocity.x = (self.body.velocity.x + 12. / 256.).min(0.);
                self.final_action = FinalAction::DashRecovery;
                self.timer = 60;
                self.state = EnemyState::Boss(BossPhase::Recover);
                self.navigation_target =
                    Some(target_point(if self.boss_cycles == 2 { 128. } else { 80. }));
            }
            FinalAction::DashRecovery if self.timer == 0 => self.begin_final_aim(random),
            _ => {}
        }
        let center = self.body.center();
        if center.y < self.context.camera_y + 20. {
            self.body.position.y += 1.;
        }
        if center.y >= self.context.camera_y + 220. {
            self.body.position.y -= 1.;
        }
        if let Some(target) = self.navigation_target {
            if motion::manhattan(self.body.center(), target) < 24 {
                motion::friction(&mut self.body.velocity, 8);
            } else {
                motion::steer(self.body.center(), &mut self.body.velocity, target, 32, 2.);
            }
        }
    }

    fn begin_final_dash(&mut self) {
        self.final_action = FinalAction::DashCharge;
        self.timer = 40;
        self.state = EnemyState::Boss(BossPhase::Charge);
        self.navigation_target = None;
        self.body.velocity.x = 216. / 256.;
    }

    fn emit_final_aimed(&mut self, attacks: &mut Vec<EnemyAttack>) {
        if let Some(target) = self
            .selected_player
            .and_then(|index| self.context.players[index])
        {
            attacks.push(self.attack(
                ProjectileKind::Orb,
                motion::proportional_aim(self.body.center(), target, false),
            ));
        }
        self.shots_remaining -= 1;
        self.timer = 20;
    }

    fn radial_orbs(&self, attacks: &mut Vec<EnemyAttack>) {
        // Decoded source radial vector table: seven signed 8.8 velocities.
        const VELOCITIES: [(f32, f32); 7] = [
            (-0.75, -0.03125),
            (-0.46875, 0.5625),
            (0.15625, 0.71875),
            (0.65625, 0.3125),
            (0.65625, -0.34375),
            (0.15625, -0.75),
            (-0.46875, -0.59375),
        ];
        for (x, y) in VELOCITIES {
            attacks.push(self.attack(ProjectileKind::Orb, Vec2 { x, y }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn origin() -> Vec2 {
        Vec2 { x: 0., y: 0. }
    }
    #[test]
    fn stationary_enemies_fire_in_source_facing_with_original_muzzle_and_speed() {
        let rows: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/stationary_shot_reference.json"
        ))
        .unwrap();
        for row in rows.as_array().unwrap() {
            let hard = row["hard"].as_bool().unwrap();
            let left = row["left"].as_bool().unwrap();
            let kind = if row["kind"].as_u64().unwrap() == 7 {
                EnemyKind::Goat
            } else {
                EnemyKind::Troll
            };
            let mut enemy = Enemy::new(kind, Vec2::new(124., 68.), hard);
            enemy.facing = if left { -1. } else { 1. };
            // The player is deliberately behind the actor, proving no auto-aim.
            let target = Some(Vec2::new(if left { 200. } else { 40. }, 200.));
            for tick in 1..=row["tick"].as_u64().unwrap() {
                let attacks = enemy.tick(target);
                assert_eq!(enemy.facing, if left { -1. } else { 1. });
                if tick < row["tick"].as_u64().unwrap() {
                    assert!(attacks.is_empty());
                    continue;
                }
                assert_eq!(attacks.len(), 1);
                let shot = crate::projectile::Projectile::hostile_with_difficulty(attacks[0], hard);
                assert_eq!(shot.body.center().x, row["x"].as_f64().unwrap() as f32);
                assert_eq!(shot.body.center().y, row["y"].as_f64().unwrap() as f32);
                assert_eq!(
                    (shot.body.velocity.x * 256.) as i64,
                    row["vx"].as_i64().unwrap()
                );
                assert_eq!(
                    (shot.body.velocity.y * 256.) as i64,
                    row["vy"].as_i64().unwrap()
                );
            }
        }
    }

    #[test]
    fn projectile_callback_responses_match_original_dispatch_matrix() {
        let rows: Vec<[u16; 9]> = serde_json::from_str(include_str!(
            "../tests/fixtures/hit_response_reference.json"
        ))
        .unwrap();
        for [
            kind,
            armed,
            damage,
            active,
            protected,
            invul,
            response,
            new_state,
            _,
        ] in rows
        {
            let kind = match kind {
                1 => EnemyKind::Boss(BossKind::Inhaling),
                2 => EnemyKind::Boss(BossKind::Summoning),
                3 => EnemyKind::Boss(BossKind::Flying),
                6 => EnemyKind::Boss(BossKind::Final),
                8 => EnemyKind::Boss(BossKind::Sorcerer),
                5 => EnemyKind::Turret,
                7 => EnemyKind::Goat,
                9 => EnemyKind::Willowisp,
                10 => EnemyKind::Bomb,
                11 => EnemyKind::Troll,
                12 => EnemyKind::Snake,
                13 => EnemyKind::Chaser,
                14 => EnemyKind::Walker,
                16 => EnemyKind::SummonedWalker,
                18 => EnemyKind::DiagonalFlyer,
                20 => EnemyKind::Hopper,
                22 => EnemyKind::Minion,
                24 => EnemyKind::Leaper,
                25 => EnemyKind::Spike,
                27 => EnemyKind::Lightning,
                29 => EnemyKind::Spiderweb,
                36 => EnemyKind::SummoningPlatform,
                _ => unreachable!(),
            };
            let mut enemy = Enemy::new(kind, origin(), false);
            enemy.health = if matches!(kind, EnemyKind::Boss(_)) {
                100
            } else {
                4
            };
            if matches!(kind, EnemyKind::Boss(_)) {
                enemy.arena_waiting = active == 0;
                enemy.attack_protected = protected != 0;
                enemy.invulnerable_ticks = invul;
            }
            if kind == EnemyKind::Bomb && armed != 0 {
                enemy.state = EnemyState::Fuse;
            }
            if kind == EnemyKind::DiagonalFlyer {
                enemy.state = if armed == 0 {
                    EnemyState::Idle
                } else {
                    EnemyState::Attacking
                };
            }
            let expected =
                [HitResponse::Apply, HitResponse::Absorb, HitResponse::Pass][response as usize];
            assert_eq!(
                enemy.hit_response(damage),
                expected,
                "{kind:?} state={armed} damage={damage} active={active} protected={protected} invul={invul}"
            );
            if kind == EnemyKind::Bomb {
                enemy.hit(damage);
                assert_eq!(enemy.state == EnemyState::Fuse, new_state != 0);
                assert_eq!(enemy.health, if response == 0 { 4 - damage } else { 4 });
            }
            if kind == EnemyKind::DiagonalFlyer && damage < 4 {
                enemy.hit(damage);
                assert_eq!(enemy.state == EnemyState::Attacking, new_state != 0);
            }
        }
    }

    #[test]
    fn all_ordinary_enemy_health_matches_original_initialization() {
        let rows: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/ordinary_initialization_reference.json"
        ))
        .unwrap();
        for row in rows.as_array().unwrap() {
            let kind = match row["kind"].as_u64().unwrap() {
                5 => EnemyKind::Turret,
                7 => EnemyKind::Goat,
                9 => EnemyKind::Willowisp,
                10 => EnemyKind::Bomb,
                11 => EnemyKind::Troll,
                12 => EnemyKind::Snake,
                13 | 19 => EnemyKind::Chaser,
                14 => EnemyKind::Walker,
                16 => EnemyKind::SummonedWalker,
                18 => EnemyKind::DiagonalFlyer,
                20 => EnemyKind::Hopper,
                22 => EnemyKind::Minion,
                24 => EnemyKind::Leaper,
                25 => EnemyKind::Spike,
                27 => EnemyKind::Lightning,
                29 => EnemyKind::Spiderweb,
                36 => EnemyKind::SummoningPlatform,
                other => panic!("unmapped type {other}"),
            };
            let enemy = Enemy::new(kind, origin(), row["hard"].as_bool().unwrap());
            let health = row["health"].as_u64().unwrap() as u16;
            if health == 0 {
                // Source hazards have no damage health, while native active
                // hazards use a positive lifetime sentinel and reject hits.
                assert!(enemy.is_alive());
                assert!(!enemy.can_accept_hit());
            } else {
                assert_eq!(enemy.health, health, "{kind:?}, hard={}", enemy.hard);
            }
        }
    }

    #[test]
    fn willowisp_nonzero_hits_restart_source_hurt_animation() {
        let mut enemy = Enemy::new(EnemyKind::Willowisp, origin(), false);
        enemy.age = 40;
        assert_eq!(enemy.animation(), Some(("willowisp_idle", 40)));
        assert!(!enemy.hit(0));
        assert_eq!(enemy.animation(), Some(("willowisp_idle", 40)));
        assert!(!enemy.hit(2));
        assert_eq!(enemy.animation(), Some(("willowisp_hurt", 0)));
        enemy.age += 5;
        assert_eq!(enemy.animation(), Some(("willowisp_hurt", 5)));
        assert!(!enemy.hit(2));
        assert_eq!(enemy.animation(), Some(("willowisp_hurt", 0)));
    }

    #[test]
    fn willowisp_health_decay_matches_source_and_requires_final_hit() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/willow_health_reference.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let mut enemy = Enemy::new(
                EnemyKind::Willowisp,
                Vec2::new(124., 68.),
                case["hard"].as_bool().unwrap(),
            );
            enemy.health = case["health"].as_u64().unwrap() as u16;
            let mut rng = EnemyRng::new(0);
            for row in case["frames"].as_array().unwrap() {
                let tick = row[0].as_u64().unwrap();
                enemy.tick_with_context(
                    &EnemyContext::single(Some(Vec2::new(200., 200.)), tick),
                    &mut rng,
                );
                assert_eq!(
                    u64::from(enemy.health),
                    row[1].as_u64().unwrap(),
                    "tick {tick}"
                );
                assert_eq!(enemy.is_alive(), row[2].as_u64().unwrap() != 0);
                assert!(enemy.visible() && enemy.contact_enabled());
            }
            if enemy.health == 0 {
                assert!(enemy.hit(2));
                assert!(!enemy.is_alive());
                assert!(!enemy.hit(2));
            }
        }
    }

    #[test]
    fn ordinary_damage_has_palette_feedback_without_invented_hit_immunity() {
        // Original 8463 called three times without a frame update, type0e:
        // health10 ->8 ->6 ->4, effect03b4=10 after each call.
        let mut enemy = Enemy::new(EnemyKind::Walker, Vec2::new(120., 92.), false);
        enemy.health = 10;
        for health in [8, 6, 4] {
            assert!(!enemy.hit(2));
            assert_eq!(enemy.health, health);
            assert_eq!(enemy.palette_effect_ticks, 10);
            assert!(enemy.can_accept_hit());
        }
        let mut boss = Enemy::new(
            EnemyKind::Boss(BossKind::Inhaling),
            Vec2::new(120., 92.),
            false,
        );
        boss.hit(2);
        let health = boss.health;
        boss.hit(2);
        assert_eq!(
            boss.health, health,
            "source d411 still protects boss hits for20ticks"
        );
    }

    #[test]
    fn source_immune_hazards_reject_damage_and_ghost_freeze() {
        for kind in [
            EnemyKind::Spike,
            EnemyKind::Lightning,
            EnemyKind::Spiderweb,
            EnemyKind::SummoningPlatform,
        ] {
            let mut enemy = Enemy::new(kind, Vec2::new(120., 92.), false);
            let health = enemy.health;
            assert!(!enemy.can_accept_hit());
            assert!(!enemy.can_freeze());
            enemy.hit(100);
            enemy.freeze(190);
            assert_eq!(enemy.health, health);
            assert_eq!(enemy.frozen_ticks, 0);
        }
        let diagonal = Enemy::new(EnemyKind::DiagonalFlyer, Vec2::new(0., 0.), false);
        let leaper = Enemy::new(EnemyKind::Leaper, Vec2::new(0., 0.), false);
        assert_eq!(diagonal.score_units(), 1);
        assert_eq!(leaper.score_units(), 2);
    }

    #[test]
    fn thaw_does_not_halve_independent_modulo_three_cleanup_clock() {
        // a3ae halves $0c for thawed steering/patrol, but 8221 still reads
        // the independent $0d modulo-three scheduler and the slot's low2bits.
        let mut enemy = Enemy::new(EnemyKind::Chaser, Vec2::new(-20., 80.), false);
        enemy.update_phase = 1;
        enemy.freeze(104);
        let mut rng = EnemyRng::new(0);
        enemy.tick_with_context(
            &EnemyContext::single(Some(Vec2::new(128., 80.)), 4),
            &mut rng,
        );
        assert!(!enemy.is_stunned());
        assert!(!enemy.remove_if_outside_viewport(0.));
        enemy.tick_with_context(
            &EnemyContext::single(Some(Vec2::new(128., 80.)), 6),
            &mut rng,
        );
        assert!(enemy.remove_if_outside_viewport(0.));
    }

    #[test]
    fn freeze_palette_and_thaw_flash_match_original_frames() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/freeze_palette_reference.json"
        ))
        .unwrap();
        let mut enemy = Enemy::new(EnemyKind::Walker, Vec2::new(120., 92.), false);
        enemy.freeze(190);
        for row in fixture["rows"].as_array().unwrap() {
            enemy.frozen_ticks = enemy.frozen_ticks.saturating_sub(1);
            enemy.update_palette_effect();
            assert_eq!(enemy.frozen_ticks as u64, row["freeze"].as_u64().unwrap());
            assert_eq!(
                enemy.palette_effect_ticks as u64,
                row["effect"].as_u64().unwrap(),
                "effect at {}",
                row["tick"]
            );
            assert_eq!(
                enemy.display_palette(2) as u64,
                row["palette"].as_u64().unwrap(),
                "palette at {}",
                row["tick"]
            );
        }
    }

    #[test]
    fn hard_walker_patrol_pauses_and_shots_match_source() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/walker_motion_reference.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let mut enemy = Enemy::new(
                EnemyKind::Walker,
                Vec2::new(124., 68.),
                case["hard"].as_bool().unwrap(),
            );
            let mut rng = EnemyRng::new(0);
            for row in case["frames"].as_array().unwrap() {
                let tick = row[0].as_u64().unwrap();
                let result = enemy.tick_with_context(
                    &EnemyContext::single(Some(Vec2::new(148., 72.)), tick),
                    &mut rng,
                );
                enemy.body.position.x += enemy.body.velocity.x;
                assert_eq!(
                    enemy.body.center().x as u64,
                    row[1].as_u64().unwrap(),
                    "x tick {tick}"
                );
                assert_eq!(
                    enemy.state == EnemyState::Attacking,
                    row[2].as_u64().unwrap() != 0,
                    "state tick {tick}"
                );
                assert_eq!(
                    result.attacks.len() as u64,
                    row[5].as_u64().unwrap(),
                    "shots tick {tick}"
                );
            }
        }
    }

    #[test]
    fn diagonal_flyer_awaken_and_motion_match_original_execution() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/diagonal_flyer_reference.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let mut enemy = Enemy::new(
                EnemyKind::DiagonalFlyer,
                origin(),
                case["hard"].as_bool().unwrap(),
            );
            enemy.body.position =
                Vec2::new(128. - enemy.body.size.x / 2., 72. - enemy.body.size.y / 2.);
            let mut rng = EnemyRng::new(0);
            for row in case["frames"].as_array().unwrap() {
                let tick = row[0].as_u64().unwrap();
                enemy.tick_with_context(
                    &EnemyContext::single(Some(Vec2::new(148., 92.)), tick),
                    &mut rng,
                );
                if !enemy.motion_paused() {
                    enemy.body.position.x += enemy.body.velocity.x;
                    enemy.body.position.y += enemy.body.velocity.y;
                }
                assert_eq!(
                    (enemy.body.center().x * 256.) as u64,
                    row[1].as_u64().unwrap(),
                    "x tick {tick}"
                );
                assert_eq!(
                    (enemy.body.center().y * 256.) as u64,
                    row[2].as_u64().unwrap(),
                    "y tick {tick}"
                );
                assert_eq!(
                    (enemy.body.velocity.x * 256.) as u64,
                    row[5].as_u64().unwrap(),
                    "vx tick {tick}"
                );
                assert_eq!(
                    (enemy.body.velocity.y * 256.) as u64,
                    row[6].as_u64().unwrap(),
                    "vy tick {tick}"
                );
            }
        }
    }

    #[test]
    fn ground_patrol_cadence_matches_source_for_modes_slots_and_thaw() {
        #[derive(serde::Deserialize)]
        struct Case {
            mode: u8,
            slot: u8,
            kind: u8,
            thaw: bool,
            frames: Vec<(u64, i32, u8)>,
        }
        let cases: Vec<Case> = serde_json::from_str(include_str!(
            "../tests/fixtures/ground_cadence_reference.json"
        ))
        .unwrap();
        for case in cases {
            for direction in [-1., 1.] {
                let kind = if case.kind == 14 {
                    EnemyKind::Walker
                } else {
                    EnemyKind::Bomb
                };
                let mut enemy = Enemy::new(kind, Vec2::new(128., 80.), case.mode & 128 != 0);
                enemy.facing = direction;
                enemy.update_phase = case.slot;
                if case.thaw {
                    enemy.freeze(104);
                }
                let mut rng = EnemyRng::new(0);
                for (tick, x, moved) in case.frames.iter().copied() {
                    // Extra Walker attacks after 17 steps; this fixture isolates
                    // patrol cadence. Its attack lifecycle has a separate trace.
                    if case.mode & 64 != 0 && case.kind == 14 && tick > 32 {
                        break;
                    }
                    let mut context = EnemyContext::single(Some(Vec2::new(200., 100.)), tick);
                    context.extra_mode = case.mode & 64 != 0;
                    enemy.tick_with_context(&context, &mut rng);
                    enemy.body.position.x += enemy.body.velocity.x;
                    assert_eq!(
                        enemy.body.velocity.x,
                        direction * f32::from(moved),
                        "kind {} mode {} slot {} thaw {} tick {}",
                        case.kind,
                        case.mode,
                        case.slot,
                        case.thaw,
                        tick
                    );
                    assert_eq!(enemy.body.position.x, 128. + direction * (x - 128) as f32);
                }
            }
        }
    }

    #[test]
    fn hard_inhale_caps_chasers_instead_of_sorcerer_minions() {
        for (extra, chasers, expected) in [
            (false, 0, true),
            (false, 1, false),
            (true, 2, true),
            (true, 3, false),
        ] {
            let mut boss = Enemy::new(
                EnemyKind::Boss(BossKind::Inhaling),
                Vec2::new(48., 104.),
                true,
            );
            boss.age = 1000;
            boss.state = EnemyState::Boss(BossPhase::Patrol);
            let mut context = EnemyContext::single(Some(origin()), 128);
            context.active_minions = 99;
            context.active_chasers = chasers;
            context.extra_mode = extra;
            let frame = boss.tick_with_context(&context, &mut EnemyRng::new(0));
            assert_eq!(
                frame.actions.iter().any(|a| matches!(
                    a,
                    WorldAction::SpawnEnemy {
                        kind: EnemyKind::Chaser,
                        ..
                    }
                )),
                expected
            );
        }
    }

    #[test]
    fn complete_boss_motion_matches_reference_with_moving_players() {
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../tests/fixtures/boss_motion_lifecycles.json"
        ))
        .unwrap();
        for case in cases {
            let kind = match case["kind"].as_u64().unwrap() {
                1 => BossKind::Inhaling,
                8 => BossKind::Sorcerer,
                _ => BossKind::Final,
            };
            let hard = case["hard"].as_bool().unwrap();
            let player_count = case["players"].as_u64().unwrap() as usize;
            let mut enemy = Enemy::new(EnemyKind::Boss(kind), Vec2 { x: 112., y: 56. }, hard);
            if kind == BossKind::Sorcerer {
                enemy.body.position.x = 120.;
                enemy.hit(70);
            }
            let mut random = EnemyRng::new(case["seed"].as_u64().unwrap_or(0) as u16);
            for row in case["frames"].as_array().unwrap() {
                let tick = row[0].as_u64().unwrap();
                if case["damage"].as_bool() == Some(true) {
                    match tick {
                        500 => enemy.health = 53,
                        1500 => enemy.health = 35,
                        2500 => enemy.health = 17,
                        _ => {}
                    }
                }
                let context = EnemyContext {
                    players: std::array::from_fn(|p| {
                        (p < player_count).then_some(Vec2 {
                            x: (64 + (tick / 20 + p as u64 * 40) % 128) as f32,
                            y: (128 + (tick / 60 + p as u64 * 12) % 64) as f32,
                        })
                    }),
                    camera_y: 0.,
                    frame_index: tick,
                    active_minions: 0,
                    active_chasers: 0,
                    extra_mode: false,
                    nearby_shots: [None; 12],
                    at_ledge: false,
                    turret_active: false,
                    turret_period: 216,
                };
                enemy.tick_with_context(&context, &mut random);
                if !enemy.motion_paused() {
                    enemy.body.position.x += enemy.body.velocity.x;
                    enemy.body.position.y += enemy.body.velocity.y;
                }
                enemy.body.position.x =
                    enemy.body.center().x.rem_euclid(256.) - enemy.body.size.x * 0.5;
                enemy.body.position.y =
                    enemy.body.center().y.rem_euclid(256.) - enemy.body.size.y * 0.5;
                let actual = [
                    enemy.body.center().x,
                    enemy.body.center().y,
                    enemy.body.velocity.x,
                    enemy.body.velocity.y,
                ]
                .map(|v| (v * 256.) as i64);
                let expected: [i64; 4] = std::array::from_fn(|i| row[i + 1].as_i64().unwrap());
                assert_eq!(
                    actual, expected,
                    "{kind:?} hard={hard} players={player_count} tick={tick}"
                );
            }
        }
    }

    #[test]
    fn troll_animation_and_shots_match_original_for_both_difficulties() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/troll_animation_reference.json"
        ))
        .unwrap();
        for hard in [false, true] {
            let mut enemy = Enemy::new(EnemyKind::Troll, origin(), hard);
            for row in cases[hard.to_string()].as_array().unwrap() {
                let attacks = enemy.tick(Some(Vec2 { x: 100., y: 0. }));
                assert_eq!(
                    enemy.animation_frame as u64,
                    row[1].as_u64().unwrap(),
                    "hard={hard} tick={}",
                    row[0]
                );
                assert_eq!(
                    attacks.len() as u64,
                    row[3].as_u64().unwrap(),
                    "hard={hard} tick={}",
                    row[0]
                );
            }
        }
    }

    #[test]
    fn chaser_and_minion_motion_matches_source_through_escape() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/chasing_enemy_reference.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let kind = if case["kind"] == 22 {
                EnemyKind::Minion
            } else {
                EnemyKind::Chaser
            };
            let mut enemy = Enemy::new(
                kind,
                Vec2 { x: 124., y: 68. },
                case["hard"].as_bool().unwrap(),
            );
            enemy.set_target(Some(case["target"].as_u64().unwrap_or(0) as usize));
            if kind == EnemyKind::Minion {
                enemy.body.velocity.y = -2.;
            }
            for row in case["frames"].as_array().unwrap() {
                let tick = row[0].as_u64().unwrap();
                enemy.tick(Some(Vec2 {
                    x: (64 + (tick / 20) % 128) as f32,
                    y: (128 + (tick / 60) % 64) as f32,
                }));
                enemy.body.position.x += enemy.body.velocity.x;
                enemy.body.position.y += enemy.body.velocity.y;
                enemy.body.position.x = enemy.body.center().x.rem_euclid(256.) - 4.;
                enemy.body.position.y = enemy.body.center().y.rem_euclid(256.) - 4.;
                let actual = [
                    enemy.body.center().x,
                    enemy.body.center().y,
                    enemy.body.velocity.x,
                    enemy.body.velocity.y,
                ]
                .map(|v| (v * 256.) as i64);
                let expected: [i64; 4] = std::array::from_fn(|i| row[i + 1].as_i64().unwrap());
                assert_eq!(
                    actual, expected,
                    "kind={kind:?} hard={} tick={tick}",
                    case["hard"]
                );
            }
        }
    }

    #[test]
    fn turret_lifecycle_matches_source_every_frame() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/turret_reference.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let mut enemy = Enemy::new(
                EnemyKind::Turret,
                Vec2 { x: 40., y: 92. },
                case["left"].as_bool().unwrap(),
            );
            enemy.facing = if case["left"].as_bool().unwrap() {
                -1.
            } else {
                1.
            };
            let mut random = EnemyRng::new(0);
            for row in case["rows"].as_array().unwrap().iter().skip(1) {
                let tick = row["tick"].as_u64().unwrap();
                let mut context = EnemyContext::single(Some(origin()), tick);
                context.turret_active = tick >= 16;
                context.turret_period = case["repeat_delay"].as_u64().unwrap() as u16;
                let output = enemy.tick_with_context(&context, &mut random);
                assert_eq!(
                    enemy.body.center(),
                    Vec2 {
                        x: row["x"].as_f64().unwrap() as f32,
                        y: row["y"].as_f64().unwrap() as f32
                    },
                    "tick={tick}"
                );
                assert_eq!(
                    enemy.timer as u64,
                    row["timer"].as_u64().unwrap(),
                    "tick={tick}"
                );
                assert_eq!(
                    output.actions.iter().any(|a| matches!(
                        a,
                        WorldAction::SpawnEnemy {
                            kind: EnemyKind::SummonedWalker,
                            ..
                        }
                    )),
                    !row["shot"].is_null(),
                    "tick={tick}"
                );
            }
        }
    }

    #[test]
    fn lightning_contact_and_chain_match_source_every_frame() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/lightning_reference.json"))
                .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let boss = case["boss"].as_bool().unwrap();
            let mut enemy = Enemy::new(
                EnemyKind::Lightning,
                Vec2 { x: 124., y: 60. },
                case["hard"].as_bool().unwrap(),
            );
            if boss {
                enemy.start_lightning_warning(true);
            }
            for row in case["rows"].as_array().unwrap() {
                enemy.tick(Some(origin()));
                let hit_mask =
                    [52., 68., 76., 60.]
                        .into_iter()
                        .enumerate()
                        .fold(0u64, |mask, (i, y)| {
                            let body = Body {
                                position: Vec2 { x: 124., y: y - 4. },
                                velocity: origin(),
                                size: Vec2 { x: 8., y: 8. },
                                grounded: false,
                            };
                            mask | if enemy.contact_enabled() && enemy.overlaps(&body) {
                                1 << i
                            } else {
                                0
                            }
                        });
                assert_eq!(
                    hit_mask,
                    row["hit_mask"].as_u64().unwrap(),
                    "boss={boss} hard={} tick={}",
                    case["hard"],
                    row["tick"]
                );
                assert_eq!(
                    enemy.bodies().len(),
                    row["segments"].as_array().unwrap().len(),
                    "tick={}",
                    row["tick"]
                );
                if enemy.health != 0 {
                    assert_eq!(
                        enemy.timer as u64,
                        row["segments"][0]["timer"].as_u64().unwrap(),
                        "tick={}",
                        row["tick"]
                    );
                }
            }
        }
    }

    #[test]
    fn established_inhale_motion_matches_original_positions() {
        #[derive(serde::Deserialize)]
        struct Sample {
            tick: u64,
            x_units: u16,
            y_units: u16,
        }
        let samples: Vec<Sample> = serde_json::from_str(include_str!(
            "../tests/fixtures/inhaling_boss_reference.json"
        ))
        .unwrap();
        let mut e = Enemy::new(
            EnemyKind::Boss(BossKind::Inhaling),
            Vec2 { x: 16., y: 48. },
            false,
        );
        e.state = EnemyState::Boss(BossPhase::Inhale);
        e.body.velocity = Vec2 { x: 1.5, y: 0. };
        let mut tick = 1; // Fixture begins immediately after original inhale setup.
        for sample in samples {
            while tick < sample.tick {
                e.tick(Some(origin()));
                e.body.position.x += e.body.velocity.x;
                e.body.position.y += e.body.velocity.y;
                tick += 1;
            }
            assert_eq!((e.body.center().x * 256.) as u16, sample.x_units);
            assert_eq!((e.body.center().y * 256.) as u16, sample.y_units);
        }
    }

    #[test]
    fn final_dash_velocity_matches_original_charge_and_deceleration() {
        #[derive(serde::Deserialize)]
        struct Sample {
            tick: u64,
            vx_units: i16,
        }
        let samples: Vec<Sample> = serde_json::from_str(include_str!(
            "../tests/fixtures/final_boss_dash_reference.json"
        ))
        .unwrap();
        let mut e = Enemy::new(EnemyKind::Boss(BossKind::Final), origin(), false);
        e.final_action = FinalAction::DashCharge;
        e.timer = 41;
        e.body.velocity.x = 216. / 256.;
        let mut tick = 0;
        for sample in samples {
            while tick < sample.tick {
                e.tick(Some(origin()));
                tick += 1;
            }
            assert_eq!(
                (e.body.velocity.x * 256.) as i16,
                sample.vx_units,
                "tick {}",
                sample.tick
            );
        }
    }

    #[test]
    fn summoning_and_flying_boss_emissions_match_original() {
        #[derive(serde::Deserialize)]
        struct Sample {
            tick: u64,
            kind: u8,
        }
        for (kind, fixture) in [
            (
                BossKind::Summoning,
                include_str!("../tests/fixtures/summoning_boss_reference.json"),
            ),
            (
                BossKind::Flying,
                include_str!("../tests/fixtures/flying_boss_reference.json"),
            ),
        ] {
            let samples: Vec<Sample> = serde_json::from_str(fixture).unwrap();
            let mut e = Enemy::new(EnemyKind::Boss(kind), origin(), false);
            let mut actual = Vec::new();
            for tick in 1..=1800 {
                for shot in e.tick(Some(Vec2 { x: 128., y: 160. })) {
                    actual.push((
                        tick,
                        match shot.kind {
                            ProjectileKind::Lightning => 27,
                            ProjectileKind::Orb => 41,
                            _ => 0,
                        },
                    ));
                }
            }
            assert_eq!(
                actual,
                samples
                    .into_iter()
                    .map(|s| (s.tick, s.kind))
                    .collect::<Vec<_>>(),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn sorcerer_attack_schedule_matches_original_through_two_patterns() {
        #[derive(serde::Deserialize)]
        struct Sample {
            tick: u64,
            kind: u8,
        }
        let samples: Vec<Sample> = serde_json::from_str(include_str!(
            "../tests/fixtures/sorcerer_boss_reference.json"
        ))
        .unwrap();
        let mut e = Enemy::new(EnemyKind::Boss(BossKind::Sorcerer), origin(), false);
        let mut actual = Vec::new();
        let mut random = EnemyRng::new(0);
        for tick in 1..=1000 {
            let frame =
                e.tick_with_context(&EnemyContext::single(Some(origin()), tick), &mut random);
            for shot in frame.attacks {
                actual.push((
                    tick,
                    if shot.kind == ProjectileKind::FallingHazard {
                        30
                    } else {
                        0
                    },
                ));
            }
            for action in frame.actions {
                if let WorldAction::SpawnEnemy {
                    kind: EnemyKind::Minion,
                    ..
                } = action
                {
                    actual.push((tick, 22));
                }
            }
        }
        assert_eq!(
            actual,
            samples
                .into_iter()
                .map(|s| (s.tick, s.kind))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn sorcerer_health_gates_end_normal_and_transform_hard_mode() {
        for hard in [false, true] {
            let mut e = Enemy::new(EnemyKind::Boss(BossKind::Sorcerer), origin(), hard);
            assert!(!e.hit(46));
            assert_eq!(e.boss_health_phase, 1);
            for _ in 0..20 {
                e.tick(None);
            }
            assert_eq!(e.hit(24), !hard);
            if hard {
                assert_eq!(e.state, EnemyState::Boss(BossPhase::Transform));
                let hp = e.health;
                assert!(!e.hit(100));
                assert_eq!(e.health, hp);
            } else {
                assert_eq!(e.health, 0);
            }
        }
    }

    #[test]
    fn freeze_preserves_velocity_and_bosses_are_immune() {
        let mut e = Enemy::new(EnemyKind::Snake, origin(), false);
        e.state = EnemyState::Attacking;
        e.timer = 0;
        e.freeze(190);
        for _ in 0..86 {
            e.tick(Some(origin()));
            assert!(e.is_stunned());
            assert!(!e.contact_enabled());
        }
        e.tick(Some(origin()));
        assert!(e.is_stunned());
        e.tick(Some(origin()));
        assert!(!e.is_stunned());
        assert!(e.contact_enabled());
        assert_eq!(e.body.velocity.x, -176. / 256.);
        e.tick(Some(origin()));
        assert!(e.is_stunned());
        assert_eq!(e.body.velocity.x, 0.);
        e.tick(Some(origin()));
        assert!(!e.is_stunned());
        assert_eq!(e.body.velocity.x, -176. / 256.);
        let mut b = Enemy::new(EnemyKind::Boss(BossKind::Flying), origin(), false);
        b.freeze(120);
        assert_eq!(b.frozen_ticks, 0);
    }

    #[test]
    fn segmented_boss_matches_recorded_original_movement() {
        #[derive(serde::Deserialize)]
        struct Sample {
            tick: u64,
            x: f32,
            y: f32,
            wait: u16,
            step: u8,
        }
        let samples: Vec<Sample> = serde_json::from_str(include_str!(
            "../tests/fixtures/segmented_boss_reference.json"
        ))
        .unwrap();
        let mut e = Enemy::new(EnemyKind::Boss(BossKind::Segmented), origin(), false);
        for sample in samples {
            while e.age < sample.tick {
                e.tick(None);
            }
            assert_eq!(
                e.segments[0].position.x + 4.,
                sample.x,
                "tick {} x",
                sample.tick
            );
            assert_eq!(
                e.segments[0].position.y + 4.,
                sample.y,
                "tick {} y",
                sample.tick
            );
            assert_eq!(e.movement_wait, sample.wait);
            assert_eq!(e.movement_step, sample.step);
        }
    }

    #[test]
    fn segmented_boss_damage_removes_only_impacted_segment() {
        let mut e = Enemy::new(EnemyKind::Boss(BossKind::Segmented), origin(), false);
        let first = e.segments[0];
        assert!(e.overlaps(&first));
        assert!(!e.hit_at(first.center(), 4));
        assert_eq!(e.health, 29);
        assert!(!e.overlaps(&first));
        assert!(!e.hit_at(first.center(), 4));
        assert_eq!(e.health, 29);
    }

    #[test]
    fn goat_bursts_follow_recovered_difficulty_timers() {
        for (hard, expected) in [(false, vec![209, 308]), (true, vec![104, 183, 262])] {
            let mut e = Enemy::new(EnemyKind::Goat, origin(), hard);
            let actual: Vec<_> = (1..=330)
                .filter(|_| !e.tick(Some(origin())).is_empty())
                .collect();
            assert_eq!(actual, expected);
        }
    }
    #[test]
    fn lethal_bomb_hit_arms_one_sixty_tick_fuse() {
        let mut e = Enemy::new(EnemyKind::Bomb, origin(), false);
        assert!(!e.hit(100));
        assert_eq!(e.state, EnemyState::Fuse);
        assert!(!e.hit(100));
        for _ in 0..59 {
            assert!(e.tick(None).is_empty());
        }
        assert_eq!(e.tick(None).len(), 2);
        assert_eq!(e.health, 0);
        assert!(e.tick(None).is_empty());
    }
    #[test]
    fn boss_damage_has_a_twenty_tick_protection_window() {
        let mut e = Enemy::new(EnemyKind::Boss(BossKind::Final), origin(), false);
        assert!(!e.hit(4));
        assert_eq!(e.health, 70);
        assert!(!e.hit(4));
        assert_eq!(e.health, 70);
        for _ in 0..20 {
            e.tick(None);
        }
        assert!(!e.hit(4));
        assert_eq!(e.health, 66);
        for _ in 0..20 {
            e.tick(None);
        }
        assert!(e.hit(100));
        assert!(!e.hit(100));
        assert!(e.tick(Some(origin())).is_empty());
    }
    #[test]
    fn troll_keeps_its_facing_and_cannot_fire_each_tick() {
        let mut e = Enemy::new(EnemyKind::Troll, origin(), false);
        let target = Some(Vec2 { x: -30., y: 0. });
        for _ in 0..143 {
            assert!(e.tick(target).is_empty());
        }
        let shot = e.tick(target);
        assert_eq!(shot.len(), 1);
        assert_eq!(shot[0].kind, ProjectileKind::Spear);
        assert!(shot[0].velocity.x < 0.);
        assert!(e.tick(target).is_empty());
    }
    #[test]
    fn hopper_jumps_only_when_grounded_and_threatened() {
        let mut e = Enemy::new(EnemyKind::Hopper, Vec2 { x: 60., y: 68. }, false);
        e.body.grounded = true;
        e.tick(Some(origin()));
        assert_eq!(e.body.velocity.y, 0.);
        let mut context = EnemyContext::single(Some(origin()), 2);
        context.nearby_shots[0] = Some(Vec2 { x: 80., y: 72. });
        e.tick_with_context(&context, &mut EnemyRng::new(0));
        assert_eq!(e.body.velocity.y, -2.5);
        assert!(!e.body.grounded);
    }
    #[test]
    fn final_boss_charge_dash_and_recovery_have_bounded_durations() {
        let mut e = Enemy::new(EnemyKind::Boss(BossKind::Final), origin(), false);
        e.final_action = FinalAction::DashCharge;
        e.state = EnemyState::Boss(BossPhase::Charge);
        e.timer = 40;
        let target = Some(Vec2 { x: -10., y: 0. });
        for _ in 0..40 {
            e.tick(target);
        }
        assert_eq!(e.state, EnemyState::Boss(BossPhase::Dash));
        assert_eq!(e.body.velocity.x, -5.023_437_5);
        for _ in 0..50 {
            e.tick(target);
        }
        assert_eq!(e.state, EnemyState::Boss(BossPhase::Recover));
        assert_eq!(e.body.velocity.x, -2.);
    }
}
