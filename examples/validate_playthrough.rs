//! Legal-input search. A fresh game replays every saved completion independently.
//! This tool never writes player health, position, enemy state, or world geometry.
use micro_mages_re::{
    assets::{Assets, Level},
    game::{Game, Screen},
    player::{Player, PlayerState},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BinaryHeap, HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::Arc,
};

#[derive(Clone)]
struct Candidate {
    game: Game,
    first: Vec<[u8; 4]>,
    score: f32,
}
#[derive(Serialize, Deserialize)]
struct Replay {
    stage: usize,
    hard: bool,
    players: usize,
    buttons: Vec<[u8; 4]>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    campaign: bool,
}

/// Publish a complete snapshot atomically so independent continuation searches
/// can safely read a live planner's latest legal-input prefix.
fn write_trace(
    path: impl AsRef<std::path::Path>,
    replay: &Replay,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.as_ref();
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&temporary, serde_json::to_vec(replay)?)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

struct Navigation {
    distance: Vec<u32>,
    width: usize,
    height: usize,
    waypoint: Option<(f32, f32)>,
}
impl Navigation {
    fn new(level: &Level) -> Self {
        Self::targeted(level, None)
    }
    fn targeted(level: &Level, waypoint: Option<(f32, f32)>) -> Self {
        let width = level.width;
        let height = level.height;
        let mut distance = vec![u32::MAX; width * height];
        let mut queue = VecDeque::new();
        let targets: Vec<_> = level
            .spawns
            .iter()
            .filter(|s| {
                matches!(
                    s.entity,
                    micro_mages_re::assets::Entity::LevelExit
                        | micro_mages_re::assets::Entity::BossEntrance
                )
            })
            .collect();
        if let Some((x, y)) = waypoint {
            let x = (x / 8.0) as usize;
            let y = (y / 8.0) as usize;
            if x < width && y < height {
                distance[y * width + x] = 0;
                queue.push_back((x, y));
            }
        } else if targets.is_empty() {
            for (x, cell) in distance.iter_mut().take(width).enumerate() {
                *cell = 0;
                queue.push_back((x, 0));
            }
        } else {
            for target in targets {
                let x = (target.x / 8.0) as usize;
                let y = (target.y / 8.0) as usize;
                if x < width && y < height {
                    distance[y * width + x] = 0;
                    queue.push_back((x, y));
                }
            }
        }
        while let Some((x, y)) = queue.pop_front() {
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                    continue;
                }
                let ni = ny as usize * width + nx as usize;
                let material = level.collision[ni];
                if (material & 1 != 0 && material & 16 == 0) || material & 64 != 0 {
                    continue;
                }
                let nd = distance[y * width + x] + 1;
                if distance[ni] > nd {
                    distance[ni] = nd;
                    queue.push_back((nx as usize, ny as usize));
                }
            }
        }
        Self {
            distance,
            width,
            height,
            waypoint,
        }
    }
    fn cost(&self, x: f32, y: f32) -> f32 {
        let col = ((x / 8.0) as usize).min(self.width - 1);
        let row = ((y / 8.0) as usize).min(self.height - 1);
        let distance = self.distance[row * self.width + col];
        if distance == u32::MAX {
            y / 8.0 + 100.0
        } else {
            distance as f32
        }
    }
}
fn contact_penalty(
    player: &micro_mages_re::physics::Body,
    enemy: &micro_mages_re::physics::Body,
) -> f32 {
    let p = player.center();
    let e = enemy.center();
    [0.0, 12.0, 24.0]
        .into_iter()
        .map(|ticks| {
            let dx = (e.x + enemy.velocity.x * ticks - p.x - player.velocity.x * ticks).abs();
            let dy = (e.y + enemy.velocity.y * ticks - p.y - player.velocity.y * ticks).abs();
            let overlap_x = (player.size.x + enemy.size.x) * 0.5 + 8.0 - dx;
            let overlap_y = (player.size.y + enemy.size.y) * 0.5 + 8.0 - dy;
            overlap_x.min(overlap_y).max(0.0) * 16.0
        })
        .fold(0.0, f32::max)
}
fn score(game: &Game, nav: &Navigation) -> f32 {
    if game.screen == Screen::StageClear {
        return 1_000_000.0;
    }
    if game.screen != Screen::Playing {
        return -1_000_000.0;
    }
    let world = game.world.as_ref().unwrap();
    let mut best = -1_000_000.0f32;
    let mut team_danger = 0.0;
    for player in &world.players {
        if player.state != PlayerState::Alive {
            continue;
        }
        let p = player.body.center();
        let arena = world
            .level
            .spawns
            .iter()
            .any(|spawn| spawn.entity == micro_mages_re::assets::Entity::BossEntrance);
        let progress = if player.flight_mode {
            let alignment = world
                .enemies
                .iter()
                .filter(|e| {
                    matches!(e.kind, micro_mages_re::enemy::EnemyKind::Boss(_))
                        && e.can_accept_hit()
                })
                .map(|e| {
                    let target = e.body.center();
                    (target.y - p.y).abs() * 2.0 + (target.x - p.x - 80.0).abs()
                })
                .fold(0.0f32, f32::max);
            2000.0 - alignment * 4.0
        } else if arena && p.y < 320.0 {
            if p.y < 240.0 {
                let alignment = world
                    .enemies
                    .iter()
                    .filter(|e| {
                        matches!(e.kind, micro_mages_re::enemy::EnemyKind::Boss(_))
                            && e.can_accept_hit()
                    })
                    .map(|e| {
                        let target = e.body.center();
                        let delta =
                            micro_mages_re::physics::Vec2::new(target.x - p.x, target.y - p.y);
                        let face = if delta.x.signum() == f32::from(player.facing) {
                            0.0
                        } else {
                            80.0
                        };
                        (delta.x.abs() + (delta.y.abs() - 60.0).abs())
                            .min(delta.y.abs() * 2.0 + face)
                    })
                    .fold(0.0f32, f32::max);
                2000.0 - alignment * 4.0
            } else {
                2000.0 - p.y * 4.0
            }
        } else {
            -(nav.cost(p.x, p.y) * 16.0) - p.y * 0.1
        };
        let danger = world
            .enemies
            .iter()
            .filter(|e| matches!(e.kind, micro_mages_re::enemy::EnemyKind::Boss(_)) && e.health > 0)
            .map(|enemy| {
                let protected_lane = if enemy.arena_active() && !enemy.can_accept_hit() {
                    (40.0 - (enemy.body.center().y - p.y).abs()).max(0.0) * 8.0
                } else {
                    0.0
                };
                contact_penalty(&player.body, &enemy.body) + protected_lane
            })
            .sum::<f32>();
        team_danger += danger;
        let value = progress + player.health as f32 * 250.0 - danger;
        best = best.max(value);
    }
    let remaining_boss_health: u32 = world
        .enemies
        .iter()
        .filter(|e| matches!(e.kind, micro_mages_re::enemy::EnemyKind::Boss(_)))
        .map(|e| e.health as u32)
        .sum();
    let team_survival = if world.players.len() > 1
        && (world.camera.position.y == 0.0 || world.level.height == 32)
    {
        world
            .players
            .iter()
            .filter(|p| p.state == PlayerState::Alive)
            .map(|p| 600.0 + f32::from(p.health) * 150.0)
            .sum()
    } else {
        0.0
    };
    best + team_survival - team_danger * 0.5
        + world.lives as f32 * 4000.0
        + world.score as f32 * 0.01
        - remaining_boss_health as f32 * 100.0
}
fn action_count(game: &Game) -> usize {
    if game
        .world
        .as_ref()
        .unwrap()
        .enemies
        .iter()
        .any(|enemy| matches!(enemy.kind, micro_mages_re::enemy::EnemyKind::Boss(_)))
    {
        let world = game.world.as_ref().unwrap();
        if world
            .players
            .iter()
            .filter(|p| p.state == PlayerState::Alive)
            .count()
            > 1
            && world.camera.position.y == 0.0
        {
            81
        } else {
            27
        }
    } else {
        24
    }
}
fn action(
    game: &mut Game,
    id: usize,
    ticks: usize,
    players: usize,
    solo_climb: bool,
) -> Vec<[u8; 4]> {
    if game.screen != Screen::Playing {
        return Vec::new();
    }
    let spread = id >= 27;
    let aim_team = id >= 54;
    let id = id % 27;
    let horizontal = [0, 64, 128][id % 3];
    let mode = id / 3;
    let mut result = Vec::new();
    for tick in 0..ticks {
        // A release tick between macros allows repeated ground/wall jumps.
        let jump = matches!(mode, 6..=8) || (matches!(mode, 1 | 2 | 4) && tick != 0);
        let aim = match mode {
            2 | 3 | 7 => 16,
            4 | 5 | 8 => 32,
            _ => 0,
        };
        // Hold beyond the original 55-tick charge threshold: reinforced terrain
        // and boss damage require charged shots as well as ordinary presses.
        let fire = if game.frame % 60 < 58 { 2 } else { 0 };
        let mask = horizontal | aim | fire | u8::from(jump);
        let mut buttons = [0; 4];
        buttons[..players].fill(mask);
        if spread {
            let leader = leading_player(game).id;
            for (index, buttons) in buttons.iter_mut().take(players).enumerate() {
                if index != leader && index % 2 != leader % 2 {
                    let reversed = match horizontal {
                        64 => 128,
                        128 => 64,
                        _ => 0,
                    };
                    *buttons = (*buttons & !192) | reversed;
                }
            }
        }
        if aim_team {
            let world = game.world.as_ref().unwrap();
            let leader = leading_player(game).id;
            if let Some(target) = world.enemies.iter().find(|e| {
                matches!(e.kind, micro_mages_re::enemy::EnemyKind::Boss(_)) && e.health > 0
            }) {
                let target = target.body.center();
                for (index, buttons) in buttons.iter_mut().take(players).enumerate() {
                    if index == leader {
                        continue;
                    }
                    let p = world.players[index].body.center();
                    if (target.y - p.y).abs() > (target.x - p.x).abs() {
                        *buttons = (*buttons & !48) | if target.y < p.y { 16 } else { 32 };
                    } else {
                        *buttons = (*buttons & !240) | if target.x < p.x { 64 } else { 128 };
                    }
                }
            }
        }
        if solo_climb && game.world.as_ref().unwrap().camera.position.y > 0.0 {
            let leader = leading_player(game).id;
            for (index, buttons) in buttons.iter_mut().take(players).enumerate() {
                if index != leader {
                    *buttons = fire;
                }
            }
        }
        result.push(buttons);
        game.update(buttons);
        if game.screen != Screen::Playing {
            break;
        }
    }
    result
}
fn leading_player(game: &Game) -> &Player {
    let players = &game.world.as_ref().unwrap().players;
    players
        .iter()
        .filter(|p| p.state == PlayerState::Alive)
        .min_by(|a, b| a.body.position.y.total_cmp(&b.body.position.y))
        .unwrap_or(&players[0])
}
fn key(game: &Game) -> (i16, i16, i16, i16, u8, u64) {
    let world = game.world.as_ref().unwrap();
    let p = leading_player(game);
    // Equal player motion does not imply equal gameplay state: an upward shot
    // and a horizontal shot must remain separate search branches.
    let mut fingerprint = game.frame;
    for &tile in &world.level.metatiles {
        fingerprint = fingerprint.wrapping_mul(31).wrapping_add(u64::from(tile));
    }
    for player in &world.players {
        for value in [
            ((player.body.position.x / 2.0) as i32) as u32,
            ((player.body.position.y / 2.0) as i32) as u32,
            ((player.body.velocity.x * 4.0) as i32) as u32,
            ((player.body.velocity.y * 4.0) as i32) as u32,
            u32::from(player.health),
            player.state as u32,
            u32::from(player.feather),
            u32::from(player.invulnerability / 8),
            player.facing as u32,
            u32::from(player.climbing),
            u32::from(player.flight_mode),
        ] {
            fingerprint = fingerprint.wrapping_mul(31).wrapping_add(u64::from(value));
        }
    }
    for shot in &world.projectiles {
        for value in [
            ((shot.body.position.x / 4.0) as i32) as u32,
            ((shot.body.position.y / 4.0) as i32) as u32,
            shot.body.velocity.x.to_bits(),
            shot.body.velocity.y.to_bits(),
            u32::from(shot.damage),
            u32::from(shot.ticks_left / 6),
        ] {
            fingerprint = fingerprint.wrapping_mul(31).wrapping_add(u64::from(value));
        }
    }
    for enemy in &world.enemies {
        use micro_mages_re::enemy::EnemyState;
        let state = match enemy.state {
            EnemyState::Idle => 0,
            EnemyState::Moving => 1,
            EnemyState::Attacking => 2,
            EnemyState::Fuse => 3,
            EnemyState::Defeated => 4,
            EnemyState::Boss(phase) => 5 + phase as u32,
        };
        for value in [
            ((enemy.body.position.x / 4.0) as i32) as u32,
            ((enemy.body.position.y / 4.0) as i32) as u32,
            u32::from(enemy.health),
            state,
            enemy.body.velocity.x.to_bits(),
            enemy.body.velocity.y.to_bits(),
            u32::from(enemy.invulnerable_ticks),
            u32::from(enemy.frozen_ticks),
            u32::from(enemy.boss_health_phase),
            u32::from(enemy.can_accept_hit()),
        ] {
            fingerprint = fingerprint.wrapping_mul(31).wrapping_add(u64::from(value));
        }
    }
    (
        (p.body.position.x / 4.0) as i16,
        (p.body.position.y / 4.0) as i16,
        (p.body.velocity.x * 4.0) as i16,
        (p.body.velocity.y * 4.0) as i16,
        p.health,
        fingerprint,
    )
}
struct Frontier {
    rank: i64,
    serial: usize,
    game: Game,
    buttons: Vec<[u8; 4]>,
}
impl PartialEq for Frontier {
    fn eq(&self, other: &Self) -> bool {
        self.rank == other.rank && self.serial == other.serial
    }
}
impl Eq for Frontier {}
impl PartialOrd for Frontier {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Frontier {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank
            .cmp(&other.rank)
            .then_with(|| other.serial.cmp(&self.serial))
    }
}
fn global_search(
    start: Game,
    nav: &Navigation,
    players: usize,
    limit: usize,
    solo_climb: bool,
    node_limit: usize,
    step: usize,
) -> Result<Vec<[u8; 4]>, Vec<[u8; 4]>> {
    let mut queue = BinaryHeap::new();
    let mut seen = HashSet::new();
    let mut serial = 0;
    queue.push(Frontier {
        rank: 0,
        serial,
        game: start,
        buttons: Vec::new(),
    });
    let mut best_progress = Vec::new();
    let mut least_boss_health = u32::MAX;
    let mut best_progress_score = f32::NEG_INFINITY;
    for iteration in 0..node_limit {
        let Some(current) = queue.pop() else {
            return Err(best_progress);
        };
        if current.buttons.len() >= limit {
            continue;
        }
        for id in 0..action_count(&current.game) {
            let mut game = current.game.clone();
            let segment = action(&mut game, id, step, players, solo_climb);
            if game.screen == Screen::GameOver {
                continue;
            }
            let world = game.world.as_ref().unwrap();
            let p = leading_player(&game);
            if p.state != PlayerState::Alive {
                continue;
            }
            let boss: u32 = world
                .enemies
                .iter()
                .filter(|e| matches!(e.kind, micro_mages_re::enemy::EnemyKind::Boss(_)))
                .map(|e| e.health as u32)
                .sum();
            let state = (key(&game), game.frame, world.score, boss, world.lives);
            if !seen.insert(state) {
                continue;
            }
            let mut buttons = current.buttons.clone();
            buttons.extend(segment);
            if game.screen == Screen::StageClear {
                return Ok(buttons);
            }
            let progress_score = score(&game, nav);
            if boss < least_boss_health
                || (boss == least_boss_health && progress_score > best_progress_score)
            {
                least_boss_health = boss;
                best_progress_score = progress_score;
                best_progress = buttons.clone();
            }
            let rank = ((score(&game, nav) - buttons.len() as f32 * 0.05) * 100.0) as i64;
            serial += 1;
            queue.push(Frontier {
                rank,
                serial,
                game,
                buttons,
            });
        }
        if iteration % 5000 == 0 {
            let p = leading_player(&current.game).body.position;
            eprintln!(
                "search={iteration} frontier={} xy={:.1},{:.1} ticks={} boss_health={}",
                queue.len(),
                p.x,
                p.y,
                current.buttons.len(),
                current
                    .game
                    .world
                    .as_ref()
                    .unwrap()
                    .enemies
                    .iter()
                    .filter(|e| matches!(e.kind, micro_mages_re::enemy::EnemyKind::Boss(_)))
                    .map(|e| e.health as u32)
                    .sum::<u32>()
            );
        }
        if queue.len() > 4000 {
            let mut best = queue.into_sorted_vec();
            best.reverse();
            best.truncate(2000);
            queue = BinaryHeap::from(best);
        }
    }
    eprintln!(
        "Search budget exhausted; retained {} legal continuation ticks at boss_health={least_boss_health}",
        best_progress.len()
    );
    Err(best_progress)
}
// Ghosts can support living teammates by freezing ordinary enemies. This
// controller uses the same directional and fire buttons as a human player.
fn ghost_support_input(game: &Game, player: &Player) -> u8 {
    let world = game.world.as_ref().unwrap();
    let center = player.body.center();
    let target = world
        .enemies
        .iter()
        .filter(|enemy| {
            enemy.contact_enabled()
                && (0.0..=256.0).contains(&enemy.body.center().x)
                && (world.camera.position.y..=world.camera.position.y + 240.0)
                    .contains(&enemy.body.center().y)
                && enemy.frozen_ticks == 0
                && !matches!(enemy.kind, micro_mages_re::enemy::EnemyKind::Boss(_))
        })
        .min_by(|a, b| {
            let distance = |enemy: &micro_mages_re::enemy::Enemy| {
                let target = enemy.body.center();
                (target.x - center.x).abs() + (target.y - center.y).abs()
            };
            distance(a).total_cmp(&distance(b))
        })
        .map(|enemy| enemy.body.center())
        .or_else(|| {
            world
                .players
                .iter()
                .find(|p| p.state == PlayerState::Alive)
                .map(|p| p.body.center())
        });
    let Some(target) = target else {
        return 0;
    };
    (if target.x - center.x > 10.0 {
        128
    } else if target.x - center.x < -10.0 {
        64
    } else {
        0
    }) | (if target.y - center.y > 10.0 {
        32
    } else if target.y - center.y < -10.0 {
        16
    } else {
        0
    }) | if game.frame % 12 < 6 { 2 } else { 0 }
}
// Randomized legal controller rollouts are useful when a positional heuristic
// repeatedly selects a protected boss's attack lane. The seed is reproducible.
fn random_search(
    start: Game,
    trials: usize,
    ticks: usize,
    step: usize,
    mut seed: u64,
    ghost_support: bool,
) -> Result<Vec<[u8; 4]>, Vec<[u8; 4]>> {
    let mut best_health = u32::MAX;
    let mut best = Vec::new();
    for trial in 0..trials {
        let mut game = start.clone();
        let mut buttons = Vec::new();
        let mut mask = 0;
        for tick in 0..ticks {
            if tick % step == 0 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                mask = [
                    0, 64, 128, 1, 65, 129, 16, 80, 144, 17, 81, 145, 32, 96, 160, 33, 97, 161,
                ][(seed % 18) as usize];
            }
            let mut input = [0; 4];
            for player in &game.world.as_ref().unwrap().players {
                if player.state == PlayerState::Alive {
                    input[player.id] = mask | if game.frame % 60 < 58 { 2 } else { 0 };
                } else if ghost_support && player.state == PlayerState::Ghost {
                    input[player.id] = ghost_support_input(&game, player);
                }
            }
            game.update(input);
            buttons.push(input);
            if game.screen == Screen::StageClear {
                return Ok(buttons);
            }
            let world = game.world.as_ref().unwrap();
            if game.screen != Screen::Playing
                || world.players.iter().all(|p| p.state != PlayerState::Alive)
            {
                break;
            }
            let health = world
                .enemies
                .iter()
                .filter(|e| matches!(e.kind, micro_mages_re::enemy::EnemyKind::Boss(_)))
                .map(|e| u32::from(e.health))
                .sum();
            if health < best_health {
                best_health = health;
                best = buttons.clone();
                eprintln!("random_trial={trial} ticks={tick} best_boss_health={health}");
            }
        }
    }
    Err(best)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stage = 0;
    let mut hard = false;
    let mut players = 1;
    let mut limit: usize = 18000;
    let mut beam_width = 24;
    let mut depth = 6;
    let mut output = None;
    let mut global = false;
    let mut random_trials = 0;
    let mut ghost_support = false;
    let mut seed = 34567u64;
    let mut random_ticks = 1800usize;
    let mut node_limit = 400000;
    let mut step = 6;
    let mut campaign = false;
    let mut prefix = None;
    let mut trace = None;
    let mut waypoint = None;
    let mut solo_climb = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--waypoint" => {
                let value = args.next().ok_or("missing x,y waypoint")?;
                let (x, y) = value.split_once(',').ok_or("waypoint must be x,y")?;
                waypoint = Some((x.parse::<f32>()?, y.parse::<f32>()?));
            }
            "--stage" => stage = args.next().ok_or("missing stage")?.parse()?,
            "--hard" => hard = true,
            "--solo-climb" => solo_climb = true,
            "--global" => global = true,
            "--ghost-support" => ghost_support = true,
            "--random-trials" => random_trials = args.next().ok_or("missing trials")?.parse()?,
            "--random-ticks" => {
                random_ticks = args.next().ok_or("missing random ticks")?.parse()?
            }
            "--seed" => seed = args.next().ok_or("missing seed")?.parse()?,
            "--step" => step = args.next().ok_or("missing macro step")?.parse()?,
            "--nodes" => node_limit = args.next().ok_or("missing nodes")?.parse()?,
            "--campaign" => campaign = true,
            "--prefix" => prefix = Some(PathBuf::from(args.next().ok_or("missing prefix")?)),
            "--trace" => trace = Some(PathBuf::from(args.next().ok_or("missing trace")?)),
            "--players" => players = args.next().ok_or("missing players")?.parse()?,
            "--ticks" => limit = args.next().ok_or("missing ticks")?.parse()?,
            "--beam" => beam_width = args.next().ok_or("missing beam")?.parse()?,
            "--depth" => depth = args.next().ok_or("missing depth")?.parse()?,
            "--output" => output = Some(PathBuf::from(args.next().ok_or("missing output")?)),
            _ => return Err(format!("unknown argument {arg}").into()),
        }
    }
    if !(1..=60).contains(&step) {
        return Err("macro step must be 1..60 ticks".into());
    }
    if campaign && stage != 0 {
        return Err("campaign search starts at stage 0".into());
    }
    let assets = Arc::new(Assets::load(&micro_mages_re::assets::default_root())?);
    let mut game = Game::new(assets.clone());
    game.start(stage, players, hard)?;
    let mut nav = Navigation::new(&game.world.as_ref().unwrap().level);
    let mut replay = Replay {
        stage,
        hard,
        players,
        buttons: Vec::new(),
        campaign,
    };
    if let Some(path) = prefix {
        let recorded: Replay = serde_json::from_slice(&std::fs::read(path)?)?;
        if (recorded.stage, recorded.hard, recorded.players) != (stage, hard, players) {
            return Err("prefix configuration mismatch".into());
        }
        for (tick, &buttons) in recorded.buttons.iter().enumerate() {
            if game.screen != Screen::Playing && !(campaign && game.screen == Screen::StageClear) {
                return Err(format!("prefix ended before tick {tick}: {:?}", game.screen).into());
            }
            game.update(buttons);
        }
        replay.buttons = recorded.buttons;
        nav = Navigation::new(&game.world.as_ref().unwrap().level);
    }
    if waypoint.is_some() {
        nav = Navigation::targeted(&game.world.as_ref().unwrap().level, waypoint);
    }
    if global || random_trials > 0 {
        if seed == 0 {
            return Err("random seed must be nonzero".into());
        }
        let search_result = if random_trials > 0 {
            random_search(
                game.clone(),
                random_trials,
                random_ticks.min(limit.saturating_sub(replay.buttons.len())),
                step,
                seed,
                ghost_support,
            )
        } else {
            global_search(
                game.clone(),
                &nav,
                players,
                limit.saturating_sub(replay.buttons.len()),
                solo_climb,
                node_limit,
                step,
            )
        };
        let continuation = match search_result {
            Ok(buttons) => buttons,
            Err(partial) => {
                replay.buttons.extend(partial);
                if let Some(path) = &trace {
                    write_trace(path, &replay)?;
                }
                return Err("search exhausted without completion; --trace retains best legal partial progress".into());
            }
        };
        for &buttons in &continuation {
            game.update(buttons);
        }
        replay.buttons.extend(continuation);
    }
    let mut old_score = -1_000_000.0;
    let mut stalled = 0;
    let mut visits = HashMap::<(i16, i16), u32>::new();
    let mut last_lives = game.world.as_ref().unwrap().lives;
    let mut reported_frame = 0usize;
    while (game.screen == Screen::Playing || (campaign && game.screen == Screen::StageClear))
        && replay.buttons.len() < limit
    {
        if game.screen == Screen::StageClear {
            if let Some(path) = &trace {
                write_trace(path, &replay)?;
            }
            let completed = game.world.as_ref().unwrap().level.id;
            eprintln!("Completed campaign stage {completed}; carrying state to next stage");
            for _ in 0..512 {
                if game.screen != Screen::StageClear {
                    break;
                }
                game.update([0; 4]);
                replay.buttons.push([0; 4]);
            }
            if game.screen == Screen::Ending {
                break;
            }
            if game.screen != Screen::Playing {
                return Err("campaign transition failed".into());
            }
            nav = Navigation::new(&game.world.as_ref().unwrap().level);
            old_score = -1_000_000.0;
            stalled = 0;
            visits.clear();
        }
        if let Some((x, y)) = nav.waypoint {
            let p = leading_player(&game).body.center();
            if (p.x - x).abs() < 12.0 && (p.y - y).abs() < 12.0 {
                nav = Navigation::new(&game.world.as_ref().unwrap().level);
                visits.clear();
                stalled = 0;
                old_score = -1_000_000.0;
                eprintln!("Reached navigation waypoint {x},{y}");
            }
        }
        let mut beam = vec![Candidate {
            game: game.clone(),
            first: Vec::new(),
            score: 0.0,
        }];
        for layer in 0..depth {
            let mut next = Vec::new();
            let mut seen = HashSet::new();
            for candidate in &beam {
                if candidate.game.screen != Screen::Playing {
                    next.push(candidate.clone());
                    continue;
                }
                for id in 0..action_count(&candidate.game) {
                    let mut future = candidate.game.clone();
                    let buttons = action(&mut future, id, step, players, solo_climb);
                    let p = leading_player(&future).body.center();
                    let visited = *visits
                        .get(&((p.x / 8.0) as i16, (p.y / 8.0) as i16))
                        .unwrap_or(&0);
                    let value =
                        score(&future, &nav) - visited as f32 * 100.0 + candidate.score * 0.95;
                    if !seen.insert(key(&future)) {
                        continue;
                    }
                    next.push(Candidate {
                        game: future,
                        first: if layer == 0 {
                            buttons
                        } else {
                            candidate.first.clone()
                        },
                        score: value,
                    });
                }
            }
            next.sort_by(|a, b| b.score.total_cmp(&a.score));
            next.truncate(beam_width);
            beam = next;
            if beam
                .first()
                .is_some_and(|c| c.game.screen == Screen::StageClear)
            {
                break;
            }
        }
        let best = beam.first().ok_or("empty search")?;
        for &buttons in &best.first {
            game.update(buttons);
            replay.buttons.push(buttons);
            if game.screen != Screen::Playing {
                break;
            }
        }
        let p = leading_player(&game).body.center();
        *visits
            .entry(((p.x / 8.0) as i16, (p.y / 8.0) as i16))
            .or_default() += 1;
        let current_score = score(&game, &nav);
        let lives = game.world.as_ref().unwrap().lives;
        if lives < last_lives {
            old_score = -1_000_000.0;
            visits.clear();
        }
        last_lives = lives;
        if current_score <= old_score + 0.1 {
            stalled += 1;
        } else {
            stalled = 0;
            old_score = current_score;
        }
        if replay.buttons.len().saturating_sub(reported_frame) >= 240
            || game.screen != Screen::Playing
        {
            reported_frame = replay.buttons.len();
            if let Some(path) = &trace {
                write_trace(path, &replay)?;
            }
            let w = game.world.as_ref().unwrap();
            let p = leading_player(&game);
            eprintln!(
                "ticks={} state={:?} xy={:.1},{:.1} health={} lives={} score={:.1} stalled={} boss_health={}",
                replay.buttons.len(),
                game.screen,
                p.body.position.x,
                p.body.position.y,
                p.health,
                w.lives,
                current_score,
                stalled,
                w.enemies
                    .iter()
                    .filter(|e| matches!(e.kind, micro_mages_re::enemy::EnemyKind::Boss(_)))
                    .map(|e| e.health as u32)
                    .sum::<u32>()
            );
        }
        if stalled > 1000 {
            break;
        }
    }
    if let Some(path) = trace {
        write_trace(path, &replay)?;
    }
    let expected = if campaign {
        Screen::Ending
    } else {
        Screen::StageClear
    };
    if game.screen != expected {
        return Err(format!(
            "No completion found: requested_stage={stage} current_stage={} hard={hard} players={players} screen={:?} ticks={} player={:?} lives={} boss_health={}",
            game.world.as_ref().unwrap().level.id, game.screen, replay.buttons.len(), game.world.as_ref().unwrap().players[0].body.position, game.world.as_ref().unwrap().lives, game.world.as_ref().unwrap().enemies.iter().filter(|e| matches!(e.kind,micro_mages_re::enemy::EnemyKind::Boss(_))).map(|e| e.health as u32).sum::<u32>()
        )
        .into());
    }
    // A claimed outcome must reproduce using buttons alone from a fresh start.
    let mut verify = Game::new(assets);
    verify.start(stage, players, hard)?;
    for (tick, &buttons) in replay.buttons.iter().enumerate() {
        if verify.screen != Screen::Playing && !(campaign && verify.screen == Screen::StageClear) {
            return Err(
                format!("fresh replay ended before tick {tick}: {:?}", verify.screen).into(),
            );
        }
        verify.update(buttons);
    }
    if verify.screen != expected {
        return Err("fresh replay did not complete".into());
    }
    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        write_trace(path, &replay)?;
    }
    println!(
        "Verified legal-input completion: stage={stage} hard={hard} players={players} frames={}",
        replay.buttons.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start() -> Game {
        let assets = Arc::new(Assets::load(&micro_mages_re::assets::default_root()).unwrap());
        let mut game = Game::new(assets);
        game.start(0, 1, false).unwrap();
        game
    }

    #[test]
    fn search_keeps_different_shot_directions_with_identical_player_motion() {
        let mut horizontal = start();
        let mut upward = horizontal.clone();
        action(&mut horizontal, 0, 1, 1, false);
        action(&mut upward, 9, 1, 1, false);
        assert_eq!(
            horizontal.world.as_ref().unwrap().players[0].body.position,
            upward.world.as_ref().unwrap().players[0].body.position
        );
        assert_ne!(
            key(&horizontal),
            key(&upward),
            "aiming must survive branch deduplication"
        );
    }

    #[test]
    fn ghost_support_approaches_living_teammate_and_pulses_fire_without_targets() {
        let mut game = start();
        game.start(13, 2, true).unwrap();
        let world = game.world.as_mut().unwrap();
        world.enemies.clear();
        world.players[0].state = PlayerState::Ghost;
        world.players[0].body.position.x = 20.0;
        world.players[0].body.position.y = 20.0;
        world.players[1].body.position.x = 100.0;
        world.players[1].body.position.y = 100.0;
        game.frame = 0;
        assert_eq!(
            ghost_support_input(&game, &game.world.as_ref().unwrap().players[0]),
            128 | 32 | 2
        );
        game.frame = 6;
        assert_eq!(
            ghost_support_input(&game, &game.world.as_ref().unwrap().players[0]),
            128 | 32
        );
    }

    #[test]
    fn protected_boss_heuristic_prefers_leaving_its_attack_lane() {
        let mut game = start();
        game.start(13, 1, true).unwrap();
        let world = game.world.as_mut().unwrap();
        world.enemies[0].invulnerable_ticks = 1;
        world.players[0].body.position.y = world.enemies[0].body.position.y;
        let nav = Navigation::new(&world.level);
        let aligned = score(&game, &nav);
        game.world.as_mut().unwrap().players[0].body.position.y += 64.0;
        assert!(score(&game, &nav) > aligned);
    }

    #[test]
    fn combat_heuristic_avoids_current_and_imminent_body_overlap() {
        use micro_mages_re::physics::{Body, Vec2};
        let player = Body::new(Vec2::new(0.0, 0.0), Vec2::new(8.0, 8.0));
        let mut enemy = Body::new(Vec2::new(100.0, 0.0), Vec2::new(16.0, 16.0));
        assert_eq!(contact_penalty(&player, &enemy), 0.0);
        enemy.velocity.x = -4.0;
        assert!(contact_penalty(&player, &enemy) > 0.0);
        enemy.position.x = 0.0;
        enemy.velocity.x = 0.0;
        assert!(contact_penalty(&player, &enemy) > 0.0);
    }

    #[test]
    fn search_distinguishes_equal_positions_at_different_simulation_times() {
        let game = start();
        let mut later = game.clone();
        later.frame += 1;
        assert_ne!(key(&game), key(&later));
    }

    #[test]
    fn cooperative_score_penalizes_losing_a_teammate_before_a_team_wipe() {
        let mut game = start();
        game.start(13, 4, true).unwrap();
        let nav = Navigation::new(&game.world.as_ref().unwrap().level);
        let before = score(&game, &nav);
        game.world.as_mut().unwrap().players[3].state = PlayerState::Ghost;
        assert!(score(&game, &nav) < before - 500.0);
    }

    #[test]
    fn search_does_not_advance_terminal_screens() {
        let mut game = start();
        game.screen = Screen::GameOver;
        let before = game.frame;
        assert!(action(&mut game, 18, 6, 1, false).is_empty());
        assert_eq!(game.frame, before);
        assert_eq!(game.screen, Screen::GameOver);
    }

    #[test]
    fn cooperative_arena_macro_can_move_players_independently() {
        let mut game = start();
        game.start(2, 4, false).unwrap();
        let inputs = action(&mut game, 28, 1, 4, false);
        assert_eq!(inputs[0][0] & 192, 64);
        assert_eq!(inputs[0][1] & 192, 128);
    }

    #[test]
    fn held_jump_macro_preserves_jump_across_search_boundaries() {
        let mut game = start();
        let first = action(&mut game, 18, 6, 1, false);
        let second = action(&mut game, 18, 6, 1, false);
        assert!(
            first
                .iter()
                .chain(&second)
                .all(|buttons| buttons[0] & 1 != 0)
        );
        let retrigger = action(&mut game, 3, 6, 1, false);
        assert_eq!(retrigger[0][0] & 1, 0);
        assert!(retrigger[1..].iter().all(|buttons| buttons[0] & 1 != 0));
    }

    #[test]
    fn search_controller_holds_long_enough_to_release_a_charged_shot() {
        let mut game = start();
        action(&mut game, 0, 60, 1, false);
        assert!(
            game.world
                .as_ref()
                .unwrap()
                .projectiles
                .iter()
                .any(|shot| shot.charged && shot.damage == 6)
        );
    }
}
