//! PNG sprite/tile rendering. The renderer reads the world and never changes it.
use crate::{
    assets::{Assets, Level},
    game::{Game, Screen},
    physics::Vec2,
    player::{Player, PlayerState},
    projectile::Appearance,
};
pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 240;

pub struct Video {
    pixels: Vec<u32>,
    background_opaque: Vec<bool>,
    behind_background: bool,
}
impl Default for Video {
    fn default() -> Self {
        Self::new()
    }
}
impl Video {
    pub fn new() -> Self {
        Self {
            pixels: vec![0; WIDTH * HEIGHT],
            background_opaque: vec![false; WIDTH * HEIGHT],
            behind_background: false,
        }
    }
    pub fn render(&mut self, game: &Game, assets: &Assets) -> &[u32] {
        self.pixels.fill(0x080d1c);
        self.background_opaque.fill(false);
        if let Some(run) = &game.world {
            let camera = run.camera.position.y.floor() as i32;
            self.level(&run.level, assets, camera);
            for prop in &run.props {
                if !prop.active || prop.pickup.as_ref().is_some_and(|p| !p.visible) {
                    continue;
                }
                let screen = run.camera.screen(prop.body.center());
                let mut frame = assets
                    .sprites
                    .iter()
                    .find(|sprite| sprite.kind == prop.source_kind)
                    .map_or(0, |sprite| sprite.default_frame);
                if let Some(pickup) = &prop.pickup {
                    frame = assets
                        .animations
                        .iter()
                        .find(|a| a.id == pickup.animation())
                        .expect("pickup animation")
                        .frame_at(pickup.animation_ticks)
                        .sprite;
                }
                if prop.kind == crate::world::PropKind::Checkpoint
                    && prop.body.position == run.checkpoint
                {
                    frame = 2;
                }
                self.sprite(
                    assets,
                    &run.level,
                    SpriteDraw {
                        kind: prop.source_kind,
                        frame,
                        center: screen,
                        flip: prop.flip_x,
                        palette: None,
                    },
                );
            }
            for enemy in &run.enemies {
                if !enemy.visible() {
                    continue;
                }
                for (part, body) in enemy.bodies().into_iter().enumerate() {
                    let pose = enemy.animation_for_part(part).and_then(|(name, tick)| {
                        assets
                            .animations
                            .iter()
                            .find(|animation| animation.id == name)
                            .map(|animation| animation.frame_at(tick))
                    });
                    let mut center = run.camera.screen(body.center());
                    if let Some(pose) = pose {
                        center.x += f32::from(pose.offset_x);
                        center.y += f32::from(pose.offset_y);
                    }
                    self.sprite(
                        assets,
                        &run.level,
                        SpriteDraw {
                            kind: enemy.source_kind,
                            frame: pose.map_or(enemy.animation_frame, |pose| pose.sprite),
                            center,
                            flip: enemy.facing < 0.0,
                            palette: assets
                                .sprites
                                .iter()
                                .find(|sprite| sprite.kind == enemy.source_kind)
                                .map(|sprite| enemy.display_palette(sprite.default_palette)),
                        },
                    );
                }
            }
            for player in &run.players {
                self.behind_background = player.behind_exit();
                self.player(
                    assets,
                    &run.level,
                    player,
                    run.camera.screen(player.body.center()),
                    run.tick,
                );
            }
            self.behind_background = false;
            for particle in &run.particles {
                self.sprite(
                    assets,
                    &run.level,
                    SpriteDraw {
                        kind: particle.source_kind,
                        frame: if particle.source_kind == 50 {
                            particle.ticks_left.saturating_add(1).min(11) / 4
                        } else {
                            0
                        },
                        center: run.camera.screen(particle.position),
                        flip: particle.velocity.x < 0.0,
                        palette: None,
                    },
                );
            }
            for shot in &run.projectiles {
                let center = run.camera.screen(shot.body.center());
                if shot.appearance == Appearance::Spark {
                    let phase = ((run.tick >> 2) & 3) as usize;
                    self.tile(
                        assets,
                        &run.level,
                        TileDraw {
                            tile: [474, 475, 476, 475][phase],
                            x: center.x.floor() as i32 - 4,
                            y: center.y.floor() as i32 - 4,
                            palette: 0,
                            sprite: true,
                            flip_x: false,
                            flip_y: phase == 3,
                        },
                    );
                    continue;
                }
                let (kind, frame) = match shot.appearance {
                    Appearance::Spell => (
                        if shot.charged { 53 } else { 58 },
                        u16::from(shot.body.velocity.y != 0.0),
                    ),
                    Appearance::Freeze => (57, 0),
                    Appearance::Bubble => (59, ((run.tick / 8) % 3) as u16),
                    Appearance::Spear => (56, 0),
                    Appearance::Orb => (41, 1),
                    Appearance::Spark => (58, 0),
                    Appearance::FallingHazard => (30, 1),
                };
                self.sprite(
                    assets,
                    &run.level,
                    SpriteDraw {
                        kind,
                        frame,
                        center,
                        flip: shot.body.velocity.x < 0.0,
                        palette: None,
                    },
                );
            }
        }
        match game.screen {
            Screen::Intro => assets
                .presentation
                .draw(true, game.screen_ticks, &mut self.pixels),
            Screen::Title => assets
                .presentation
                .draw(false, game.screen_ticks, &mut self.pixels),
            Screen::PlayerSelect | Screen::Password => {
                assets
                    .presentation
                    .draw(false, game.screen_ticks, &mut self.pixels);
                self.rect(56, 136, 104, 56, 0);
                self.source_text(assets, 80, 144, "PLAYERS", 0xffffff);
                self.source_text(assets, 88, 152, "1 2 3 4", 0x585858);
                self.source_text(
                    assets,
                    88 + (game.player_count as i32 - 1) * 16,
                    152,
                    &game.player_count.to_string(),
                    0xffffff,
                );
                self.source_text(assets, 80, 168, "PASSWORD", 0xffffff);
                for (index, digit) in game.password.iter().enumerate() {
                    let text = if *digit == 0 {
                        ".".to_owned()
                    } else {
                        (digit % 10).to_string()
                    };
                    self.source_text(assets, 88 + index as i32 * 16, 176, &text, 0xffffff);
                }
                let (x, y) = if game.screen == Screen::PlayerSelect {
                    (88 + (game.player_count as i32 - 1) * 16, 160)
                } else {
                    (88 + game.password_cursor as i32 * 16, 184)
                };
                self.rect(x, y, 5, 1, 0xffffff);
                self.rect(8, 198, 176, 8, 0);
                self.source_text(
                    assets,
                    8,
                    198,
                    if game.hard {
                        "B MODE HARD"
                    } else {
                        "B MODE NORMAL"
                    },
                    0xacacac,
                );
            }
            Screen::Loading => self.loading(game, assets),
            Screen::StageClear => {}
            Screen::GameOver | Screen::Ending => self.outcome(game, assets),
            Screen::Playing => {}
        }
        &self.pixels
    }
    fn loading(&mut self, game: &Game, assets: &Assets) {
        // The scorecard wait and black map-setup interval are presentation time;
        // no world simulation runs until the level fade completes.
        if game.screen_ticks >= 295 {
            self.fade_pixels(
                match game.screen_ticks {
                    295 => 1,
                    296 => 2,
                    _ => 3,
                },
                3,
            );
            return;
        }
        assets.presentation.draw_loading(&mut self.pixels);
        let Some(run) = game.world.as_ref() else {
            return;
        };
        self.rect(96, 40, 72, 8, 0);
        self.rect(144, 64, 24, 8, 0);
        self.source_text(
            assets,
            96,
            40,
            &format!("TOWER {}-{}", run.level.tower + 1, run.level.floor + 1),
            0xffffff,
        );
        if game.hard {
            // The original hard-mode skull is tile C7 beside the floor number.
            for y in 0..8 {
                for x in 0..8 {
                    if assets.atlas.pixels
                        [(199 / 16 * 8 + y) * assets.atlas.width + 199 % 16 * 8 + x]
                        != 0
                    {
                        self.pixels[(40 + y) * WIDTH + 168 + x] = 0xffffff;
                    }
                }
            }
        }
        self.password_label(game, assets, 200);
        self.source_text(assets, 144, 64, &run.lives.to_string(), 0xffffff);
        self.source_text(assets, 112, 88, "SCORE", 0xffffff);
        for id in 0..4 {
            self.rect(112, 104 + id as i32 * 16, 48, 8, 0);
            if id >= game.player_count {
                for y in 104 + id * 16..112 + id * 16 {
                    for x in 96..104 {
                        if self.pixels[y * WIDTH + x] != 0 {
                            self.pixels[y * WIDTH + x] = 0x585858;
                        }
                    }
                }
            }
            self.source_text(
                assets,
                112,
                104 + id as i32 * 16,
                &format!("{:06}", run.player_scores[id].saturating_mul(100)),
                if id < game.player_count {
                    0xffffff
                } else {
                    0x585858
                },
            );
        }
        self.source_text(assets, 104, 168, ". . . .", 0x585858);
        let brightness = match game.screen_ticks {
            0..=4 | 262..=294 => 0,
            5 | 261 => 1,
            6 | 260 => 2,
            _ => 3,
        };
        self.fade_pixels(brightness, 3);
    }
    fn fade_pixels(&mut self, brightness: u32, maximum: u32) {
        for color in &mut self.pixels {
            let r = ((*color >> 16) & 255) * brightness / maximum;
            let g = ((*color >> 8) & 255) * brightness / maximum;
            let b = (*color & 255) * brightness / maximum;
            *color = r << 16 | g << 8 | b;
        }
    }
    fn password_label(&mut self, game: &Game, assets: &Assets, y: i32) {
        let Some(run) = game.world.as_ref() else {
            return;
        };
        if game.hard || run.level.tower > 0 {
            const NORMAL: [&str; 4] = ["1511", "2869", "3645", "7641"];
            const HARD: [&str; 4] = ["1394", "6411", "4913", "2556"];
            self.source_text(assets, 64, y, "PASSWORD", 0xa1a1a1);
            self.rect(130, y + 2, 2, 2, 0xa1a1a1);
            self.rect(130, y + 6, 2, 2, 0xa1a1a1);
            let code = if game.hard { HARD } else { NORMAL }[run.level.tower.min(3)];
            for (index, digit) in code.chars().enumerate() {
                self.source_text(
                    assets,
                    144 + index as i32 * 16,
                    y,
                    &digit.to_string(),
                    0xa1a1a1,
                );
            }
        }
    }
    /// Original 8-pixel lettering, using editable PNG atlas glyphs (e397/e3eb).
    pub(crate) fn source_text(&mut self, assets: &Assets, x: i32, y: i32, text: &str, color: u32) {
        const GLYPHS: &str = "EOSRATINMLDHYCGPUF-.W!V? BJXZ0123456789";
        for (index, character) in text.chars().enumerate() {
            if character == ' ' {
                continue;
            }
            let Some(glyph) = GLYPHS.chars().position(|c| c == character) else {
                continue;
            };
            let tile = if character == '0' { 218 } else { 217 + glyph };
            for row in 0..8 {
                for col in 0..8 {
                    let px = x + index as i32 * 8 + col as i32;
                    let py = y + row as i32;
                    if !(0..WIDTH as i32).contains(&px) || !(0..HEIGHT as i32).contains(&py) {
                        continue;
                    }
                    let shade = assets.atlas.pixels
                        [(tile / 16 * 8 + row) * assets.atlas.width + tile % 16 * 8 + col];
                    if shade != 0 {
                        self.pixels[py as usize * WIDTH + px as usize] = color;
                    }
                }
            }
        }
    }
    fn outcome(&mut self, game: &Game, assets: &Assets) {
        let game_over = game.screen == Screen::GameOver;
        assets.outcomes.draw(
            game_over,
            game.hard,
            game.player_count,
            game.screen_ticks,
            &mut self.pixels,
        );
        if !game_over && game.hard && game.screen_ticks < 2150 {
            return;
        }
        if !game_over && game.hard {
            self.pixels.fill(0);
            self.source_text(assets, 104, 48, "THE END", 0xffffff);
            self.source_text(assets, 112, 72, "SCORE", 0xffffff);
            self.source_text(assets, 104, 152, ". . . .", 0x585858);
        }
        let score_y = if game_over {
            72
        } else if game.hard {
            88
        } else {
            104
        };
        if let Some(run) = &game.world {
            for id in 0..4 {
                let y = score_y + id * 16;
                assets
                    .outcomes
                    .draw_icon(id, id < game.player_count, 96, y, &mut self.pixels);
                self.rect(112, y as i32, 48, 8, 0);
                self.source_text(
                    assets,
                    112,
                    y as i32,
                    &format!("{:06}", (run.player_scores[id] % 10000) * 100),
                    if id < game.player_count {
                        0xffffff
                    } else {
                        0x585858
                    },
                );
            }
        }
        if !game_over && game.hard {
            self.ending_stars(assets, game.screen_ticks);
        }
        if game_over {
            self.password_label(game, assets, 168);
            // Original f548/f561 reveal choices only after 120 ticks.
            if game.screen_ticks < 120 {
                self.rect(80, 192, 96, 16, 0);
            } else if !game.game_over_continue {
                let cursor: Vec<_> = (192..200)
                    .flat_map(|y| self.pixels[y * WIDTH + 80..y * WIDTH + 88].iter().copied())
                    .collect();
                self.rect(80, 192, 8, 8, 0);
                for row in 0..8 {
                    self.pixels[(200 + row) * WIDTH + 80..(200 + row) * WIDTH + 88]
                        .copy_from_slice(&cursor[row * 8..row * 8 + 8]);
                }
            }
        } else if !game.hard && game.screen_ticks < 120 {
            self.rect(96, 192, 80, 8, 0);
        }
    }
    /// Source a131 resets the paired-byte random stream each frame. Stars
    /// scroll one pixel per tick behind the score lettering (attribute22).
    fn ending_stars(&mut self, assets: &Assets, tick: u64) {
        let mut state = 0xaf84u16;
        let scroll = 210u8.wrapping_sub(tick as u8);
        for index in (1..=28).rev() {
            let feedback = state & 1 != 0;
            state = (state >> 1) ^ if feedback { 0xb400 } else { 0 };
            let x = (state & 255) as usize;
            let y = ((state >> 8) as u8)
                .wrapping_sub(scroll)
                .wrapping_sub(u8::from(!feedback)) as usize;
            let tile = if index % 2 == 0 { 496 } else { 511 };
            for row in 0..8 {
                for col in 0..8 {
                    if x + col >= WIDTH || y + row >= HEIGHT {
                        continue;
                    }
                    let shade = assets.atlas.pixels
                        [(tile / 16 * 8 + row) * assets.atlas.width + tile % 16 * 8 + col]
                        as usize;
                    let dest = (y + row) * WIDTH + x + col;
                    if shade != 0 && self.pixels[dest] == 0 {
                        self.pixels[dest] = [0, 0x006d85, 0x39c3df, 0xffffff][shade];
                    }
                }
            }
        }
    }
    fn level(&mut self, level: &Level, assets: &Assets, camera: i32) {
        self.pixels
            .fill(assets.colors[usize::from(level.colors[0])]);
        let first = (camera / 8).max(0) as usize;
        for row in first..(first + 31).min(level.height) {
            for col in 0..level.width {
                let index = row * level.width + col;
                self.tile(
                    assets,
                    level,
                    TileDraw {
                        tile: level.tiles[index],
                        x: col as i32 * 8,
                        y: row as i32 * 8 - camera,
                        palette: level.palettes[index],
                        sprite: false,
                        flip_x: false,
                        flip_y: false,
                    },
                );
            }
        }
    }
    fn player(&mut self, assets: &Assets, level: &Level, player: &Player, center: Vec2, tick: u64) {
        if player.state == PlayerState::Dead || player.has_exited() {
            return;
        }
        // The extra-health fairy remains visible during the player's hurt flicker.
        if player.health >= 2 {
            const X: [i32; 16] = [4, 3, 1, -1, -3, -6, -8, -10, -10, -10, -8, -6, -4, -1, 1, 3];
            const Y: [i32; 16] = [-3, -1, 1, 3, 4, 3, 1, -1, -3, -6, -8, -10, -10, -10, -8, -6];
            let index = ((tick >> 1) & 15) as usize;
            self.tile(
                assets,
                level,
                TileDraw {
                    tile: 486,
                    x: center.x.floor() as i32 + X[index],
                    y: center.y.floor() as i32 + Y[index],
                    palette: player.id as u8,
                    sprite: true,
                    flip_x: false,
                    flip_y: false,
                },
            );
        }
        if player.invulnerability & 2 != 0 {
            return;
        }
        let (name, animation_tick) = player.animation_pose();
        let pose = assets
            .animations
            .iter()
            .find(|animation| animation.id == name)
            .map(|animation| animation.frame_at(animation_tick));
        let mut sprite_center = center;
        if let Some(pose) = pose {
            sprite_center.x += f32::from(pose.offset_x);
            sprite_center.y += f32::from(pose.offset_y);
        }
        self.sprite(
            assets,
            level,
            SpriteDraw {
                kind: 33,
                frame: pose.map_or(player.animation_frame, |pose| pose.sprite),
                center: sprite_center,
                flip: player.facing < 0,
                palette: Some(player.display_palette()),
            },
        );
        if let Some(feather) = pose
            .filter(|_| player.feather)
            .and_then(|pose| pose.attachment.as_ref())
        {
            let offset_x = if player.facing < 0 {
                -feather.offset_x
            } else {
                feather.offset_x
            };
            self.tile(
                assets,
                level,
                TileDraw {
                    tile: feather.tile,
                    x: center.x.floor() as i32 + i32::from(offset_x) - 4,
                    y: center.y.floor() as i32 + i32::from(feather.offset_y) - 5,
                    palette: 1,
                    sprite: true,
                    flip_x: player.facing < 0,
                    flip_y: false,
                },
            );
        }
    }
    fn sprite(&mut self, assets: &Assets, level: &Level, draw: SpriteDraw) {
        let SpriteDraw {
            kind,
            frame,
            center,
            flip,
            palette,
        } = draw;
        let Some(sprite) = assets.sprites.iter().find(|s| s.kind == kind) else {
            return;
        };
        let Some(frame) = sprite
            .frames
            .iter()
            .find(|f| f.id == frame)
            .or_else(|| sprite.frames.first())
        else {
            return;
        };
        for part in &frame.parts {
            let x = if flip {
                -i32::from(part.x) - 8
            } else {
                i32::from(part.x)
            };
            self.tile(
                assets,
                level,
                TileDraw {
                    tile: part.tile,
                    x: center.x.floor() as i32 + x,
                    y: center.y.floor() as i32 + i32::from(part.y),
                    palette: palette.unwrap_or(sprite.default_palette),
                    sprite: true,
                    flip_x: flip ^ part.flip_x,
                    flip_y: part.flip_y,
                },
            );
        }
    }
    fn tile(&mut self, assets: &Assets, level: &Level, draw: TileDraw) {
        let atlas_x = usize::from(draw.tile) % 16 * 8;
        let atlas_y = usize::from(draw.tile) / 16 * 8;
        for row in 0..8 {
            let y = draw.y + row;
            if !(0..HEIGHT as i32).contains(&y) {
                continue;
            }
            for col in 0..8 {
                let x = draw.x + col;
                if !(0..WIDTH as i32).contains(&x) {
                    continue;
                }
                let tx = if draw.flip_x { 7 - col } else { col } as usize;
                let ty = if draw.flip_y { 7 - row } else { row } as usize;
                let shade = assets.atlas.pixels[(atlas_y + ty) * assets.atlas.width + atlas_x + tx];
                let index = y as usize * WIDTH + x as usize;
                if !draw.sprite {
                    self.background_opaque[index] = shade != 0;
                } else if self.behind_background && self.background_opaque[index] {
                    continue;
                }
                if shade == 0 && draw.sprite {
                    continue;
                }
                let palette_index = if shade == 0 {
                    0
                } else {
                    usize::from(draw.sprite) * 16
                        + usize::from(draw.palette) * 4
                        + usize::from(shade)
                };
                self.pixels[y as usize * WIDTH + x as usize] =
                    assets.colors[usize::from(level.colors[palette_index])];
            }
        }
    }
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        for row in y.max(0)..(y + h).min(HEIGHT as i32) {
            for col in x.max(0)..(x + w).min(WIDTH as i32) {
                self.pixels[row as usize * WIDTH + col as usize] = color;
            }
        }
    }
}
struct TileDraw {
    tile: u16,
    x: i32,
    y: i32,
    palette: u8,
    sprite: bool,
    flip_x: bool,
    flip_y: bool,
}

