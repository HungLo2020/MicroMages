//! One playable world: entity interaction, terrain, checkpoints, and combat.
use crate::{
    assets::{Entity, Level, TerrainChange},
    camera::Camera,
    enemy::{BossKind, Enemy, EnemyContext, EnemyKind, EnemyRng, HitResponse, WorldAction},
    events::{GameEvent, Sound},
    input::InputFrame,
    physics::{self, Body, CollisionMap, Contact, Vec2},
    pickup::{DropContext, PickupKind, Rewards},
    player::{GRAVITY, Player, PlayerEvent, PlayerState},
    projectile::{Appearance, Owner, Projectile},
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    Checkpoint,
    Exit,
    Portal(u8),
    Flame,
    Wind,
    Updraft { reach: u8, affects_players: bool },
    BossEntrance,
    Gem(u32),
    Fairy,
    Feather,
    Swap,
}
#[derive(Clone, Debug)]
pub struct Prop {
    pub pickup: Option<crate::pickup::Pickup>,
    pub kind: PropKind,
    pub source_kind: u8,
    pub body: Body,
    pub active: bool,
    pub flip_x: bool,
}
impl PropKind {
    fn from_entity(entity: Entity) -> Option<Self> {
        Some(match entity {
            Entity::Checkpoint => Self::Checkpoint,
            Entity::LevelExit => Self::Exit,
            Entity::PortalBlue => Self::Portal(0),
            Entity::PortalGreen => Self::Portal(1),
            Entity::PortalPink => Self::Portal(2),
            Entity::Flame => Self::Flame,
            Entity::HorizontalWind => Self::Wind,
            Entity::WindCurrent => Self::Updraft {
                reach: 69,
                affects_players: true,
            },
            Entity::BossEntrance => Self::BossEntrance,
            _ => return None,
        })
    }
}
fn enemy_kind(entity: Entity) -> Option<EnemyKind> {
    Some(match entity {
        Entity::InhalingBoss => EnemyKind::Boss(BossKind::Inhaling),
        Entity::SummoningBoss => EnemyKind::Boss(BossKind::Summoning),
        Entity::FlyingBoss => EnemyKind::Boss(BossKind::Flying),
        Entity::FinalBoss => EnemyKind::Boss(BossKind::Final),
        Entity::SorcererBoss => EnemyKind::Boss(BossKind::Sorcerer),
        Entity::SegmentedBoss => EnemyKind::Boss(BossKind::Segmented),
        Entity::Turret => EnemyKind::Turret,
        Entity::SummoningPlatform => EnemyKind::SummoningPlatform,
        Entity::Leaper => EnemyKind::Leaper,
        Entity::Goat => EnemyKind::Goat,
        Entity::Willowisp => EnemyKind::Willowisp,
        Entity::Bomb => EnemyKind::Bomb,
        Entity::Troll => EnemyKind::Troll,
        Entity::Snake => EnemyKind::Snake,
        Entity::Walker => EnemyKind::Walker,
        Entity::DiagonalFlyer => EnemyKind::DiagonalFlyer,
        Entity::Chaser => EnemyKind::Chaser,
        Entity::Hopper => EnemyKind::Hopper,
        Entity::WallSpike => EnemyKind::Spike,
        Entity::Lightning => EnemyKind::Lightning,
        Entity::Spiderweb => EnemyKind::Spiderweb,
        _ => return None,
    })
}

// Platforms support actors but are not solid to player spells (source c7ce).
struct SpellTerrain<'a>(&'a Level);
impl CollisionMap for SpellTerrain<'_> {
    fn width(&self) -> usize {
        self.0.width
    }
    fn height(&self) -> usize {
        self.0.height
    }
    fn material(&self, column: i32, row: i32) -> physics::Material {
        physics::Material(self.0.material(column, row).0 & !4)
    }
}

#[derive(Clone, Debug)]
pub struct Particle {
    pub source_kind: u8,
    pub position: Vec2,
    pub velocity: Vec2,
    pub ticks_left: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArenaEntry {
    Gathering,
    Ascending,
    Closing(u8),
}

#[derive(Clone)]
pub struct World {
    pub level: Level,
    pub players: Vec<Player>,
    pub enemies: Vec<Enemy>,
    pub props: Vec<Prop>,
    pub projectiles: Vec<Projectile>,
    pub particles: Vec<Particle>,
    pub camera: Camera,
    pub checkpoint: Vec2,
    pub score: u32,
    /// Individual decimal score units; the original score table appends "00".
    pub player_scores: [u32; 4],
    pub lives: u8,
    pub tick: u64,
    pub(crate) rewards: Rewards,
    pub(crate) portal_reward_collected: bool,
    random: EnemyRng,
    initial_level: Arc<Level>,
    contacts: Vec<Contact>,
    portal_cooldown: Vec<u16>,
    wipe_ticks: u16,
    segmented_reward_spawned: bool,
    arena_entry: Option<ArenaEntry>,
    arena_ticks: u16,
    ending_ascent: bool,
}

impl World {
    pub fn new(level: Level, player_count: usize) -> Self {
        let start = Vec2::new(level.player_start.x - 4.0, level.player_start.y - 4.0);
        let mut enemies = Vec::new();
        let mut props = Vec::new();
        for (spawn_index, spawn) in level.spawns.iter().enumerate() {
            if spawn.entity == Entity::PlayerStart {
                continue;
            }
            let position = Vec2::new(spawn.x - 4.0, spawn.y - 4.0);
            if let Some(mut kind) = PropKind::from_entity(spawn.entity) {
                if let PropKind::Updraft {
                    affects_players, ..
                } = &mut kind
                {
                    *affects_players = !spawn.flip_y;
                }
                props.push(Prop {
                    pickup: None,
                    kind,
                    source_kind: spawn.kind,
                    body: Body::new(position, Vec2::new(8.0, 8.0)),
                    active: true,
                    flip_x: spawn.flip_x,
                });
            } else if let Some(kind) = enemy_kind(spawn.entity) {
                let mut enemy = Enemy::new(kind, position, level.hard);
                enemy.body.position = Vec2::new(
                    spawn.x - enemy.body.size.x * 0.5,
                    spawn.y - enemy.body.size.y * 0.5,
                );
                if matches!(kind, EnemyKind::Boss(_)) && level.id != 13 {
                    enemy.wait_for_arena();
                }
                if !matches!(kind, EnemyKind::Boss(_)) {
                    enemy.update_phase = (spawn_index.saturating_sub(1) % 4) as u8;
                }
                if matches!(kind, EnemyKind::Turret | EnemyKind::SummoningPlatform) {
                    enemy.body.position.x += 4.0;
                }
                enemy.source_kind = spawn.kind;
                enemy.facing = if spawn.flip_x { -1.0 } else { 1.0 };
                enemies.push(enemy);
            }
        }
        let players = (0..player_count)
            .map(|id| {
                let mut player = Player::new(
                    id,
                    Vec2::new(start.x + (player_count - 1 - id) as f32 * 8.0, start.y),
                );
                if level.hard && level.id == 13 {
                    player.flight_mode = true;
                    player.health = 2;
                }
                player
            })
            .collect();
        let camera = Camera::at_player(start, (level.height * 8) as f32);
        Self {
            initial_level: Arc::new(level.clone()),
            level,
            players,
            enemies,
            props,
            projectiles: Vec::new(),
            particles: Vec::new(),
            camera,
            checkpoint: start,
            score: 0,
            player_scores: [0; 4],
            lives: 3,
            tick: 0,
            rewards: Rewards::default(),
            portal_reward_collected: false,
            random: EnemyRng::new(0),
            contacts: vec![Contact::default(); player_count],
            portal_cooldown: vec![0; player_count],
            wipe_ticks: 0,
            segmented_reward_spawned: false,
            arena_entry: None,
            arena_ticks: 0,
            ending_ascent: false,
        }
    }

    pub(crate) fn seed_random(&mut self, frame: u64) {
        let low = frame as u8;
        self.random = EnemyRng::new(u16::from(low) | (u16::from(low >> 1) << 8));
    }

    pub(crate) fn player_events(
        &mut self,
        id: usize,
        events: Vec<PlayerEvent>,
        output: &mut Vec<GameEvent>,
    ) {
        for event in events {
            let sound = match event {
                PlayerEvent::Jumped => Sound::Jump,
                PlayerEvent::Shot { direction, charged } => {
                    if self
                        .projectiles
                        .iter()
                        .filter(|p| p.owner == Owner::Player(id) && !p.freezes)
                        .count()
                        < 2
                    {
                        let extra_arena = self.level.hard && self.level.id == 13;
                        let direction = if extra_arena {
                            Vec2::new(1.0, 0.0)
                        } else {
                            direction
                        };
                        let mut shot = Projectile::spell(
                            id,
                            self.players[id].body.center(),
                            direction,
                            charged,
                        );
                        if extra_arena {
                            shot.body.velocity.x = 4.0;
                        }
                        self.projectiles.push(shot);
                    }
                    if charged {
                        Sound::ChargedSpell
                    } else {
                        Sound::Spell
                    }
                }
                PlayerEvent::GhostShot { direction } => {
                    if !self
                        .projectiles
                        .iter()
                        .any(|p| p.owner == Owner::Player(id) && p.freezes)
                    {
                        let shot =
                            Projectile::freeze(id, self.players[id].body.center(), direction);
                        self.projectiles.push(shot);
                    }
                    Sound::Spell
                }
                PlayerEvent::Hurt => Sound::Hurt,
                PlayerEvent::Died => Sound::Ghost,
                PlayerEvent::Revived => Sound::Fairy,
            };
            output.push(GameEvent::Sound(sound));
        }
    }

