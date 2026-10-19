//! Campaign state and screen transitions, independent of input devices and rendering.
use crate::{
    assets::Assets,
    events::{GameEvent, Music},
    input::{InputFrame, PlayerInput},
    world::{RunResult, World},
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Intro,
    Title,
    PlayerSelect,
    Password,
    Loading,
    Playing,
    StageClear,
    GameOver,
    Ending,
}

#[derive(Clone)]
pub struct Game {
    pub screen: Screen,
    pub world: Option<World>,
    pub frame: u64,
    pub player_count: usize,
    pub hard: bool,
    pub screen_ticks: u64,
    pub password: [u8; 4],
    pub password_cursor: usize,
    pub presentation_enabled: bool,
    pub game_over_continue: bool,
    assets: Arc<Assets>,
    previous: [PlayerInput; 4],
    transition_ticks: u16,
}
impl Game {
    pub fn new(assets: Arc<Assets>) -> Self {
        Self {
            screen: Screen::Intro,
            world: None,
            frame: 0,
            player_count: 1,
            hard: false,
            screen_ticks: 0,
            password: [0; 4],
            password_cursor: 0,
            presentation_enabled: false,
            game_over_continue: true,
            assets,
            previous: [PlayerInput::default(); 4],
            transition_ticks: 0,
        }
    }
    pub fn start(
        &mut self,
        stage: usize,
        players: usize,
        hard: bool,
    ) -> Result<Vec<GameEvent>, String> {
        if !(1..=4).contains(&players) {
            return Err("player count must be 1..4".into());
        }
        let level = self
            .assets
            .level(stage, hard)
            .ok_or_else(|| format!("no playable stage {stage}, hard={hard}"))?
            .clone();
        let music = Music::from_name(&level.music).ok_or("unknown level music")?;
        let mut world = World::new(level, players);
        world.seed_random(self.frame);
        self.world = Some(world);
        self.player_count = players;
        self.hard = hard;
        self.screen = Screen::Playing;
        self.screen_ticks = 0;
        self.presentation_enabled = false;
        self.transition_ticks = 0;
        Ok(vec![GameEvent::Music(music)])
    }
    pub fn update(&mut self, buttons: [u8; 4]) -> Vec<GameEvent> {
        self.frame += 1;
        self.screen_ticks += 1;
        let mut inputs = [InputFrame::default(); 4];
        for id in 0..4 {
            let held = PlayerInput::from_buttons(buttons[id]);
            inputs[id] = InputFrame::between(self.previous[id], held);
            self.previous[id] = held;
        }
        let mut events = Vec::new();
        match self.screen {
            Screen::Intro => {
                if self.screen_ticks == 5 {
                    events.push(GameEvent::Music(Music::Intro));
                }
                if self.screen_ticks >= u64::from(crate::scenes::INTRO_TICKS)
                    || inputs.iter().any(|i| i.pressed.jump)
                {
                    if self.screen_ticks < 5 {
                        events.push(GameEvent::Music(Music::Intro));
                    }
                    self.screen = Screen::Title;
                    self.screen_ticks = 0;
                }
            }
            Screen::Title => {
                if inputs.iter().any(|i| i.pressed.jump) {
                    self.screen = Screen::PlayerSelect;
                    self.password = [0; 4];
                    self.password_cursor = 0;
                }
            }
            Screen::PlayerSelect => {
                if inputs.iter().any(|i| i.pressed.right || i.pressed.up) {
                    self.player_count = (self.player_count % 4) + 1;
                }
                if inputs.iter().any(|i| i.pressed.left || i.pressed.down) {
                    self.player_count = if self.player_count == 1 {
                        4
                    } else {
                        self.player_count - 1
                    };
                }
                if inputs.iter().any(|i| i.pressed.fire) {
                    self.hard = !self.hard;
                }
                if inputs.iter().any(|i| i.pressed.jump) {
                    self.screen = Screen::Password;
                }
            }
            Screen::Password => {
                if inputs.iter().any(|i| i.pressed.right) {
                    self.password_cursor = (self.password_cursor + 1).min(3);
                }
                if inputs.iter().any(|i| i.pressed.left) {
                    self.password_cursor = self.password_cursor.saturating_sub(1);
                }
                if inputs.iter().any(|i| i.pressed.up) {
                    self.password[self.password_cursor] =
                        self.password[self.password_cursor] % 9 + 1;
                }
                if inputs.iter().any(|i| i.pressed.down) {
                    self.password[self.password_cursor] =
                        if self.password[self.password_cursor] <= 1 {
                            9
                        } else {
                            self.password[self.password_cursor] - 1
                        };
                }
                if inputs.iter().any(|i| i.pressed.fire) {
                    self.hard = !self.hard;
                }
                if inputs.iter().any(|i| i.pressed.jump) {
                    if let Some((stage, hard)) = password_destination(self.password, self.hard) {
                        events.extend(
                            self.start(stage, self.player_count, hard)
                                .expect("validated password stage"),
                        );
                        self.presentation_enabled = true;
                        self.begin_loading();
                    } else {
                        self.password = [0; 4];
                        events.push(GameEvent::Sound(crate::events::Sound::DamageEnemy));
                    }
                }
            }
            Screen::Loading => {
                if self.screen_ticks == 294 {
                    let level = &self.world.as_ref().expect("loading requires a level").level;
                    events.push(GameEvent::Music(
                        Music::from_name(&level.music).expect("validated level music"),
                    ));
                }
                if self.screen_ticks >= u64::from(crate::scenes::LOADING_TICKS) {
                    self.screen = Screen::Playing;
                    self.screen_ticks = 0;
                }
            }
            Screen::Playing => {
                let previous_lives = self.world.as_ref().expect("playing requires a level").lives;
                let result = self
                    .world
                    .as_mut()
                    .expect("playing requires a level")
                    .step(inputs, &mut events);
                if result == RunResult::Complete {
                    self.screen = Screen::StageClear;
                    self.screen_ticks = 0;
                    // B395 decrements the half-rate counter on the callback
                    // frame itself. `frame` already counts that completed tick.
                    let consumed = if (self.frame - 1).is_multiple_of(2) {
                        2
                    } else {
                        1
                    };
                    self.transition_ticks =
                        self.world.as_mut().unwrap().begin_stage_clear() - consumed;
                    if self.world.as_ref().unwrap().level.id == 11 {
                        // B401 also starts the segmented lift script this frame.
                        self.screen_ticks = 1;
                    }
                    events.push(GameEvent::StageCompleted);
                    let level = &self.world.as_ref().unwrap().level;
                    events.push(if level.boss && level.id != 11 && level.id != 13 {
                        GameEvent::Music(Music::Fanfare)
                    } else {
                        GameEvent::FadeMusic
                    });
                } else if result == RunResult::GameOver {
                    self.screen = Screen::GameOver;
                    self.screen_ticks = 0;
                    self.game_over_continue = true;
                    events.push(GameEvent::GameOver);
                    events.push(GameEvent::StopMusic);
                } else if self.presentation_enabled
                    && self.world.as_ref().unwrap().lives < previous_lives
                {
                    // B3CF routes a surviving team wipe through the same tower
                    // scorecard as an ordinary floor transition (D03F).
                    self.begin_loading();
                }
            }
            Screen::StageClear => {
                if let Some(world) = &mut self.world {
                    world.step_after_clear(inputs, &mut events);
                }
                let segmented = self
                    .world
                    .as_ref()
                    .is_some_and(|world| world.level.id == 11);
                if segmented && self.screen_ticks == 211 {
                    self.world.as_mut().unwrap().begin_segmented_ascent();
                }
                if segmented && self.screen_ticks >= 212 {
                    if self.world.as_mut().unwrap().lift_after_segmented_boss() {
                        self.transition_ticks = self.transition_ticks.min(2);
                    } else {
                        // B770 holds the countdown while any mage remains onscreen.
                        self.transition_ticks = 3;
                    }
                }
                self.transition_ticks = self.transition_ticks.saturating_sub(1);
                if self.transition_ticks == 0 {
                    self.advance(&mut events);
                }
            }
            Screen::GameOver => {
                if self.screen_ticks < 120 {
                    return events;
                }
                if inputs.iter().any(|i| i.pressed.up) {
                    self.game_over_continue = true;
                }
                if inputs.iter().any(|i| i.pressed.down) {
                    self.game_over_continue = false;
                }
                if inputs.iter().any(|i| i.pressed.jump) {
                    if self.game_over_continue {
                        // Continue restores the last tower password, not the
                        // failed floor or its checkpoint (E1DA / E24D / B317).
                        let stage = (self
                            .world
                            .as_ref()
                            .expect("game over requires a run")
                            .level
                            .id
                            / 3
                            * 3)
                        .min(9);
                        let presentation = self.presentation_enabled;
                        events.extend(
                            self.start(stage, self.player_count, self.hard)
                                .expect("validated continue stage"),
                        );
                        self.presentation_enabled = presentation;
                        if presentation {
                            self.begin_loading();
                        }
                    } else {
                        self.return_to_title();
                        events.push(GameEvent::Music(Music::Intro));
                    }
                }
            }
            Screen::Ending => {
                if self.hard {
                    match self.screen_ticks {
                        390 => events.push(GameEvent::Music(Music::Fanfare)),
                        2148 => events.push(GameEvent::Music(Music::Dreamy)),
                        _ => {}
                    }
                }
                if !self.hard && self.screen_ticks >= 120 && inputs.iter().any(|i| i.pressed.jump) {
                    // EC34 / EC62 enter hard mode through B31B, preserving
                    // scores and reserve lives rather than starting a new run.
                    let run = self.world.as_ref().expect("ending requires a run");
                    let scores = run.player_scores;
                    let score = run.score;
                    let lives = run.lives;
                    let presentation = self.presentation_enabled;
                    events.extend(
                        self.start(0, self.player_count, true)
                            .expect("validated hard campaign"),
                    );
                    let run = self.world.as_mut().unwrap();
                    run.player_scores = scores;
                    run.score = score;
                    run.lives = lives;
                    self.presentation_enabled = presentation;
                    if presentation {
                        self.begin_loading();
                    }
                }
            }
        }
        if self.screen == Screen::Loading && self.screen_ticks == 0 {
            events.retain(|event| !matches!(event, GameEvent::Music(_)));
            events.push(GameEvent::StopMusic);
        }
        events
    }
    pub fn begin_loading(&mut self) {
        self.screen = Screen::Loading;
        self.screen_ticks = 0;
    }
    pub fn return_to_title(&mut self) {
        self.screen = Screen::Title;
        self.screen_ticks = 0;
        self.world = None;
    }
    fn advance(&mut self, events: &mut Vec<GameEvent>) {
        let run = self
            .world
            .as_ref()
            .expect("stage transition requires level");
        let next = run.level.id + 1;
        let score = run.score;
        let player_scores = run.player_scores;
        let presentation = self.presentation_enabled;
        let lives = run.lives;
        let rewards = run.rewards;
        let portal_reward_collected = run.portal_reward_collected;
        let equipment: Vec<_> = run
            .players
            .iter()
            .map(|p| (p.health.max(1), p.feather))
            .collect();
        if self.assets.level(next, self.hard).is_none() {
            self.screen = Screen::Ending;
            self.screen_ticks = 0;
            events.push(GameEvent::Victory);
            events.push(GameEvent::StopMusic);
            return;
        }
        events.extend(
            self.start(next, self.player_count, self.hard)
                .expect("validated campaign level"),
        );
        let run = self.world.as_mut().unwrap();
        run.score = score;
        run.player_scores = player_scores;
        run.lives = lives;
        run.rewards = rewards;
        run.portal_reward_collected = portal_reward_collected;
        if !(self.hard && next == 13) {
            for (player, (health, feather)) in run.players.iter_mut().zip(equipment) {
                player.health = health;
                player.feather = feather;
            }
        }
        self.presentation_enabled = presentation;
        if presentation {
            self.begin_loading();
        }
    }
}