struct SpriteDraw {
    kind: u8,
    frame: u16,
    center: Vec2,
    flip: bool,
    palette: Option<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn original_outcome_art_and_live_score_digits_match_reference() {
        let assets = Arc::new(Assets::load(&crate::assets::default_root()).unwrap());
        for (name, screen, players, hard, tick) in [
            ("game-over-4", Screen::GameOver, 4, false, 299),
            ("normal-ending", Screen::Ending, 1, false, 299),
            ("hard-ending-4", Screen::Ending, 4, true, 1349),
            ("hard-ending-score", Screen::Ending, 1, true, 2399),
        ] {
            let mut game = Game::new(assets.clone());
            game.start(0, players, hard).unwrap();
            game.screen = screen;
            game.screen_ticks = tick;
            let file = std::fs::File::open(format!(
                "{}/tests/fixtures/presentation/{name}.png",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap();
            let mut reader = png::Decoder::new(std::io::BufReader::new(file))
                .read_info()
                .unwrap();
            let mut bytes = vec![0; reader.output_buffer_size()];
            let info = reader.next_frame(&mut bytes).unwrap();
            let expected: Vec<u32> = bytes[..info.buffer_size()]
                .chunks_exact(3)
                .map(|c| u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]))
                .collect();
            let mut renderer = Video::new();
            let actual = renderer.render(&game, &assets);
            let mismatches = actual.iter().zip(&expected).filter(|(a, b)| a != b).count();
            assert_eq!(
                mismatches, 0,
                "{name} differs from independently captured source frame"
            );
            if screen == Screen::GameOver {
                game.world.as_mut().unwrap().player_scores[0] = 12;
                assert_ne!(renderer.render(&game, &assets), expected);
                game.game_over_continue = false;
                let end = renderer.render(&game, &assets);
                assert!(
                    end[192 * WIDTH + 80..192 * WIDTH + 88]
                        .iter()
                        .all(|&p| p == 0)
                );
            }
        }
    }