    pub(crate) fn step(
        &mut self,
        inputs: [InputFrame; 4],
        output: &mut Vec<GameEvent>,
    ) -> RunResult {
        self.step_with_clear_state(inputs, output, false)
    }

    /// Source d471 replaces actors whose descriptor has bit08 with a short
    /// rising disappearance sprite. Other ordinary enemies continue updating.
    pub(crate) fn begin_stage_clear(&mut self) -> u16 {
        let Some(boss) = self
            .enemies
            .iter()
            .find(|enemy| matches!(enemy.kind, EnemyKind::Boss(_)) && enemy.health == 0)
        else {
            return 256;
        };
        let duration = if boss.kind == EnemyKind::Boss(BossKind::Final) {
            140
        } else {
            510
        };
        for enemy in &mut self.enemies {
            if enemy.is_alive()
                && matches!(
                    enemy.source_kind,
                    5 | 9 | 13 | 16 | 18 | 19 | 22 | 24 | 27 | 36
                )
            {
                self.particles.push(Particle {
                    source_kind: 50,
                    position: enemy.body.center(),
                    velocity: Vec2::new(0., -0.5),
                    ticks_left: 11,
                });
                enemy.health = 0;
                enemy.state = crate::enemy::EnemyState::Defeated;
            }
        }
        duration
    }

    fn segmented_reward_pending(&mut self) -> bool {
        if !self.enemies.iter().any(|enemy| {
            enemy.kind == EnemyKind::Boss(BossKind::Segmented)
                && enemy.health == 0
                && enemy.segments_hit == 0
        }) {
            return false;
        }
        if !self.segmented_reward_spawned {
            self.segmented_reward_spawned = true;
            self.props.push(Prop {
                pickup: None,
                kind: PropKind::Gem(4),
                source_kind: 54,
                body: Body::new(Vec2::new(124., 164.), Vec2::new(8., 8.)),
                active: true,
                flip_x: false,
            });
        }
        self.props
            .iter()
            .any(|prop| prop.source_kind == 54 && prop.active)
    }

    /// The source keeps actors running during its exit countdown, but suppresses
    /// team-wipe restarts and further stage-completion notifications.
    pub(crate) fn step_after_clear(
        &mut self,
        inputs: [InputFrame; 4],
        output: &mut Vec<GameEvent>,
    ) {
        self.step_with_clear_state(inputs, output, true);
    }

    /// B73A starts the lift after its 210-tick wait. The source writes only
    /// velocity's signed high byte, preserving fractional acceleration.
    pub(crate) fn begin_segmented_ascent(&mut self) {
        self.ending_ascent = true;
    }

    pub(crate) fn lift_after_segmented_boss(&mut self) -> bool {
        self.ending_ascent = true;
        for player in &mut self.players {
            if player.state == PlayerState::Dead {
                continue;
            }
            player.body.velocity.y = -2.0 + player.body.velocity.y.rem_euclid(1.0);
            if player.body.center().y.floor() < 7.0 {
                player.state = PlayerState::Dead;
            }
        }
        self.players
            .iter()
            .all(|player| player.state == PlayerState::Dead)
    }

    fn step_with_clear_state(
        &mut self,
        inputs: [InputFrame; 4],
        output: &mut Vec<GameEvent>,
        clearing: bool,
    ) -> RunResult {
        self.tick += 1;
        for particle in &mut self.particles {
            particle.position.x += particle.velocity.x;
            particle.position.y += particle.velocity.y;
            particle.ticks_left = particle.ticks_left.saturating_sub(1);
        }
        self.particles.retain(|p| p.ticks_left > 0);
        if self.arena_entry.is_some() {
            self.step_arena_entry(output);
            return RunResult::Continue;
        }
        for (id, input) in inputs.into_iter().enumerate().take(self.players.len()) {
            if self.players[id].is_exiting() {
                self.players[id].update_exit(&self.level);
                continue;
            }
            self.portal_cooldown[id] = self.portal_cooldown[id].saturating_sub(1);
            let was_warping = self.players[id].is_warping();
            self.players[id].prepare_viewport(self.camera.position.y);
            let mut environment = physics::player_environment(&self.players[id].body, &self.level);
            environment.slippery &= self.level.hard && self.level.tower == 1;
            let events = self.players[id].update_with_environment(input, environment);
            self.player_events(id, events, output);
            let player = &mut self.players[id];
            if was_warping || player.state == PlayerState::Ghost || player.flying() {
                player.body.position.x += player.body.velocity.x;
                player.body.position.y += player.body.velocity.y;
            } else if matches!(player.state, PlayerState::Alive | PlayerState::Dying) {
                let floating = player.floating();
                self.contacts[id] = if self.ending_ascent {
                    physics::move_player_during_ascent(&mut player.body, &self.level)
                } else {
                    physics::move_player(&mut player.body, &self.level, input.held.down, floating)
                };
                if self.contacts[id].ceiling {
                    output.push(GameEvent::Sound(Sound::HeadBump));
                }
                if self.contacts[id].hazard
                    || player.body.center().y >= self.camera.position.y + 240.0
                {
                    // Source $89c9 applies horizontal knockback only to picture
                    // 1. Picture 2 shares the damage material but keeps momentum.
                    let center = player.body.center();
                    let column = (center.x.floor() as i32).div_euclid(8);
                    let row = (center.y.floor() as i32 - 2).div_euclid(8);
                    let knocks_back = self.contacts[id].hazard
                        && column >= 0
                        && row >= 0
                        && column < self.level.width as i32
                        && row < self.level.height as i32
                        && self.level.tiles[row as usize * self.level.width + column as usize] == 1;
                    let events = if knocks_back {
                        player.damage_from_hazard()
                    } else {
                        player.damage()
                    };
                    self.player_events(id, events, output);
                }
            }
            self.players[id].clamp_to_viewport(self.camera.position.y);
        }
        for (id, event) in crate::player::interact_players(&mut self.players, inputs) {
            self.player_events(id, vec![event], output);
        }
        if let Some(highest) = self
            .players
            .iter()
            .filter(|p| p.state == PlayerState::Alive)
            .map(|p| p.body.position)
            .min_by(|a, b| a.y.total_cmp(&b.y))
        {
            self.camera.follow(highest, (self.level.height * 8) as f32);
        }
        self.update_enemies(output);
        self.update_projectiles(output);
        self.segmented_reward_pending();
        let complete = self.update_props(inputs, output);
        // Cooperative bumps and wind also move bodies; apply the viewport guard
        // after those forces so no participant can escape the playable bounds.
        for player in &mut self.players {
            if !player.is_exiting() {
                player.clamp_to_viewport(self.camera.position.y);
            }
        }
        if clearing {
            return RunResult::Continue;
        }
        if complete {
            return RunResult::Complete;
        }
        if self
            .enemies
            .iter()
            .any(|e| matches!(e.kind, EnemyKind::Boss(_)))
            && !self
                .enemies
                .iter()
                .any(|e| matches!(e.kind, EnemyKind::Boss(_)) && e.health > 0)
            && !self.segmented_reward_pending()
        {
            return RunResult::Complete;
        }
        if self.players.iter().all(|p| p.state != PlayerState::Alive) {
            self.wipe_ticks += 1;
            if self.wipe_ticks >= 128 {
                if self.lives == 0 {
                    return RunResult::GameOver;
                }
                self.lives -= 1;
                self.respawn();
            }
        } else {
            self.wipe_ticks = 0;
        }
        RunResult::Continue
    }