/// Original password table at E24D, decoded into display-order digits and stages.
fn password_destination(digits: [u8; 4], selected_hard: bool) -> Option<(usize, bool)> {
    match digits {
        [0, 0, 0, 0] => Some((0, selected_hard)),
        [1, 5, 1, 1] => Some((0, false)),
        [2, 8, 6, 9] => Some((3, false)),
        [3, 6, 4, 5] => Some((6, false)),
        [7, 6, 4, 1] => Some((9, false)),
        [1, 3, 9, 4] => Some((0, true)),
        [6, 4, 1, 1] => Some((3, true)),
        [4, 9, 1, 3] => Some((6, true)),
        [2, 5, 5, 6] => Some((9, true)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        events::Sound,
        physics::{Body, Vec2},
        player::{PlayerEvent, PlayerState},
        world::{Prop, PropKind},
    };
    fn game() -> Game {
        Game::new(Arc::new(
            Assets::load(&crate::assets::default_root()).unwrap(),
        ))
    }
    #[test]
    fn continue_restarts_the_password_tower_with_fresh_run_state() {
        for hard in [false, true] {
            for stage in 0..if hard { 14 } else { 13 } {
                let mut game = game();
                game.start(stage, 4, hard).unwrap();
                game.presentation_enabled = true;
                let world = game.world.as_mut().unwrap();
                world.score = 50;
                world.player_scores = [10, 10, 10, 20];
                world.lives = 0;
                world.players[0].feather = true;
                game.screen = Screen::GameOver;
                game.screen_ticks = 120;
                game.update([1, 0, 0, 0]);
                assert_eq!(game.screen, Screen::Loading);
                let world = game.world.as_ref().unwrap();
                assert_eq!(world.level.id, (stage / 3 * 3).min(9));
                assert_eq!((world.level.hard, world.players.len()), (hard, 4));
                assert_eq!(
                    (world.score, world.player_scores, world.lives),
                    (0, [0; 4], 3)
                );
                assert!(world.players.iter().all(|player| !player.feather));
            }
        }
    }

    #[test]
    fn game_over_end_returns_to_title_instead_of_continuing() {
        let mut game = game();
        game.start(5, 2, true).unwrap();
        game.screen = Screen::GameOver;
        game.screen_ticks = 120;
        game.update([32, 0, 0, 0]);
        assert!(!game.game_over_continue);
        game.update([1, 0, 0, 0]);
        assert_eq!(game.screen, Screen::Title);
        assert!(game.world.is_none());
    }

    #[test]
    fn normal_ending_enters_hard_mode_preserving_scores_and_remaining_lives() {
        let mut game = game();
        game.start(12, 4, false).unwrap();
        game.presentation_enabled = true;
        let run = game.world.as_mut().unwrap();
        run.score = 10;
        run.player_scores = [1, 2, 3, 4];
        run.lives = 1;
        game.screen = Screen::Ending;
        game.screen_ticks = 120;
        game.update([1, 0, 0, 0]);
        assert_eq!(game.screen, Screen::Loading);
        let run = game.world.as_ref().unwrap();
        assert_eq!(
            (run.level.id, run.level.hard, run.players.len()),
            (0, true, 4)
        );
        assert_eq!(
            (run.score, run.player_scores, run.lives),
            (10, [1, 2, 3, 4], 1)
        );
    }

    #[test]
    fn hard_ending_is_not_dismissed_by_gameplay_buttons() {
        let mut game = game();
        game.start(13, 1, true).unwrap();
        game.screen = Screen::Ending;
        for _ in 0..4 {
            game.update([0xff; 4]);
            game.update([0; 4]);
        }
        assert_eq!(game.screen, Screen::Ending);
    }

    #[test]
    fn hard_ending_music_follows_the_original_cinematic_cues() {
        let mut game = game();
        game.start(13, 1, true).unwrap();
        game.screen = Screen::StageClear;
        game.transition_ticks = 1;
        assert!(game.update([0; 4]).contains(&GameEvent::StopMusic));
        let mut cues = Vec::new();
        for tick in 1..=2300 {
            for event in game.update([0; 4]) {
                if let GameEvent::Music(music) = event {
                    cues.push((tick, music));
                }
            }
        }
        assert_eq!(cues, [(390, Music::Fanfare), (2148, Music::Dreamy)]);
    }

    #[test]
    fn ordinary_respawn_shows_loading_without_advancing_the_world() {
        let mut game = game();
        game.start(0, 1, false).unwrap();
        game.presentation_enabled = true;
        let run = game.world.as_mut().unwrap();
        run.enemies.clear();
        run.props.clear();
        run.player_scores[0] = 12;
        run.players[0].damage();
        for _ in 0..128 {
            game.update([0; 4]);
        }
        assert_eq!(game.screen, Screen::Loading);
        let tick = game.world.as_ref().unwrap().tick;
        for _ in 0..crate::scenes::LOADING_TICKS {
            game.update([0; 4]);
        }
        assert_eq!(game.screen, Screen::Playing);
        let run = game.world.as_ref().unwrap();
        assert_eq!((run.tick, run.lives, run.player_scores[0]), (tick, 2, 12));
    }

    #[test]
    fn ordinary_stage_transition_preserves_player_scores_and_shows_loading() {
        let mut game = game();
        game.start(0, 4, false).unwrap();
        game.presentation_enabled = true;
        let world = game.world.as_mut().unwrap();
        world.score = 100;
        world.player_scores = [10, 20, 30, 40];
        game.screen = Screen::StageClear;
        game.transition_ticks = 1;
        let events = game.update([0; 4]);
        assert_eq!(game.screen, Screen::Loading);
        assert!(events.contains(&GameEvent::StopMusic));
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, GameEvent::Music(_)))
        );
        let world = game.world.as_ref().unwrap();
        assert_eq!(world.level.id, 1);
        assert_eq!(world.player_scores, [10, 20, 30, 40]);
        assert_eq!(world.tick, 0);
        let mut music = Vec::new();
        for tick in 1..=crate::scenes::LOADING_TICKS {
            for event in game.update([0; 4]) {
                if let GameEvent::Music(track) = event {
                    music.push((tick, track));
                }
            }
        }
        assert_eq!(music, [(294, Music::Spooky)]);
        assert_eq!(game.screen, Screen::Playing);
        assert_eq!(game.world.as_ref().unwrap().tick, 0);
        game.update([0; 4]);
        assert_eq!(game.world.as_ref().unwrap().tick, 1);
    }
    #[test]
    fn title_selection_starts_four_independent_players() {
        let mut game = game();
        for _ in 0..2 {
            game.update([1, 0, 0, 0]);
            game.update([0; 4]);
        }
        assert_eq!(game.screen, Screen::PlayerSelect);
        for _ in 0..3 {
            game.update([128, 0, 0, 0]);
            game.update([0; 4]);
        }
        game.update([1, 0, 0, 0]);
        game.update([0; 4]);
        assert_eq!(game.screen, Screen::Password);
        game.update([1, 0, 0, 0]);
        assert_eq!(game.player_count, 4);
        assert_eq!(game.screen, Screen::Loading);
        for _ in 0..crate::scenes::LOADING_TICKS {
            game.update([0; 4]);
        }
        assert_eq!(game.screen, Screen::Playing);
        game.update([128, 0, 0, 64]);
        let run = game.world.as_ref().unwrap();
        assert!(run.players[0].body.velocity.x > 0.0);
        assert!(run.players[3].body.velocity.x < 0.0);
        assert_eq!(run.players[2].body.velocity.x, 0.0);
    }
    #[test]
    fn boot_reaches_title_without_starting_or_skipping_the_intro() {
        let mut game = game();
        assert_eq!(game.screen, Screen::Intro);
        for tick in 1..crate::scenes::INTRO_TICKS {
            game.update([0; 4]);
            assert_eq!(game.screen, Screen::Intro, "tick {tick}");
            assert!(game.world.is_none());
        }
        game.update([0; 4]);
        assert_eq!(game.screen, Screen::Title);
        for _ in 0..600 {
            game.update([0; 4]);
        }
        assert_eq!(game.screen, Screen::Title);
        assert!(game.world.is_none());
    }
    #[test]
    fn intro_skip_requires_release_before_confirming_title() {
        let mut game = game();
        for _ in 0..120 {
            game.update([1, 0, 0, 0]);
        }
        assert_eq!(game.screen, Screen::Title);
        game.update([0; 4]);
        game.update([1, 0, 0, 0]);
        assert_eq!(game.screen, Screen::PlayerSelect);
    }
    #[test]
    fn password_digits_wrap_one_through_nine_like_original_entry() {
        let mut game = game();
        game.screen = Screen::Password;
        for expected in [1, 2, 3, 4, 5, 6, 7, 8, 9, 1] {
            game.update([16, 0, 0, 0]);
            game.update([0; 4]);
            assert_eq!(game.password[0], expected);
        }
        game.update([32, 0, 0, 0]);
        game.update([0; 4]);
        assert_eq!(game.password[0], 9);
        for _ in 0..8 {
            game.update([128, 0, 0, 0]);
            game.update([0; 4]);
        }
        assert_eq!(game.password_cursor, 3);
        for _ in 0..8 {
            game.update([64, 0, 0, 0]);
            game.update([0; 4]);
        }
        assert_eq!(game.password_cursor, 0);
    }
    #[test]
    fn original_passwords_select_towers_and_invalid_codes_do_not_start() {
        for (digits, stage, hard) in [
            ([2, 8, 6, 9], 3, false),
            ([3, 6, 4, 5], 6, false),
            ([7, 6, 4, 1], 9, false),
            ([1, 3, 9, 4], 0, true),
            ([6, 4, 1, 1], 3, true),
            ([4, 9, 1, 3], 6, true),
            ([2, 5, 5, 6], 9, true),
        ] {
            let mut game = game();
            game.screen = Screen::Password;
            game.password = digits;
            game.update([1, 0, 0, 0]);
            assert_eq!(game.screen, Screen::Loading);
            let world = game.world.as_ref().unwrap();
            assert_eq!((world.level.id, world.level.hard), (stage, hard));
            let pos = world.players[0].body.position;
            for _ in 0..crate::scenes::LOADING_TICKS - 1 {
                game.update([128; 4]);
            }
            assert_eq!(game.screen, Screen::Loading);
            assert_eq!(game.world.as_ref().unwrap().players[0].body.position, pos);
            assert_eq!(game.world.as_ref().unwrap().tick, 0);
            game.update([0; 4]);
            assert_eq!(game.screen, Screen::Playing);
        }
        let mut game = game();
        game.screen = Screen::Password;
        game.password = [9; 4];
        let events = game.update([1, 0, 0, 0]);
        assert_eq!(game.screen, Screen::Password);
        assert!(game.world.is_none());
        assert_eq!(game.password, [0; 4]);
        assert!(events.contains(&GameEvent::Sound(Sound::DamageEnemy)));
    }
    #[test]
    fn normal_campaign_ends_before_hard_final_boss() {
        for (hard, last) in [(false, 12), (true, 13)] {
            let mut game = game();
            game.start(last, 1, hard).unwrap();
            game.screen = Screen::StageClear;
            game.transition_ticks = 1;
            let events = game.update([0; 4]);
            assert_eq!(game.screen, Screen::Ending);
            assert!(events.contains(&GameEvent::Victory));
        }
        let mut game = game();
        game.start(12, 1, true).unwrap();
        game.screen = Screen::StageClear;
        game.transition_ticks = 1;
        game.update([0; 4]);
        assert_eq!(game.world.as_ref().unwrap().level.id, 13);
        assert_eq!(game.screen, Screen::Playing);
    }
    #[test]
    fn checkpoint_respawn_consumes_one_life_and_preserves_score() {
        let mut game = game();
        game.start(0, 1, false).unwrap();
        let run = game.world.as_mut().unwrap();
        run.enemies.clear();
        run.props.clear();
        run.checkpoint = Vec2::new(100.0, 600.0);
        run.score = 200;
        run.players[0].damage();
        for _ in 0..128 {
            game.update([0; 4]);
        }
        let run = game.world.as_ref().unwrap();
        assert_eq!(run.players[0].body.position, run.checkpoint);
        assert_eq!(run.lives, 2);
        assert_eq!(run.score, 200);
        assert_eq!(run.players[0].state, PlayerState::Alive);
    }
    #[test]
    fn last_life_transitions_to_game_over_once() {
        let mut game = game();
        game.start(0, 1, false).unwrap();
        let run = game.world.as_mut().unwrap();
        run.enemies.clear();
        run.props.clear();
        run.lives = 0;
        run.players[0].damage();
        let mut count = 0;
        for _ in 0..150 {
            count += game
                .update([0; 4])
                .iter()
                .filter(|&&e| e == GameEvent::GameOver)
                .count();
        }
        assert_eq!(game.screen, Screen::GameOver);
        assert_eq!(count, 1);
    }
    #[test]
    fn portal_requires_ground_contact_and_finishes_after_sixteen_ticks() {
        let mut game = game();
        game.start(0, 1, false).unwrap();
        let run = game.world.as_mut().unwrap();
        run.enemies.clear();
        let start = run.players[0].body.position;
        let destination = Vec2::new(200.0, start.y - 80.0);
        run.props = [start, destination]
            .into_iter()
            .map(|position| Prop {
                pickup: None,
                kind: PropKind::Portal(0x2e),
                source_kind: 0x2e,
                body: Body::new(position, Vec2::new(8.0, 8.0)),
                active: true,
                flip_x: false,
            })
            .collect();
        let mut events = Vec::new();
        run.update_props([InputFrame::default(); 4], &mut events);
        assert!(
            !run.players[0].is_warping(),
            "airborne contact cannot enter portal"
        );
        run.players[0].body.grounded = true;
        run.update_props([InputFrame::default(); 4], &mut events);
        assert!(run.players[0].is_warping());
        assert_eq!(run.players[0].body.position, start);
        assert_eq!(events, vec![GameEvent::Sound(Sound::Portal)]);
        for _ in 0..16 {
            game.update([0; 4]);
        }
        let player = &game.world.as_ref().unwrap().players[0];
        assert!(!player.is_warping());
        assert_eq!(player.body.position, destination);
        assert_eq!(player.body.velocity.y, -3.0);
    }
    #[test]
    fn player_spells_do_not_hurt_teammates_and_cannot_exceed_two() {
        let mut game = game();
        game.start(0, 2, false).unwrap();
        let run = game.world.as_mut().unwrap();
        run.enemies.clear();
        run.props.clear();
        run.level.collision.fill(0);
        run.players[1].body.position = run.players[0].body.position;
        let event = PlayerEvent::Shot {
            direction: Vec2::new(1.0, 0.0),
            charged: false,
        };
        let mut events = Vec::new();
        for _ in 0..10 {
            run.player_events(0, vec![event], &mut events);
        }
        assert_eq!(run.projectiles.len(), 2);
        run.update_projectiles(&mut events);
        assert_eq!(run.players[1].health, 1);
        assert_eq!(run.players[1].state, PlayerState::Alive);
    }
    #[test]
    fn exit_transitions_once_and_preserves_campaign_stats() {
        let mut game = game();
        game.start(0, 1, false).unwrap();
        let run = game.world.as_mut().unwrap();
        run.enemies.clear();
        run.level.collision.fill(0);
        run.score = 300;
        run.lives = 2;
        run.props = vec![Prop {
            pickup: None,
            kind: PropKind::Exit,
            source_kind: 0x1c,
            body: run.players[0].body,
            active: true,
            flip_x: false,
        }];
        let events = game.update([0; 4]);
        assert!(events.contains(&GameEvent::StageCompleted));
        assert!(events.contains(&GameEvent::FadeMusic));
        assert!(!events.contains(&GameEvent::Music(Music::Fanfare)));
        assert!(events.contains(&GameEvent::Sound(Sound::LevelExit)));
        assert!(events.contains(&GameEvent::Sound(Sound::LevelExitAlternate)));
        assert_eq!(game.screen, Screen::StageClear);
        for _ in 0..256 {
            game.update([0; 4]);
        }
        let run = game.world.as_ref().unwrap();
        assert_eq!(run.level.id, 1);
        assert_eq!(run.score, 302);
        assert_eq!(run.player_scores, [2, 0, 0, 0]);
        assert_eq!(run.lives, 2);
    }
    #[test]
    fn clear_countdown_accounts_for_callback_frame_phase() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/boss_clear_counter_reference.json"
        ))
        .unwrap();
        for sample in reference["samples"].as_array().unwrap() {
            let stage = sample["stage"].as_u64().unwrap() as usize;
            let mut game = game();
            game.start(stage, 1, true).unwrap();
            game.frame = sample["source_frame_before_callback"].as_u64().unwrap();
            let world = game.world.as_mut().unwrap();
            world.props.clear();
            for enemy in &mut world.enemies {
                enemy.health = 0;
            }
            game.update([0; 4]);
            assert_eq!(game.screen, Screen::StageClear);
            assert_eq!(
                u64::from(game.transition_ticks),
                sample["remaining_updates_after_callback_frame"]
                    .as_u64()
                    .unwrap()
            );
        }
    }

    #[test]
    fn exit_countdown_keeps_teammates_live_without_restarting_a_wipe() {
        let mut game = game();
        game.start(0, 2, false).unwrap();
        let world = game.world.as_mut().unwrap();
        world.enemies.clear();
        world.level.collision.fill(0);
        world.players[1].body.position.x = 40.0;
        world.props = vec![Prop {
            pickup: None,
            kind: PropKind::Exit,
            source_kind: 0x1c,
            body: world.players[0].body,
            active: true,
            flip_x: false,
        }];
        game.update([0; 4]);
        assert_eq!(game.screen, Screen::StageClear);
        let before = game.world.as_ref().unwrap().players[1].body.position.x;
        for _ in 0..12 {
            game.update([0, 0x80, 0, 0]);
        }
        assert!(game.world.as_ref().unwrap().players[1].body.position.x > before);
        let world = game.world.as_mut().unwrap();
        world.players[1].body = world.props[0].body;
        game.update([0; 4]);
        let world = game.world.as_mut().unwrap();
        assert_eq!(world.player_scores, [2, 2, 0, 0]);
        assert!(world.players[1].is_exiting());
        let lives = world.lives;
        for player in &mut world.players {
            player.state = PlayerState::Dead;
        }
        for _ in 0..150 {
            game.update([0; 4]);
        }
        assert_eq!(game.screen, Screen::StageClear);
        assert_eq!(game.world.as_ref().unwrap().lives, lives);
        assert_eq!(game.world.as_ref().unwrap().player_scores, [2, 2, 0, 0]);
    }

    #[test]
    fn segmented_completion_waits_for_the_original_lift_out_sequence() {
        let mut game = game();
        game.start(11, 1, true).unwrap();
        let world = game.world.as_mut().unwrap();
        world.enemies.clear();
        world.props.clear();
        world.camera.position.y = 0.0;
        world.players[0] = crate::player::Player::new(0, Vec2::new(124.0, 144.0));
        world.level.collision[22 * 32..23 * 32].fill(1);
        game.screen = Screen::StageClear;
        game.screen_ticks = 0;
        game.transition_ticks = 510;
        for tick in 1..=334 {
            game.update([0; 4]);
            assert_eq!(game.screen, Screen::StageClear, "tick {tick}");
        }
        assert_eq!(
            game.world.as_ref().unwrap().players[0].state,
            PlayerState::Dead
        );
        game.update([0; 4]);
        assert_eq!(game.screen, Screen::Playing);
        assert_eq!(game.world.as_ref().unwrap().level.id, 12);
    }

    #[test]
    fn every_campaign_stage_runs_deterministically_with_four_players() {
        let assets = Arc::new(Assets::load(&crate::assets::default_root()).unwrap());
        for hard in [false, true] {
            for id in 0..if hard { 14 } else { 13 } {
                let mut a = Game::new(assets.clone());
                let mut b = Game::new(assets.clone());
                a.start(id, 4, hard).unwrap();
                b.start(id, 4, hard).unwrap();
                for tick in 0..600 {
                    let input = std::array::from_fn(|player| {
                        let phase = tick + player * 17;
                        (if phase % 90 < 45 { 128 } else { 64 })
                            | (if phase % 40 < 20 { 1 } else { 0 })
                            | (if phase % 60 < 30 { 2 } else { 0 })
                    });
                    assert_eq!(a.update(input), b.update(input));
                }
                let a = a.world.as_ref().unwrap();
                let b = b.world.as_ref().unwrap();
                for (a, b) in a.players.iter().zip(&b.players) {
                    assert_eq!(a.body, b.body);
                    assert!(a.body.position.x.is_finite() && a.body.position.y.is_finite());
                }
            }
        }
    }
    #[test]
    fn stage_transition_preserves_each_players_health_and_feather() {
        for hard in [false, true] {
            for count in 1..=4 {
                let mut game = game();
                game.start(0, count, hard).unwrap();
                let run = game.world.as_mut().unwrap();
                for player in &mut run.players {
                    player.health = 1 + (player.id % 2) as u8;
                    player.feather = player.id % 2 == 0;
                }
                game.screen = Screen::StageClear;
                game.transition_ticks = 1;
                game.update([0; 4]);
                let run = game.world.as_ref().unwrap();
                assert_eq!(run.level.id, 1);
                for player in &run.players {
                    assert_eq!(player.health, 1 + (player.id % 2) as u8);
                    assert_eq!(player.feather, player.id % 2 == 0);
                }
            }
        }
    }

    #[test]
    fn entering_the_final_flight_arena_heals_and_removes_feathers() {
        for count in 1..=4 {
            let mut game = game();
            game.start(12, count, true).unwrap();
            let world = game.world.as_mut().unwrap();
            world.score = 125;
            world.lives = 1;
            for player in &mut world.players {
                player.health = 1;
                player.feather = true;
            }
            game.screen = Screen::StageClear;
            game.transition_ticks = 1;
            game.update([0; 4]);
            let world = game.world.as_ref().unwrap();
            assert_eq!(world.level.id, 13);
            assert_eq!((world.score, world.lives), (125, 1));
            for player in &world.players {
                assert!(player.flying());
                assert_eq!(player.health, 2);
                assert!(!player.feather);
            }
        }
    }
    #[test]
    fn three_reserve_lives_allow_four_attempts_in_both_modes() {
        for hard in [false, true] {
            for count in 1..=4 {
                let mut game = game();
                game.start(0, count, hard).unwrap();
                for attempt in 0..4 {
                    let world = game.world.as_mut().unwrap();
                    world.enemies.clear();
                    world.props.clear();
                    for player in &mut world.players {
                        player.invulnerability = 0;
                        player.damage();
                    }
                    let mut game_over_events = 0;
                    for _ in 0..128 {
                        game_over_events += game
                            .update([0; 4])
                            .iter()
                            .filter(|&&e| e == GameEvent::GameOver)
                            .count();
                    }
                    assert_eq!(game_over_events, usize::from(attempt == 3));
                    assert_eq!(
                        game.screen,
                        if attempt == 3 {
                            Screen::GameOver
                        } else {
                            Screen::Playing
                        }
                    );
                }
            }
        }
    }
}