    #[test]
    fn doorway_priority_preserves_opaque_background_pixels() {
        let assets = Arc::new(Assets::load(&crate::assets::default_root()).unwrap());
        let mut game = Game::new(assets.clone());
        game.start(0, 1, false).unwrap();
        let level = &game.world.as_ref().unwrap().level;
        let mut renderer = Video::new();
        renderer.level(level, &assets, 0);
        let background = renderer.pixels.clone();
        renderer.behind_background = true;
        for y in (0..HEIGHT).step_by(8) {
            for x in (0..WIDTH).step_by(8) {
                renderer.tile(
                    &assets,
                    level,
                    TileDraw {
                        tile: 486,
                        x: x as i32,
                        y: y as i32,
                        palette: 0,
                        sprite: true,
                        flip_x: false,
                        flip_y: false,
                    },
                );
            }
        }
        let mut changed_transparent = 0;
        let mut opaque = 0;
        for (index, original) in background.iter().enumerate() {
            if renderer.background_opaque[index] {
                opaque += 1;
                assert_eq!(renderer.pixels[index], *original);
            } else if renderer.pixels[index] != *original {
                changed_transparent += 1;
            }
        }
        assert!(opaque > 0);
        assert!(
            changed_transparent > 0,
            "background shade zero must let the mage show through"
        );
    }