    fn update_enemies(&mut self, output: &mut Vec<GameEvent>) {
        let mut summoned = Vec::new();
        let mut actions = Vec::new();
        let mut nearby_shots = [None; 12];
        for (slot, shot) in nearby_shots.iter_mut().zip(
            self.projectiles
                .iter()
                .filter(|s| matches!(s.owner, Owner::Player(_))),
        ) {
            *slot = Some(shot.body.center());
        }
        let summoner = self
            .enemies
            .iter()
            .find(|e| e.kind == EnemyKind::Boss(BossKind::Summoning));
        let turret_active = summoner.is_some_and(|e| e.arena_active());
        let turret_period = summoner.map_or(216, |e| {
            (e.health + if self.level.hard { 76 } else { 140 }) & 255
        });
        // Summoned actors occupy later source slots and update before their boss.
        // Their viewport cleanup must therefore be reflected in this tick's cap.
        let mut update_order: Vec<usize> = (0..self.enemies.len()).collect();
        update_order.sort_by_key(|&i| {
            !matches!(self.enemies[i].kind, EnemyKind::Minion | EnemyKind::Chaser)
        });
        for index in update_order {
            let active_minions = self
                .enemies
                .iter()
                .filter(|e| e.kind == EnemyKind::Minion && e.health > 0)
                .count();
            let active_chasers = self
                .enemies
                .iter()
                .filter(|e| {
                    e.kind == EnemyKind::Chaser
                        && e.source_kind == EnemyKind::Chaser.source_id()
                        && e.health > 0
                })
                .count();
            let enemy = &mut self.enemies[index];
            if !enemy.is_alive()
                || (!enemy.needs_viewport_update()
                    && (enemy.body.position.y < self.camera.position.y - 64.0
                        || enemy.body.position.y > self.camera.position.y + 304.0))
            {
                continue;
            }
            if enemy.kind == EnemyKind::Spike {
                let center = enemy.body.center();
                if !self
                    .level
                    .material((center.x / 8.0) as i32, (center.y / 8.0) as i32)
                    .hazard()
                {
                    enemy.health = 0;
                    continue;
                }
            }
            let context = EnemyContext {
                players: std::array::from_fn(|id| {
                    self.players
                        .get(id)
                        .filter(|p| p.state == PlayerState::Alive)
                        .map(|p| p.body.center())
                }),
                camera_y: self.camera.position.y,
                frame_index: self.tick,
                active_minions,
                active_chasers,
                // The source's bit-6 challenge mode is independent of the
                // hard campaign's final arena; entering that arena sets no bit.
                extra_mode: false,
                nearby_shots,
                at_ledge: {
                    let center = enemy.body.center();
                    let floor = self.level.material(
                        ((center.x + enemy.facing * 4.0) / 8.0).floor() as i32,
                        ((center.y + 9.0) / 8.0).floor() as i32,
                    );
                    !floor.solid() && !floor.platform()
                },
                turret_active,
                turret_period,
            };
            let frame = enemy.tick_with_context(&context, &mut self.random);
            actions.extend(frame.actions);
            for attack in frame.attacks {
                use crate::enemy::ProjectileKind;
                output.push(GameEvent::Sound(match attack.kind {
                    ProjectileKind::Bubble => Sound::GoatBubble,
                    ProjectileKind::Spear => Sound::TrollSpear,
                    ProjectileKind::Spark => Sound::Spark,
                    ProjectileKind::Spell => Sound::Spell,
                    ProjectileKind::FallingHazard | ProjectileKind::Orb => Sound::Spell,
                    ProjectileKind::Lightning => Sound::Spark,
                }));
                if attack.kind == ProjectileKind::Lightning {
                    let mut lightning =
                        Enemy::new(EnemyKind::Lightning, attack.position, self.level.hard);
                    lightning.body.position.x -= lightning.body.size.x * 0.5;
                    lightning.body.position.y -= lightning.body.size.y * 0.5;
                    lightning.start_lightning_warning(true);
                    summoned.push(lightning);
                } else {
                    self.projectiles
                        .push(Projectile::hostile_with_difficulty(attack, self.level.hard));
                }
            }
            if enemy.is_stunned() {
                continue;
            }
            if !enemy.motion_paused() {
                if matches!(
                    enemy.kind,
                    EnemyKind::Boss(_)
                        | EnemyKind::Chaser
                        | EnemyKind::Minion
                        | EnemyKind::Willowisp
                ) {
                    enemy.body.position.x += enemy.body.velocity.x;
                    enemy.body.position.y += enemy.body.velocity.y;
                } else if matches!(enemy.kind, EnemyKind::Walker | EnemyKind::Bomb) {
                    if enemy.body.velocity.x != 0. {
                        enemy.body.position.x += enemy.body.velocity.x;
                        let center = enemy.body.center();
                        let column = ((center.x.floor() + enemy.facing * 4.) / 8.).floor() as i32;
                        let floor = self
                            .level
                            .material(column, ((center.y.floor() + 9.) / 8.).floor() as i32);
                        let wall = self
                            .level
                            .material(column, (center.y.floor() / 8.).floor() as i32);
                        if (!floor.solid() && !floor.platform()) || wall.solid() {
                            enemy.reverse();
                        }
                    }
                } else if enemy.kind == EnemyKind::DiagonalFlyer {
                    physics::move_diagonal_flyer(&mut enemy.body, &self.level);
                    enemy.facing = if enemy.body.velocity.x < 0. { -1. } else { 1. };
                } else if enemy.kind == EnemyKind::SummonedWalker {
                    let contact = physics::move_summoned_walker(&mut enemy.body, &self.level);
                    enemy.set_wall_contact(contact.wall);
                } else {
                    if enemy.kind.uses_gravity() {
                        enemy.body.velocity.y = (enemy.body.velocity.y + GRAVITY).min(4.0);
                    }
                    let contact = physics::move_body(&mut enemy.body, &self.level, false);
                    if contact.wall != 0 {
                        enemy.reverse();
                    }
                }
            }
            if enemy.remove_if_outside_viewport(self.camera.position.y) {
                continue;
            }
            for player in &mut self.players {
                if player.state == PlayerState::Alive
                    && !player.is_warping()
                    && enemy.contact_enabled()
                    && enemy.overlaps(&player.body)
                {
                    if enemy.kind == EnemyKind::Spiderweb {
                        player.spring_launch();
                        output.push(GameEvent::Sound(Sound::Spiderweb));
                        continue;
                    }
                    for event in player.damage() {
                        output.push(GameEvent::Sound(if event == PlayerEvent::Died {
                            Sound::Ghost
                        } else {
                            Sound::Hurt
                        }));
                    }
                }
            }
        }
        for action in actions {
            match action {
                WorldAction::PullPlayersBelow { y, velocity_y } => {
                    for player in &mut self.players {
                        if player.state == PlayerState::Alive && player.body.center().y > y {
                            player.body.velocity.y = velocity_y;
                        }
                    }
                }
                WorldAction::BreakTile { x, y } => {
                    self.level.break_tile(x as i32, y as i32);
                }
                WorldAction::SpawnEnemy {
                    kind,
                    position,
                    velocity,
                    target,
                } => {
                    let mut enemy = Enemy::new(kind, position, self.level.hard);
                    enemy.body.position.x -= enemy.body.size.x * 0.5;
                    enemy.body.position.y -= enemy.body.size.y * 0.5;
                    enemy.body.velocity = velocity;
                    if velocity.x != 0.0 {
                        enemy.facing = velocity.x.signum();
                    }
                    enemy.set_target(target);
                    summoned.push(enemy);
                }
                WorldAction::Particle { position, velocity } => {
                    self.particles.push(Particle {
                        source_kind: 60,
                        position,
                        velocity,
                        ticks_left: 60,
                    });
                }
                WorldAction::LockPlayerControls { ticks } => {
                    for player in &mut self.players {
                        player.lock_controls(ticks);
                    }
                }
            }
        }
        self.enemies.extend(summoned);
    }

    pub(crate) fn update_projectiles(&mut self, output: &mut Vec<GameEvent>) {
        let mut drops = Vec::new();
        for shot in &mut self.projectiles {
            shot.age();
            if shot.ticks_left == 0 {
                continue;
            }
            let previous_body = shot.body;
            let contact = if shot.owner == Owner::Enemy {
                if shot.appearance == Appearance::Spark {
                    shot.advance_bouncing(&self.level);
                } else {
                    shot.advance_motion();
                }
                let center = shot.body.center();
                if center.x < 7.0
                    || center.x >= 249.0
                    || center.y < self.camera.position.y
                    || center.y >= self.camera.position.y + 240.0
                {
                    shot.ticks_left = 0;
                    continue;
                }
                Contact::default()
            } else {
                physics::move_body(&mut shot.body, &SpellTerrain(&self.level), false)
            };
            if contact.wall != 0 || contact.ceiling || contact.landed {
                let mut passes_terrain = false;
                if matches!(shot.owner, Owner::Player(_)) {
                    let center = shot.body.center();
                    let x = ((center.x + f32::from(contact.wall) * 4.0) / 8.0).floor() as i32;
                    let y = ((center.y
                        + if contact.ceiling {
                            -4.0
                        } else if contact.landed {
                            4.0
                        } else {
                            0.0
                        })
                        / 8.0)
                        .floor() as i32;
                    let change = self.level.hit_tile(x, y, if shot.charged { 6 } else { 2 });
                    passes_terrain = shot.freezes
                        && matches!(change, TerrainChange::None | TerrainChange::OpenedChest);
                    if change == TerrainChange::Damaged {
                        output.push(GameEvent::Sound(Sound::DamageBlock));
                    }
                    if matches!(change, TerrainChange::Broken | TerrainChange::OpenedChest) {
                        let context = DropContext {
                            active_players: self.players.len() as u8,
                            screen_y: (center.y - self.camera.position.y).clamp(0.0, 255.0) as u8,
                            random: self.random.byte(),
                        };
                        let drop = if change == TerrainChange::OpenedChest {
                            Some(self.rewards.open_chest(context))
                        } else {
                            self.rewards.break_block(context)
                        };
                        if let Some(kind) = drop {
                            let position = if change == TerrainChange::OpenedChest {
                                Vec2::new(((x * 8) & !15) as f32 + 2.0, y as f32 * 8.0 - 8.0)
                            } else {
                                Vec2::new(x as f32 * 8.0, y as f32 * 8.0)
                            };
                            drops.push((kind, position));
                        }
                        output.push(GameEvent::Sound(if change == TerrainChange::OpenedChest {
                            Sound::OpenChest
                        } else {
                            Sound::DestroyBlock
                        }));
                    }
                }
                if passes_terrain {
                    shot.body = previous_body;
                    shot.body.position.x += shot.body.velocity.x;
                    shot.body.position.y += shot.body.velocity.y;
                } else {
                    shot.ticks_left = 0;
                    continue;
                }
            }
            match shot.owner {
                Owner::Player(owner) => {
                    for prop in &mut self.props {
                        if prop.active
                            && prop.kind == PropKind::Flame
                            && prop.body.overlaps(&shot.body)
                        {
                            prop.active = false;
                            shot.ticks_left = 0;
                            output.push(GameEvent::Sound(Sound::Extinguish));
                            break;
                        }
                    }
                    if shot.ticks_left == 0 {
                        continue;
                    }
                    for enemy in &mut self.enemies {
                        if enemy.is_alive() && enemy.overlaps(&shot.body) {
                            if shot.freezes {
                                if !enemy.can_freeze() {
                                    continue;
                                }
                                if enemy.frozen_ticks == 0 {
                                    enemy.freeze(
                                        if matches!(
                                            enemy.kind,
                                            EnemyKind::Willowisp | EnemyKind::Bomb
                                        ) {
                                            105
                                        } else {
                                            190
                                        },
                                    );
                                }
                                shot.ticks_left = 0;
                                break;
                            }
                            match enemy.hit_response(shot.damage) {
                                HitResponse::Pass => continue,
                                HitResponse::Absorb => {
                                    enemy.hit(shot.damage);
                                    shot.ticks_left = 0;
                                    break;
                                }
                                HitResponse::Apply => {}
                            }
                            if enemy.kind == EnemyKind::Boss(BossKind::Segmented) {
                                let before = enemy.health;
                                enemy.hit_at(shot.body.center(), shot.damage);
                                if enemy.health < before {
                                    // b715–b731 consumes every spell and awards one
                                    // unit to its owner for each individual segment.
                                    self.score += 1;
                                    self.player_scores[owner] += 1;
                                    output.push(GameEvent::Sound(Sound::DestroyEnemy));
                                    shot.ticks_left = 0;
                                    break;
                                }
                                continue;
                            }
                            let consumed = enemy.health.min(shot.damage);
                            let killed = enemy.hit_at(shot.body.center(), shot.damage);
                            if killed {
                                let award = u32::from(enemy.score_units());
                                self.score += award;
                                self.player_scores[owner] += award;
                                output.push(GameEvent::Sound(Sound::DestroyEnemy));
                            } else {
                                output.push(GameEvent::Sound(Sound::DamageEnemy));
                            }
                            if shot.charged && consumed < 6 {
                                shot.downgrade();
                            } else {
                                shot.ticks_left = 0;
                            }
                            break;
                        }
                    }
                }
                Owner::Enemy => {
                    for player in &mut self.players {
                        if player.state == PlayerState::Alive
                            && !player.is_warping()
                            && !player.is_exiting()
                            && player.body.overlaps(&shot.body)
                        {
                            let events = player.damage();
                            if !events.is_empty() {
                                output.push(GameEvent::Sound(Sound::Hurt));
                            }
                            shot.ticks_left = 0;
                            break;
                        }
                    }
                }
            }
        }
        self.projectiles.retain(|shot| shot.ticks_left > 0);
        for (kind, position) in drops {
            self.spawn_pickup(kind, position);
        }
    }

    fn spawn_pickup(&mut self, kind: PickupKind, position: Vec2) {
        let prop_kind = match kind {
            PickupKind::SmallGem | PickupKind::MediumGem | PickupKind::LargeGem => {
                PropKind::Gem(kind.score_units())
            }
            PickupKind::Fairy => PropKind::Fairy,
            PickupKind::Feather => PropKind::Feather,
            PickupKind::Swap => PropKind::Swap,
        };
        let body = Body::new(position, Vec2::new(8.0, 8.0));
        let phase = (0..4)
            .find(|phase| {
                !self
                    .props
                    .iter()
                    .any(|p| p.active && p.pickup.as_ref().is_some_and(|p| p.phase == *phase))
            })
            .unwrap_or(0);
        self.props.push(Prop {
            pickup: Some(crate::pickup::Pickup::new(kind, phase)),
            kind: prop_kind,
            source_kind: kind.source_id(),
            body,
            active: true,
            flip_x: false,
        });
    }

    pub(crate) fn update_props(
        &mut self,
        _inputs: [InputFrame; 4],
        output: &mut Vec<GameEvent>,
    ) -> bool {
        for prop in &mut self.props {
            if let PropKind::Updraft {
                reach,
                affects_players,
            } = &mut prop.kind
            {
                if prop.body.center().y - self.camera.position.y >= 245.0 {
                    *reach = reach.saturating_sub(2);
                    if !*affects_players || *reach == 0 {
                        prop.active = false;
                    } else {
                        prop.body.position.y -= 2.0;
                    }
                }
                if prop.active && self.tick.is_multiple_of(8) {
                    self.particles.push(Particle {
                        source_kind: 60,
                        position: prop.body.center(),
                        velocity: Vec2::new(0.0, if prop.flip_x { 2.0 } else { -2.0 }),
                        ticks_left: u16::from(*reach / 2),
                    });
                }
            }
            if prop.active
                && let Some(pickup) = &mut prop.pickup
            {
                prop.active = pickup.step(self.tick, &mut prop.body);
                let screen = self.camera.screen(prop.body.center());
                prop.active &= (0.0..240.0).contains(&screen.y);
                if pickup.kind == PickupKind::Fairy {
                    prop.active &= (7.0..249.0).contains(&screen.x);
                }
            }
        }
        for prop_index in 0..self.props.len() {
            if !self.props[prop_index].active {
                continue;
            }
            let prop = self.props[prop_index].clone();
            if prop
                .pickup
                .as_ref()
                .is_some_and(|p| !p.can_collect(self.tick))
            {
                continue;
            }
            for id in 0..self.players.len() {
                if !self.props[prop_index].active {
                    break;
                }
                let player = &mut self.players[id];
                if player.is_warping()
                    || player.is_exiting()
                    || matches!(player.state, PlayerState::Dead | PlayerState::Dying)
                {
                    continue;
                }
                if player.state == PlayerState::Ghost
                    && !matches!(prop.kind, PropKind::Fairy | PropKind::Feather)
                {
                    continue;
                }
                let delta = Vec2::new(
                    player.body.center().x - prop.body.center().x,
                    player.body.center().y - prop.body.center().y,
                );
                if prop.kind == PropKind::Wind
                    && (-9.0..10.0).contains(&delta.y)
                    && if prop.flip_x {
                        (-72.0..0.0).contains(&delta.x)
                    } else {
                        (0.0..73.0).contains(&delta.x)
                    }
                {
                    player.body.position.x += if prop.flip_x { -1.0 } else { 1.0 };
                }
                if let PropKind::Updraft {
                    reach,
                    affects_players: true,
                } = prop.kind
                    && (-11.0..12.0).contains(&delta.x)
                    && (6.0 - f32::from(reach)..6.0).contains(&delta.y)
                {
                    player.body.velocity.y -= 136.0 / 256.0;
                    if player.body.velocity.y.floor() < -2.0 {
                        player.body.velocity.y = -258.0 / 256.0;
                    }
                }
                let radius = match prop.kind {
                    PropKind::BossEntrance => 16.0,
                    PropKind::Exit => 6.0,
                    PropKind::Portal(_) => 4.0,
                    PropKind::Flame => 9.0,
                    PropKind::Fairy | PropKind::Feather => 7.0,
                    _ => 8.0,
                };
                let touches = (player.body.center().x.floor() - prop.body.center().x.floor()).abs()
                    < radius
                    && (player.body.center().y.floor() - prop.body.center().y.floor()).abs()
                        < radius;
                if !touches {
                    continue;
                }
                match prop.kind {
                    PropKind::Checkpoint => {
                        if self.checkpoint != prop.body.position {
                            self.checkpoint = prop.body.position;
                            output.push(GameEvent::Sound(Sound::Checkpoint));
                        }
                    }
                    PropKind::Exit => {
                        player.begin_exit(prop.body.center().x.floor() + 11.0);
                        // C245/C264 play the two exit effects; C250 awards two
                        // decimal score units to the player entering the door.
                        self.score += 2;
                        self.player_scores[player.id] += 2;
                        output.push(GameEvent::Sound(Sound::LevelExitAlternate));
                        output.push(GameEvent::Sound(Sound::LevelExit));
                        return true;
                    }
                    PropKind::BossEntrance => {
                        self.props[prop_index].active = false;
                        self.projectiles.clear();
                        self.arena_entry = Some(ArenaEntry::Gathering);
                        self.arena_ticks = 0;
                        self.level.tiles[36 * self.level.width + 14] = 0;
                        for player in &mut self.players {
                            if player.state == PlayerState::Alive {
                                player.begin_seek(Vec2::new(111.0 + player.id as f32 * 8.0, 296.0));
                            }
                        }
                        self.wipe_ticks = 0;
                        return false;
                    }
                    PropKind::Portal(channel)
                        if player.body.grounded && self.portal_cooldown[id] == 0 =>
                    {
                        if let Some(destination) =
                            self.props.iter().enumerate().find(|(index, p)| {
                                *index != prop_index
                                    && p.active
                                    && p.kind == PropKind::Portal(channel)
                                    && p.body.position.y >= self.camera.position.y - 16.0
                                    && p.body.position.y <= self.camera.position.y + 256.0
                            })
                        {
                            player.begin_portal(destination.1.body.position);
                            self.portal_cooldown[id] = 16;
                            output.push(GameEvent::Sound(Sound::Portal));
                            if self.level.id == 5
                                && self.camera.position.y < 252.0
                                && !self.portal_reward_collected
                                && !self.props.iter().any(|p| p.source_kind == 54 && p.active)
                            {
                                self.props.push(Prop {
                                    pickup: None,
                                    kind: PropKind::Gem(10),
                                    source_kind: 54,
                                    body: Body::new(
                                        Vec2::new(124.0, self.camera.position.y + 116.0),
                                        Vec2::new(8.0, 8.0),
                                    ),
                                    active: true,
                                    flip_x: false,
                                });
                            }
                        }
                    }
                    // Source flame update $d2f5 skips the contact-damage call
                    // at $d2f2. Wall torches animate and can be extinguished by
                    // shots, but touching them is harmless.
                    PropKind::Flame => {}
                    PropKind::Gem(value) => {
                        self.score += value;
                        self.player_scores[player.id] += value;
                        if prop.source_kind == 54 && self.level.id != 11 {
                            self.portal_reward_collected = true;
                        }
                        self.props[prop_index].active = false;
                        output.push(GameEvent::Sound(Sound::Emerald));
                    }
                    PropKind::Fairy | PropKind::Feather => {
                        let was_ghost = player.state == PlayerState::Ghost;
                        if was_ghost {
                            let center = player.body.center();
                            if self
                                .level
                                .material((center.x / 8.0) as i32, ((center.y - 8.0) / 8.0) as i32)
                                .solid()
                            {
                                continue;
                            }
                            player.revive();
                        }
                        if prop.kind == PropKind::Feather {
                            player.feather = true;
                        } else if !was_ghost {
                            player.health = 2;
                        }
                        self.props[prop_index].active = false;
                        output.push(GameEvent::Sound(Sound::Fairy));
                    }
                    PropKind::Swap => {
                        if let Some(next) = self
                            .players
                            .iter()
                            .enumerate()
                            .rev()
                            .filter(|(other, p)| *other != id && p.state == PlayerState::Alive)
                            .min_by(|(_, a), (_, b)| {
                                a.body.position.y.total_cmp(&b.body.position.y)
                            })
                            .map(|(other, _)| other)
                        {
                            let position = self.players[next].body.position;
                            self.players[next].begin_swap(prop.body.position);
                            self.players[id].begin_swap(position);
                            output.push(GameEvent::Sound(Sound::Star));
                            self.props[prop_index].active = false;
                        }
                    }
                    _ => {}
                }
            }
        }
        false
    }