    #[test]
    fn gameplay_keeps_top_of_level_visible_during_stage_clear() {
        let assets = Arc::new(Assets::load(&crate::assets::default_root()).unwrap());
        let mut game = Game::new(assets.clone());
        game.start(0, 1, false).unwrap();
        let mut renderer = Video::new();
        let run = game.world.as_mut().unwrap();
        run.enemies.clear();
        run.props.clear();
        run.players.clear();
        let playing = renderer.render(&game, &assets).to_vec();
        let world = game.world.as_ref().unwrap();
        let mut level_only = Video::new();
        level_only.level(
            &world.level,
            &assets,
            world.camera.position.y.floor() as i32,
        );
        assert_eq!(&playing[..13 * WIDTH], &level_only.pixels[..13 * WIDTH]);
        game.screen = Screen::StageClear;
        assert_eq!(renderer.render(&game, &assets), playing);
    }

    #[test]
    fn feather_and_health_fairy_follow_source_oam_offsets() {
        let mut assets = Assets::load(&crate::assets::default_root()).unwrap();
        assets.atlas.pixels.fill(0);
        // Isolate the idle feather and health fairy, retaining their original atlas IDs.
        for tile in [469usize, 486] {
            for y in 0..8 {
                for x in 0..8 {
                    let index = ((tile / 16) * 8 + y) * assets.atlas.width + (tile % 16) * 8 + x;
                    assets.atlas.pixels[index] = 1;
                }
            }
        }
        let level = assets.level(0, false).unwrap();
        let mut renderer = Video::new();
        let mut player = Player::new(0, Vec2::new(124., 96.));
        let center = Vec2::new(128., 100.);
        renderer.player(&assets, level, &player, center, 0);
        assert!(renderer.pixels.iter().all(|&pixel| pixel == 0));
        player.feather = true;
        renderer.player(&assets, level, &player, center, 0);
        let color = assets.colors[usize::from(level.colors[21])];
        assert_eq!(renderer.pixels[90 * WIDTH + 122], color);
        assert_eq!(renderer.pixels[89 * WIDTH + 122], 0);
        assert_eq!(renderer.pixels[90 * WIDTH + 121], 0);
        renderer.pixels.fill(0);
        player.facing = -1;
        renderer.player(&assets, level, &player, center, 0);
        assert_eq!(renderer.pixels[90 * WIDTH + 126], color);
        assert_eq!(renderer.pixels[90 * WIDTH + 125], 0);
        renderer.pixels.fill(0);
        player.health = 2;
        player.invulnerability = 2;
        renderer.player(&assets, level, &player, center, 0);
        assert_eq!(
            renderer.pixels[97 * WIDTH + 132],
            assets.colors[usize::from(level.colors[17])]
        );
        assert_eq!(renderer.pixels[90 * WIDTH + 126], 0);
        assert_eq!(
            renderer.pixels.iter().filter(|&&pixel| pixel != 0).count(),
            64
        );
    }
    #[test]
    fn lightning_is_hidden_at_rest_and_draws_all_three_active_segments() {
        use crate::enemy::{Enemy, EnemyContext, EnemyKind, EnemyRng};
        let assets = Arc::new(Assets::load(&crate::assets::default_root()).unwrap());
        let mut game = Game::new(assets.clone());
        game.start(0, 1, false).unwrap();
        let world = game.world.as_mut().unwrap();
        world.camera.position.y = 0.;
        world.enemies.clear();
        world.props.clear();
        world.players.clear();
        let mut renderer = Video::new();
        let background = renderer.render(&game, &assets).to_vec();
        game.world.as_mut().unwrap().enemies.push(Enemy::new(
            EnemyKind::Lightning,
            Vec2::new(124., 92.),
            false,
        ));
        assert_eq!(renderer.render(&game, &assets), background);
        game.world.as_mut().unwrap().enemies[0].start_lightning_warning(true);
        assert_ne!(renderer.render(&game, &assets), background);
        let mut random = EnemyRng::new(1);
        for tick in 0..80 {
            game.world.as_mut().unwrap().enemies[0].tick_with_context(
                &EnemyContext::single(Some(Vec2::new(64., 100.)), tick),
                &mut random,
            );
        }
        let active = renderer.render(&game, &assets);
        for row in [80, 112, 144] {
            assert!(
                (row..row + 32)
                    .any(|y| (120..140)
                        .any(|x| { active[y * WIDTH + x] != background[y * WIDTH + x] })),
                "missing lightning segment near row {row}"
            );
        }
    }
    #[test]
    fn loading_scorecards_match_independent_original_solo_and_four_player_pixels() {
        let assets = Arc::new(Assets::load(&crate::assets::default_root()).unwrap());
        for (players, hard, name) in [
            (1, false, "solo"),
            (4, false, "four"),
            (4, true, "hard-four"),
        ] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                "tests/fixtures/presentation/original-loading-{name}.png"
            ));
            let mut reader =
                png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()))
                    .read_info()
                    .unwrap();
            let mut bytes = vec![0; reader.output_buffer_size()];
            let info = reader.next_frame(&mut bytes).unwrap();
            let expected: Vec<u32> = bytes[..info.buffer_size()]
                .chunks_exact(3)
                .map(|c| u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]))
                .collect();
            let mut game = Game::new(assets.clone());
            game.start(0, players, hard).unwrap();
            game.begin_loading();
            game.screen_ticks = 10;
            let mut renderer = Video::new();
            let actual = renderer.render(&game, &assets);
            assert_eq!(
                actual.iter().zip(&expected).filter(|(a, b)| a != b).count(),
                0,
                "{name} scorecard differs from original pixels"
            );
        }
    }
    #[test]
    fn rendering_does_not_advance_simulation_and_is_deterministic() {
        let assets = Arc::new(Assets::load(&crate::assets::default_root()).unwrap());
        let mut game = Game::new(assets.clone());
        game.start(0, 4, false).unwrap();
        let mut renderer = Video::new();
        let first = renderer.render(&game, &assets).to_vec();
        assert_eq!(first, renderer.render(&game, &assets));
        assert_eq!(game.frame, 0);
        assert!(first.iter().any(|&p| p != 0));
    }
    #[test]
    fn sprite_transparency_flip_and_clipping_use_png_pixels() {
        let mut assets = Assets::load(&crate::assets::default_root()).unwrap();
        assets.atlas.pixels.fill(0);
        assets.atlas.pixels[0] = 1;
        let level = assets.level(0, false).unwrap();
        let mut renderer = Video::new();
        renderer.pixels.fill(0x123456);
        renderer.tile(
            &assets,
            level,
            TileDraw {
                tile: 0,
                x: 0,
                y: 0,
                palette: 0,
                sprite: true,
                flip_x: true,
                flip_y: false,
            },
        );
        assert_eq!(renderer.pixels[0], 0x123456);
        assert_eq!(
            renderer.pixels[7],
            assets.colors[usize::from(level.colors[17])]
        );
        renderer.tile(
            &assets,
            level,
            TileDraw {
                tile: 0,
                x: -100,
                y: -100,
                palette: 0,
                sprite: true,
                flip_x: false,
                flip_y: false,
            },
        );
        assert_eq!(renderer.pixels[0], 0x123456);
    }
}