    fn step_arena_entry(&mut self, output: &mut Vec<GameEvent>) {
        self.arena_ticks += 1;
        if self.arena_ticks <= 24 && self.arena_ticks.is_multiple_of(8) {
            self.level.tiles[36 * self.level.width + 14 + usize::from(self.arena_ticks / 8)] = 0;
        }
        match self.arena_entry.expect("active arena entry") {
            ArenaEntry::Gathering => {
                if self.tick.is_multiple_of(16)
                    && let Some(player) = self
                        .players
                        .iter_mut()
                        .rev()
                        .find(|p| p.state != PlayerState::Alive)
                {
                    *player = Player::new(player.id, player.body.position);
                    player.begin_seek(Vec2::new(111.0 + player.id as f32 * 8.0, 296.0));
                    output.push(GameEvent::Sound(Sound::Fairy));
                }
                for player in &mut self.players {
                    if player.is_warping() {
                        player.update(InputFrame::default(), 0);
                        player.body.position.x += player.body.velocity.x;
                        player.body.position.y += player.body.velocity.y;
                    }
                }
                if self
                    .players
                    .iter()
                    .all(|p| p.state == PlayerState::Alive && !p.is_warping())
                {
                    self.arena_entry = Some(ArenaEntry::Ascending);
                }
            }
            ArenaEntry::Ascending => {
                self.camera.position.y = (self.camera.position.y - 2.0).max(0.0);
                for player in &mut self.players {
                    player.body.position.y = (player.body.position.y - 1.0).max(184.0);
                    player.body.velocity = Vec2::default();
                }
                if self.camera.position.y == 0.0
                    && self.players.iter().all(|p| p.body.position.y == 184.0)
                {
                    self.level.set_metatile(7, 12, 19);
                    self.level.set_metatile(8, 12, 19);
                    for column in 14..18 {
                        self.level.tiles[24 * self.level.width + column] = 0;
                    }
                    self.arena_entry = Some(ArenaEntry::Closing(32));
                }
            }
            ArenaEntry::Closing(ticks) if ticks > 1 => {
                if ticks.is_multiple_of(8) {
                    self.level.tiles[24 * self.level.width + 17 - usize::from((32 - ticks) / 8)] =
                        17;
                }
                self.arena_entry = Some(ArenaEntry::Closing(ticks - 1));
            }
            ArenaEntry::Closing(_) => {
                for enemy in &mut self.enemies {
                    if matches!(enemy.kind, EnemyKind::Boss(_)) {
                        enemy.activate_arena();
                    }
                }
                self.arena_entry = None;
                output.push(GameEvent::Music(crate::events::Music::Boss));
            }
        }
    }

    fn respawn(&mut self) {
        // Reload the level just as a new attempt does: destroyed terrain, defeated
        // enemies, collected rewards and spent projectiles belong to that attempt.
        let fresh = Self::new((*self.initial_level).clone(), self.players.len());
        self.seed_random(self.tick);
        self.level = fresh.level;
        self.enemies = fresh.enemies;
        self.props = fresh.props;
        let count = self.players.len();
        for player in &mut self.players {
            *player = Player::new(
                player.id,
                Vec2::new(
                    self.checkpoint.x + (count - 1 - player.id) as f32 * 8.0,
                    self.checkpoint.y,
                ),
            );
            player.invulnerability = 110;
            if self.level.hard && self.level.id == 13 {
                player.flight_mode = true;
                player.health = 2;
            }
        }
        self.contacts.fill(Contact::default());
        self.portal_cooldown.fill(0);
        self.projectiles.clear();
        self.particles.clear();
        self.wipe_ticks = 0;
        self.arena_entry = None;
        self.camera = Camera::at_player(self.checkpoint, (self.level.height * 8) as f32);
    }
}
#[derive(PartialEq, Eq)]
pub(crate) enum RunResult {
    Continue,
    Complete,
    GameOver,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn world() -> World {
        let assets = crate::assets::Assets::load(&crate::assets::default_root()).unwrap();
        let mut world = World::new(assets.level(0, false).unwrap().clone(), 1);
        world.props.clear();
        world.enemies.clear();
        world.level.collision.fill(0);
        world.level.metatiles.fill(0);
        world
    }
    #[test]
    fn first_level_wall_torches_do_not_hurt_any_player() {
        let assets = crate::assets::Assets::load(&crate::assets::default_root()).unwrap();
        for hard in [false, true] {
            for count in 1..=4 {
                let mut world = World::new(assets.level(0, hard).unwrap().clone(), count);
                world.props.retain(|prop| prop.kind == PropKind::Flame);
                assert_eq!(world.props.len(), 8);
                for index in 0..world.props.len() {
                    let position = world.props[index].body.position;
                    for health in [1, 2] {
                        for player in &mut world.players {
                            *player = Player::new(player.id, position);
                            player.health = health;
                            player.invulnerability = 0;
                        }
                        let mut events = Vec::new();
                        for tick in 0..256 {
                            world.tick = tick;
                            assert!(!world.update_props([InputFrame::default(); 4], &mut events));
                        }
                        for player in &world.players {
                            assert_eq!(
                                player.health, health,
                                "hard={hard}, torch={index}, player={}",
                                player.id
                            );
                            assert_eq!(player.state, PlayerState::Alive);
                            assert_eq!(player.invulnerability, 0);
                        }
                        assert!(events.is_empty());
                        assert!(world.props[index].active);
                    }
                }
            }
        }
    }
    #[test]
    fn wall_torches_allow_player_contact_and_still_extinguish_when_shot() {
        let assets = crate::assets::Assets::load(&crate::assets::default_root()).unwrap();
        for hard in [false, true] {
            let level = assets.level(0, hard).unwrap();
            for spawn in level.spawns.iter().filter(|s| s.entity == Entity::Flame) {
                for charged in [false, true] {
                    let mut world = World::new(level.clone(), 1);
                    world.enemies.clear();
                    world.props.retain(|p| p.kind == PropKind::Flame);
                    world.camera.position.y = spawn.y - 120.;
                    let player = &mut world.players[0];
                    player.body.position = Vec2::new(spawn.x - 4., spawn.y - 4.);
                    player.invulnerability = 0;
                    let mut events = Vec::new();
                    // Keep the actual level terrain and run the complete world
                    // update, so tile damage cannot hide behind a prop-only test.
                    world.step([InputFrame::default(); 4], &mut events);
                    assert_eq!(world.players[0].state, PlayerState::Alive);
                    assert_eq!(world.players[0].health, 1);
                    assert!(!events.contains(&GameEvent::Sound(Sound::Hurt)));
                    events.clear();
                    world.projectiles.push(Projectile::spell(
                        0,
                        Vec2::new(spawn.x - 6., spawn.y),
                        Vec2::new(1., 0.),
                        charged,
                    ));
                    world.update_projectiles(&mut events);
                    assert_eq!(world.props.iter().filter(|p| !p.active).count(), 1);
                    assert_eq!(events, vec![GameEvent::Sound(Sound::Extinguish)]);
                    events.clear();
                    world.update_projectiles(&mut events);
                    assert!(events.is_empty(), "extinguishing must happen only once");
                }
            }
        }
    }
    #[test]
    fn segmented_boss_lift_matches_original_player_trace() {
        let assets = crate::assets::Assets::load(&crate::assets::default_root()).unwrap();
        let mut world = World::new(assets.level(11, true).unwrap().clone(), 1);
        world.enemies.clear();
        world.props.clear();
        world.camera.position.y = 0.0;
        world.players[0] = Player::new(0, Vec2::new(124.0, 144.0));
        // The isolated original probe retained its collision ring when resetting
        // the camera: a solid floor at y176, rather than the arena's initial map.
        world.level.collision[22 * 32..23 * 32].fill(1);
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/segmented_clear_reference.json"
        ))
        .unwrap();
        for row in reference["frames"].as_array().unwrap() {
            let tick = row[0].as_u64().unwrap();
            if row[7] != 11 {
                break;
            }
            world.step_after_clear([InputFrame::default(); 4], &mut Vec::new());
            if tick == 211 {
                world.begin_segmented_ascent();
            }
            if tick >= 212 {
                world.lift_after_segmented_boss();
            }
            let player = &world.players[0];
            assert_eq!(
                (player.body.center().y * 256.0) as i64,
                row[2].as_i64().unwrap(),
                "y tick {tick}"
            );
            if row[1] != 0 {
                assert_eq!(
                    (player.body.velocity.y * 256.0) as i64,
                    row[3].as_i64().unwrap(),
                    "velocity tick {tick}"
                );
            }
            assert_eq!(
                player.state == PlayerState::Dead,
                row[1] == 0,
                "removed tick {tick}"
            );
        }
    }

    #[test]
    fn boss_clear_delay_and_cleanup_follow_source_descriptors() {
        for (boss_kind, ticks) in [
            (BossKind::Inhaling, 510),
            (BossKind::Summoning, 510),
            (BossKind::Sorcerer, 510),
            (BossKind::Segmented, 510),
            (BossKind::Final, 140),
        ] {
            let mut world = world();
            let mut boss = Enemy::new(EnemyKind::Boss(boss_kind), Vec2::new(100., 80.), false);
            boss.health = 0;
            world.enemies.push(boss);
            for kind in [
                EnemyKind::Turret,
                EnemyKind::Willowisp,
                EnemyKind::Chaser,
                EnemyKind::SummonedWalker,
                EnemyKind::DiagonalFlyer,
                EnemyKind::Minion,
                EnemyKind::Leaper,
                EnemyKind::Lightning,
                EnemyKind::SummoningPlatform,
                EnemyKind::Goat,
                EnemyKind::Troll,
                EnemyKind::Snake,
                EnemyKind::Walker,
                EnemyKind::Bomb,
                EnemyKind::Hopper,
                EnemyKind::Spike,
                EnemyKind::Spiderweb,
            ] {
                world
                    .enemies
                    .push(Enemy::new(kind, Vec2::new(80., 80.), false));
            }
            assert_eq!(world.begin_stage_clear(), ticks);
            assert!(world.enemies[1..10].iter().all(|enemy| !enemy.is_alive()));
            assert!(world.enemies[10..].iter().all(Enemy::is_alive));
            assert_eq!(world.particles.len(), 9);
            assert!(
                world
                    .particles
                    .iter()
                    .all(|effect| effect.source_kind == 50 && effect.ticks_left == 11)
            );
        }
        assert_eq!(world().begin_stage_clear(), 256);
    }

    #[test]
    fn each_segment_awards_its_shooter_and_consumes_charged_spells() {
        let mut world = world();
        let boss = Enemy::new(EnemyKind::Boss(BossKind::Segmented), Vec2::default(), false);
        let center = boss.segments[0].center();
        world.enemies.push(boss);
        world
            .projectiles
            .push(Projectile::spell(0, center, Vec2::default(), true));
        let mut events = Vec::new();
        world.update_projectiles(&mut events);
        assert_eq!(world.enemies[0].health, 29);
        assert_eq!(world.enemies[0].segments_hit, 1);
        assert_eq!(world.player_scores, [1, 0, 0, 0]);
        assert_eq!(world.score, 1);
        assert!(world.projectiles.is_empty());
        assert_eq!(events, vec![GameEvent::Sound(Sound::DestroyEnemy)]);
    }

    #[test]
    fn segmented_clear_reward_is_only_for_zero_shot_completion() {
        for shots in [0, 1, 30] {
            let mut world = world();
            let mut boss = Enemy::new(EnemyKind::Boss(BossKind::Segmented), Vec2::default(), false);
            boss.health = 0;
            boss.segments_hit = shots;
            world.enemies.push(boss);
            assert_eq!(world.segmented_reward_pending(), shots == 0);
            if shots == 0 {
                let reward = &world.props[0];
                assert_eq!(reward.source_kind, 54);
                assert_eq!(reward.kind, PropKind::Gem(4));
                assert_eq!(reward.body.center(), Vec2::new(128., 168.));
                assert!(world.segmented_reward_pending());
                assert_eq!(world.props.len(), 1);
                world.props[0].active = false;
                assert!(!world.segmented_reward_pending());
                assert_eq!(world.props.len(), 1);
            } else {
                assert!(world.props.is_empty());
            }
        }
    }

    #[test]
    fn depleted_willowisp_stays_in_world_until_a_final_projectile_hit() {
        let mut world = world();
        world.camera.position.y = 0.;
        world.players[0].body.position = Vec2::new(200., 180.);
        let mut enemy = Enemy::new(EnemyKind::Willowisp, Vec2::new(124., 68.), false);
        enemy.health = 2;
        world.enemies.push(enemy);
        for tick in 1..=5 {
            world.tick = tick;
            world.update_enemies(&mut Vec::new());
        }
        assert_eq!(world.enemies[0].health, 0);
        assert!(world.enemies[0].is_alive());
        let position = world.enemies[0].body.center();
        world
            .projectiles
            .push(Projectile::spell(0, position, Vec2::new(0., 0.), false));
        world.update_projectiles(&mut Vec::new());
        assert!(!world.enemies[0].is_alive());
    }

    #[test]
    fn stationary_enemy_anchors_match_source_even_without_supporting_ground() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/anchored_enemy_reference.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let mut world = world();
            world.level.hard = case["hard"].as_bool().unwrap();
            world.camera.position.y = 0.;
            world.players[0].body.position = Vec2::new(196., 196.);
            let kind = if case["kind"].as_u64().unwrap() == 7 {
                EnemyKind::Goat
            } else {
                EnemyKind::Troll
            };
            world
                .enemies
                .push(Enemy::new(kind, Vec2::new(124., 68.), world.level.hard));
            for row in case["frames"].as_array().unwrap() {
                world.tick = row[0].as_u64().unwrap();
                world.update_enemies(&mut Vec::new());
                let center = world.enemies[0].body.center();
                assert_eq!(center.x, row[1].as_f64().unwrap() as f32);
                assert_eq!(
                    center.y,
                    row[2].as_f64().unwrap() as f32,
                    "{kind:?} tick {}: stationary source actors retain their map anchor",
                    world.tick
                );
            }
        }
    }

    #[test]
    fn ground_patrol_turns_at_platform_edges_without_falling() {
        for kind in [EnemyKind::Walker, EnemyKind::Bomb] {
            let mut world = world();
            world.camera.position.y = 0.;
            world.players[0].body.position = Vec2::new(220., 140.);
            let width = world.level.width;
            world.level.collision[20 * width + 12..20 * width + 16].fill(1);
            let mut enemy = Enemy::new(kind, Vec2::new(100., 148.), false);
            enemy.facing = 1.;
            world.enemies.push(enemy);
            for tick in 1..=512 {
                world.tick = tick;
                world.update_enemies(&mut Vec::new());
                let position = world.enemies[0].body.position;
                assert_eq!(position.y, 148.);
                assert!((92. ..=120.).contains(&position.x), "{kind:?} {position:?}");
            }
        }
    }

    #[test]
    fn ground_enemy_speed_is_preserved_by_world_physics() {
        for hard in [false, true] {
            for (kind, distance) in [(EnemyKind::Walker, 16.), (EnemyKind::Bomb, 8.)] {
                let mut world = world();
                world.level.hard = hard;
                world.camera.position.y = 0.;
                world.players[0].body.position = Vec2::new(220., 140.);
                let width = world.level.width;
                world.level.collision[20 * width..21 * width].fill(1);
                let mut enemy = Enemy::new(kind, Vec2::new(100., 148.), hard);
                enemy.facing = 1.;
                world.enemies.push(enemy);
                for tick in 1..=64 {
                    world.tick = tick;
                    world.update_enemies(&mut Vec::new());
                }
                assert_eq!(
                    world.enemies[0].body.position.x,
                    100. + distance,
                    "{kind:?} hard={hard}: world integration must not bypass patrol cadence"
                );
            }
        }
    }

    #[test]
    fn offscreen_frozen_minions_thaw_cleanup_and_release_sorcerer_spawn_cap() {
        let mut world = world();
        world.level.hard = true;
        world.camera.position.y = 0.;
        world.players[0].body.position = Vec2::new(60., 188.);
        world.players[0].invulnerability = u16::MAX;
        let mut boss = Enemy::new(
            EnemyKind::Boss(BossKind::Sorcerer),
            Vec2::new(112., 56.),
            true,
        );
        boss.hit(70);
        world.enemies.push(boss);
        for tick in 1..=250 {
            world.tick = tick;
            world.update_enemies(&mut Vec::new());
        }
        for x in [-100., 350.] {
            let mut minion = Enemy::new(EnemyKind::Minion, Vec2::new(x, -100.), true);
            minion.freeze(190);
            world.enemies.push(minion);
        }
        for tick in 251..=282 {
            world.tick = tick;
            world.update_enemies(&mut Vec::new());
        }
        assert_eq!(
            world
                .enemies
                .iter()
                .filter(|e| e.kind == EnemyKind::Minion && e.health > 0)
                .count(),
            2
        );
        assert!(
            world.enemies[1..]
                .iter()
                .all(|e| e.frozen_ticks == 158 && e.health > 0)
        );
        for tick in 283..=410 {
            world.tick = tick;
            world.update_enemies(&mut Vec::new());
        }
        assert_eq!(world.enemies[1].health, 0);
        assert_eq!(world.enemies[2].health, 0);
        assert_eq!(
            world
                .enemies
                .iter()
                .filter(|e| e.kind == EnemyKind::Minion && e.health > 0)
                .count(),
            1,
            "Sorcerer must resume summoning after escaped actors leave its source slot count"
        );
    }

    #[test]
    fn minion_cleanup_uses_source_center_bounds_after_movement() {
        for (center, removed) in [
            (Vec2::new(7., 0.), false),
            (Vec2::new(248.99, 239.99), false),
            (Vec2::new(6.99, 100.), true),
            (Vec2::new(249., 100.), true),
            (Vec2::new(128., -0.01), true),
            (Vec2::new(128., 240.), true),
        ] {
            let mut world = world();
            world.camera.position.y = 0.;
            let mut minion = Enemy::new(
                EnemyKind::Minion,
                Vec2::new(center.x - 4., center.y - 4.),
                false,
            );
            minion.set_target(Some(0));
            world.enemies.push(minion);
            world.tick = 1;
            world.update_enemies(&mut Vec::new());
            assert_eq!(world.enemies[0].health == 0, removed, "center={center:?}");
        }
    }

    #[test]
    fn terrain_hazards_share_damage_but_only_picture_one_knocks_back() {
        for (picture, facing) in [(1, 1), (1, -1), (2, 1), (2, -1)] {
            let mut world = world();
            world.camera.position.y = 0.;
            let player = &mut world.players[0];
            player.body.position = Vec2::new(124., 96.);
            player.body.velocity = Vec2::new(1., 0.);
            player.facing = facing;
            player.health = 2;
            let cell = 12 * world.level.width + 16;
            world.level.tiles[cell] = picture;
            world.level.collision[cell] = 64;
            world.step([InputFrame::default(); 4], &mut Vec::new());
            let player = &world.players[0];
            assert_eq!(player.health, 1, "picture {picture}");
            assert_eq!(player.invulnerability, 110);
            let expected = if picture == 1 {
                -f32::from(facing) * 1.5
            } else {
                225. / 256.
            };
            assert_eq!(
                player.body.velocity.x, expected,
                "picture {picture}, facing {facing}"
            );
        }
    }
    #[test]
    fn projectile_opens_chest_and_spawns_one_collectible_reward() {
        let mut world = world();
        world.level.metatiles[10 * 16 + 5] = 17;
        world.level.collision[21 * 32 + 10] = 17;
        world.level.collision[21 * 32 + 11] = 17;
        let mut events = Vec::new();
        for _ in 0..2 {
            world.projectiles.push(Projectile::spell(
                0,
                Vec2::new(74.0, 169.0),
                Vec2::new(1.0, 0.0),
                false,
            ));
            world.update_projectiles(&mut events);
        }
        assert_eq!(world.level.metatiles[10 * 16 + 5], 18);
        assert_eq!(world.rewards.broken_blocks(), 1);
        assert_eq!(world.props.len(), 1);
        assert_eq!(world.props[0].kind, PropKind::Feather);
        assert_eq!(world.props[0].body.center(), Vec2::new(86.0, 164.0));
        assert_eq!(world.props[0].body.velocity, Vec2::default());
        assert_eq!(
            events
                .iter()
                .filter(|&&e| e == GameEvent::Sound(Sound::OpenChest))
                .count(),
            1
        );
    }
    #[test]
    fn fairy_revives_only_collecting_ghost_with_one_health() {
        let mut world = world();
        world.players[0].damage();
        for _ in 0..128 {
            world.players[0].update(InputFrame::default(), 0);
        }
        let position = world.players[0].body.position;
        world
            .players
            .push(Player::new(1, Vec2::new(position.x + 100.0, position.y)));
        world.players[1].damage();
        for _ in 0..128 {
            world.players[1].update(InputFrame::default(), 0);
        }
        world.spawn_pickup(PickupKind::Fairy, position);
        world.update_props([InputFrame::default(); 4], &mut Vec::new());
        assert_eq!(world.players[0].state, PlayerState::Alive);
        assert_eq!(world.players[0].health, 1);
        assert_eq!(world.players[1].state, PlayerState::Ghost);
    }
    #[test]
    fn blocked_headroom_prevents_ghost_revival() {
        let mut world = world();
        world.camera.position.y = 0.0;
        world.players[0].body.position = Vec2::new(80.0, 80.0);
        world.players[0].damage();
        for _ in 0..128 {
            world.players[0].update(InputFrame::default(), 0);
        }
        world.level.collision[9 * 32 + 10] = 1;
        world.spawn_pickup(PickupKind::Fairy, Vec2::new(80.0, 80.0));
        world.update_props([InputFrame::default(); 4], &mut Vec::new());
        assert_eq!(world.players[0].state, PlayerState::Ghost);
        assert!(world.props[0].active);
    }
    #[test]
    fn ghost_projectile_freezes_enemy_without_damage_or_contact_harm() {
        let mut world = world();
        let position = world.players[0].body.position;
        world
            .enemies
            .push(Enemy::new(EnemyKind::Walker, position, false));
        let mut shot = Projectile::spell(0, position, Vec2::default(), false);
        shot.freezes = true;
        shot.damage = 0;
        world.projectiles.push(shot);
        world.update_projectiles(&mut Vec::new());
        let health = world.enemies[0].health;
        assert_eq!(world.enemies[0].frozen_ticks, 190);
        world.update_enemies(&mut Vec::new());
        assert_eq!(world.players[0].state, PlayerState::Alive);
        assert_eq!(world.enemies[0].health, health);
    }
    #[test]
    fn shared_pickup_is_collected_exactly_once_by_overlapping_players() {
        let mut world = world();
        let position = world.players[0].body.position;
        for id in 1..4 {
            world.players.push(Player::new(id, position));
        }
        world.spawn_pickup(PickupKind::SmallGem, position);
        let mut events = Vec::new();
        world.update_props([InputFrame::default(); 4], &mut events);
        assert_eq!(world.score, PickupKind::SmallGem.score_units());
        assert_eq!(world.player_scores, [1, 0, 0, 0]);
        assert_eq!(
            events
                .iter()
                .filter(|&&e| e == GameEvent::Sound(Sound::Emerald))
                .count(),
            1
        );
    }

    #[test]
    fn enemy_score_belongs_to_the_player_who_fired_the_killing_shot() {
        let mut world = world();
        let position = world.players[0].body.position;
        let mut enemy = Enemy::new(EnemyKind::Walker, position, false);
        enemy.health = 1;
        let award = u32::from(enemy.score_units());
        world.enemies.push(enemy);
        world
            .projectiles
            .push(Projectile::spell(3, position, Vec2::default(), false));
        world.update_projectiles(&mut Vec::new());
        assert_eq!(world.enemies[0].health, 0);
        assert_eq!(world.player_scores, [0, 0, 0, award]);
        assert_eq!(world.score, award);
        world.update_projectiles(&mut Vec::new());
        assert_eq!(world.player_scores, [0, 0, 0, award]);
    }

    #[test]
    fn respawn_restores_terrain_and_entities_but_preserves_run_progress() {
        let assets = crate::assets::Assets::load(&crate::assets::default_root()).unwrap();
        for hard in [false, true] {
            for count in 1..=4 {
                let level = assets.level(0, hard).unwrap().clone();
                let mut world = World::new(level.clone(), count);
                let enemy_count = world.enemies.len();
                let prop_count = world.props.len();
                world.level.collision.fill(0);
                world.enemies.clear();
                world.props.clear();
                world.score = 1234;
                world.lives = 1;
                world.checkpoint = Vec2::new(80.0, 600.0);
                world.respawn();
                assert_eq!(world.level.collision, level.collision);
                assert_eq!(world.enemies.len(), enemy_count);
                assert_eq!(world.props.len(), prop_count);
                assert_eq!((world.score, world.lives), (1234, 1));
                for player in &world.players {
                    assert_eq!(
                        player.body.position,
                        Vec2::new(80.0 + (count - 1 - player.id) as f32 * 8.0, 600.0)
                    );
                    assert_eq!(player.state, PlayerState::Alive);
                    assert_eq!(player.invulnerability, 110);
                }
            }
        }
    }

    #[test]
    fn swap_automatically_targets_highest_living_teammate() {
        let mut world = world();
        let start = world.players[0].body.position;
        world.players.push(Player::new(1, Vec2::new(80.0, 800.0)));
        world.players.push(Player::new(2, Vec2::new(100.0, 600.0)));
        world.players.push(Player::new(3, Vec2::new(140.0, 400.0)));
        world.players[3].damage();
        for _ in 0..128 {
            world.players[3].update(InputFrame::default(), 0);
        }
        world.spawn_pickup(PickupKind::Swap, start);
        world.update_props([InputFrame::default(); 4], &mut Vec::new());
        assert!(world.players[0].is_warping());
        assert!(world.players[2].is_warping());
        assert!(!world.players[1].is_warping());
        assert!(!world.players[3].is_warping());
        assert!(!world.props[0].active);
    }

    #[test]
    fn extra_stage_fires_right_at_four_pixels_regardless_of_aim() {
        let assets = crate::assets::Assets::load(&crate::assets::default_root()).unwrap();
        for (stage, expected) in [(0, -3.0), (13, 4.0)] {
            let mut world = World::new(assets.level(stage, true).unwrap().clone(), 4);
            for id in 0..4 {
                world.player_events(
                    id,
                    vec![PlayerEvent::Shot {
                        direction: Vec2::new(-1.0, 0.0),
                        charged: false,
                    }],
                    &mut Vec::new(),
                );
                let shot = world.projectiles.last().unwrap();
                assert_eq!(shot.body.center(), world.players[id].body.center());
                assert_eq!(shot.body.velocity.x, expected);
                assert_eq!(shot.body.velocity.y, 0.0);
                assert_eq!(shot.owner, Owner::Player(id));
            }
        }
    }

    #[test]
    fn final_arena_players_fly_without_a_floor_and_respawn_with_two_health() {
        let assets = crate::assets::Assets::load(&crate::assets::default_root()).unwrap();
        for count in 1..=4 {
            let mut world = World::new(assets.level(13, true).unwrap().clone(), count);
            assert!(world.level.collision.iter().all(|&cell| cell == 0));
            world.enemies.clear();
            let positions: Vec<_> = world.players.iter().map(|p| p.body.position).collect();
            for _ in 0..600 {
                world.step([InputFrame::default(); 4], &mut Vec::new());
            }
            for (player, position) in world.players.iter().zip(positions) {
                assert!(player.flying());
                assert_eq!(player.health, 2);
                assert_eq!(player.body.position, position);
            }
            for player in &mut world.players {
                player.health = 0;
                player.state = PlayerState::Dead;
                player.feather = true;
            }
            world.respawn();
            for player in &world.players {
                assert!(player.flying());
                assert_eq!(player.health, 2);
                assert!(!player.feather);
            }
        }
    }

    #[test]
    fn arena_door_gathers_revives_and_lifts_all_players_before_combat() {
        let assets = crate::assets::Assets::load(&crate::assets::default_root()).unwrap();
        for hard in [false, true] {
            for count in 1..=4 {
                let mut world = World::new(assets.level(2, hard).unwrap().clone(), count);
                world.players[0].body.position = Vec2::new(124.0, 296.0);
                for player in world.players.iter_mut().skip(1) {
                    player.damage();
                }
                let mut events = Vec::new();
                assert!(!world.update_props([InputFrame::default(); 4], &mut events));
                assert!(
                    world.arena_entry.is_some(),
                    "door senses a player below its solid ceiling"
                );
                assert!(
                    world
                        .enemies
                        .iter()
                        .filter(|e| matches!(e.kind, EnemyKind::Boss(_)))
                        .all(|e| !e.can_accept_hit())
                );
                for _ in 0..1000 {
                    if world.arena_entry.is_none() {
                        break;
                    }
                    assert!(
                        world.step([InputFrame::default(); 4], &mut events) == RunResult::Continue
                    );
                }
                assert!(world.arena_entry.is_none());
                assert_eq!(world.camera.position.y, 0.0);
                assert_eq!(world.level.metatiles[12 * 16 + 7], 19);
                assert_eq!(world.level.metatiles[12 * 16 + 8], 19);
                for player in &world.players {
                    assert_eq!(player.state, PlayerState::Alive);
                    assert_eq!(player.body.position.y, 184.0);
                    assert!(!player.is_warping());
                }
                assert!(events.contains(&GameEvent::Music(crate::events::Music::Boss)));
            }
        }
    }

    #[test]
    fn absorbed_charged_spells_do_not_pierce_or_emit_damage_feedback() {
        for kind in [EnemyKind::Bomb, EnemyKind::Boss(BossKind::Inhaling)] {
            let mut world = world();
            let mut enemy = Enemy::new(kind, Vec2::new(80., 80.), false);
            if matches!(kind, EnemyKind::Boss(_)) {
                enemy.invulnerable_ticks = 20;
            } else {
                enemy.health = 4;
            }
            let health = enemy.health;
            let center = enemy.body.center();
            world.enemies.push(enemy);
            world
                .projectiles
                .push(Projectile::spell(0, center, Vec2::default(), true));
            let mut events = Vec::new();
            world.update_projectiles(&mut events);
            assert!(world.projectiles.is_empty());
            assert_eq!(world.enemies[0].health, health);
            assert!(events.is_empty());
        }
    }

    #[test]
    fn armed_bombs_pass_lethal_spells_but_accept_sublethal_damage() {
        let mut world = world();
        let mut bomb = Enemy::new(EnemyKind::Bomb, Vec2::new(80., 80.), false);
        bomb.health = 4;
        bomb.hit(6);
        let center = bomb.body.center();
        world.enemies.push(bomb);
        world
            .projectiles
            .push(Projectile::spell(0, center, Vec2::default(), true));
        world.update_projectiles(&mut Vec::new());
        assert_eq!(world.enemies[0].health, 4);
        assert!(world.projectiles[0].charged);
        world.projectiles.clear();
        world
            .projectiles
            .push(Projectile::spell(0, center, Vec2::default(), false));
        world.update_projectiles(&mut Vec::new());
        assert_eq!(world.enemies[0].health, 2);
        assert!(world.projectiles.is_empty());
    }

    #[test]
    fn charged_spell_pierces_a_weak_enemy_then_becomes_ordinary() {
        let mut world = world();
        world
            .enemies
            .push(Enemy::new(EnemyKind::Chaser, Vec2::new(80.0, 80.0), false));
        let center = world.enemies[0].body.center();
        world
            .projectiles
            .push(Projectile::spell(0, center, Vec2::default(), true));
        world.update_projectiles(&mut Vec::new());
        assert_eq!(world.enemies[0].health, 0);
        assert_eq!(world.projectiles.len(), 1);
        assert!(!world.projectiles[0].charged);
        assert_eq!(world.projectiles[0].damage, 2);
    }

    #[test]
    fn hostile_shots_cross_terrain_but_expire_at_viewport_edges() {
        let mut world = world();
        world.camera.position.y = 0.0;
        world.level.collision.fill(1);
        world
            .projectiles
            .push(Projectile::hostile(crate::enemy::EnemyAttack {
                kind: crate::enemy::ProjectileKind::Orb,
                position: Vec2::new(80.0, 80.0),
                velocity: Vec2::new(2.0, 0.0),
            }));
        world.update_projectiles(&mut Vec::new());
        assert_eq!(world.projectiles[0].body.center(), Vec2::new(82.0, 80.0));
        for _ in 0..100 {
            world.update_projectiles(&mut Vec::new());
        }
        assert!(world.projectiles.is_empty());
    }
    #[test]
    fn freeze_shots_pass_stone_but_damage_and_break_reinforced_blocks() {
        let mut world = world();
        world.level.collision[10 * 32 + 10] = 1;
        world.projectiles.push(Projectile::freeze(
            0,
            Vec2::new(78.0, 84.0),
            Vec2::new(1.0, 0.0),
        ));
        world.update_projectiles(&mut Vec::new());
        assert_eq!(world.projectiles.len(), 1);
        assert_eq!(world.projectiles[0].body.center().x, 81.0);
        world.projectiles.clear();
        world.level.set_metatile(5, 5, 16);
        for expected in [16, 16, 0] {
            world.projectiles.push(Projectile::freeze(
                0,
                Vec2::new(78.0, 84.0),
                Vec2::new(1.0, 0.0),
            ));
            world.update_projectiles(&mut Vec::new());
            assert!(world.projectiles.is_empty());
            assert_eq!(world.level.metatiles[5 * 16 + 5], expected);
        }
        assert_eq!(world.rewards.broken_blocks(), 1);
    }
    #[test]
    fn vertical_currents_lift_only_living_players_inside_the_column() {
        let mut world = world();
        world.players[0].body.position = Vec2::new(96.0, 66.0);
        world.props.push(Prop {
            pickup: None,
            kind: PropKind::Updraft {
                reach: 69,
                affects_players: true,
            },
            source_kind: 31,
            body: Body::new(Vec2::new(96.0, 96.0), Vec2::new(8.0, 8.0)),
            active: true,
            flip_x: false,
        });
        world.update_props([InputFrame::default(); 4], &mut Vec::new());
        assert_eq!(world.players[0].body.velocity.y, -136.0 / 256.0);
        assert_eq!(world.players[0].body.position.x, 96.0);
        world.players[0].body.velocity.y = -2.0;
        world.update_props([InputFrame::default(); 4], &mut Vec::new());
        assert_eq!(world.players[0].body.velocity.y, -258.0 / 256.0);
        world.players[0].body.position.x = 109.0;
        world.players[0].body.velocity.y = 0.0;
        world.update_props([InputFrame::default(); 4], &mut Vec::new());
        assert_eq!(world.players[0].body.velocity.y, 0.0);
    }

    #[test]
    fn horizontal_wind_pushes_only_in_front_within_its_nineteen_pixel_band() {
        let mut world = world();
        world.players[0].body.position = Vec2::new(76.0, 96.0);
        world.props.push(Prop {
            pickup: None,
            kind: PropKind::Wind,
            source_kind: 26,
            body: Body::new(Vec2::new(96.0, 96.0), Vec2::new(8.0, 8.0)),
            active: true,
            flip_x: false,
        });
        world.update_props([InputFrame::default(); 4], &mut Vec::new());
        assert_eq!(world.players[0].body.position.x, 76.0);
        world.players[0].body.position.x = 116.0;
        world.update_props([InputFrame::default(); 4], &mut Vec::new());
        assert_eq!(world.players[0].body.position.x, 117.0);
        world.players[0].body.position.y = 106.0;
        world.update_props([InputFrame::default(); 4], &mut Vec::new());
        assert_eq!(world.players[0].body.position.x, 117.0);
    }

    #[test]
    fn player_spells_cross_one_way_platforms_in_both_directions() {
        let mut world = world();
        world.level.collision[10 * 32 + 10] = 4;
        for (y, direction) in [(78.0, 1.0), (90.0, -1.0)] {
            world.projectiles.clear();
            world.projectiles.push(Projectile::spell(
                0,
                Vec2::new(84.0, y),
                Vec2::new(0.0, direction),
                false,
            ));
            for _ in 0..4 {
                world.update_projectiles(&mut Vec::new());
            }
            assert_eq!(world.projectiles.len(), 1);
            assert_eq!(world.projectiles[0].body.center().y, y + direction * 12.0);
        }
    }
}
