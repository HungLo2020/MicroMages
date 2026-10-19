//! Outcome tests driven only by recorded legal controller inputs.
//!
//! Unlike synthetic unit scenarios, these never set positions, health, enemy
//! state, or completion flags. Each recording starts through the public API.
use micro_mages_re::{
    assets::{Assets, default_root},
    game::{Game, Screen},
};
use serde::Deserialize;
use std::{collections::HashSet, path::PathBuf, sync::Arc};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Replay {
    stage: usize,
    hard: bool,
    players: usize,
    buttons: Vec<[u8; 4]>,
    #[serde(default)]
    campaign: bool,
}

#[test]
fn recorded_legal_inputs_reach_stage_clear_without_state_injection() {
    let assets = Arc::new(Assets::load(&default_root()).unwrap());
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/replays");
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "No independently verified completion recordings"
    );
    let mut stages = HashSet::new();
    let mut campaigns = HashSet::new();
    for path in files {
        let replay: Replay = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        if replay.campaign {
            campaigns.insert((replay.hard, replay.players));
        } else {
            stages.insert((replay.stage, replay.hard, replay.players));
        }
        assert!(
            !replay.buttons.is_empty(),
            "{}: empty recording",
            path.display()
        );
        assert!(
            replay.buttons.len() <= if replay.campaign { 240000 } else { 36000 },
            "{}: unreasonable recording length",
            path.display()
        );
        let mut game = Game::new(assets.clone());
        game.start(replay.stage, replay.players, replay.hard)
            .unwrap();
        let mut camera_y = game.world.as_ref().unwrap().camera.position.y;
        let mut lives = game.world.as_ref().unwrap().lives;
        let mut stage = game.world.as_ref().unwrap().level.id;
        for (tick, &buttons) in replay.buttons.iter().enumerate() {
            assert!(buttons[replay.players..].iter().all(|&button| button == 0));
            assert!(
                game.screen == Screen::Playing
                    || (replay.campaign && game.screen == Screen::StageClear),
                "{}: ended before tick{tick}",
                path.display()
            );
            game.update(buttons);
            let world = game.world.as_ref().unwrap();
            let height = (world.level.height * 8) as f32;
            assert!(
                world.camera.position.y >= 0.0
                    && world.camera.position.y <= (height - 240.0).max(0.0)
            );
            if world.lives == lives && world.level.id == stage {
                assert!(
                    world.camera.position.y <= camera_y,
                    "{}: camera scrolled down without a respawn",
                    path.display()
                );
            }
            camera_y = world.camera.position.y;
            lives = world.lives;
            stage = world.level.id;
        }
        assert_eq!(
            game.screen,
            if replay.campaign {
                Screen::Ending
            } else {
                Screen::StageClear
            },
            "{}: legal inputs failed to complete",
            path.display()
        );
        if !replay.campaign {
            for _ in 0..512 {
                if game.screen != Screen::StageClear {
                    break;
                }
                game.update([0; 4]);
            }
            let final_stage = if replay.hard { 13 } else { 12 };
            assert_eq!(
                game.screen,
                if replay.stage == final_stage {
                    Screen::Ending
                } else {
                    Screen::Playing
                },
                "{}: stage transition failed",
                path.display()
            );
            assert_eq!(game.player_count, replay.players);
            assert_eq!(game.hard, replay.hard);
            let world = game.world.as_ref().unwrap();
            assert_eq!(world.players.len(), replay.players);
            assert_eq!(world.level.hard, replay.hard);
            if replay.stage != final_stage {
                assert_eq!(world.level.id, replay.stage + 1);
            }
        }
    }
    assert!(
        campaigns.contains(&(false, 1)),
        "missing complete normal solo campaign"
    );
    assert!(
        campaigns.contains(&(true, 1)),
        "missing complete hard solo campaign"
    );
    assert!(
        campaigns.contains(&(false, 4)),
        "missing complete normal four-player campaign"
    );
    assert!(
        campaigns.contains(&(true, 4)),
        "missing complete hard four-player campaign"
    );
    assert_eq!(stages.len(), 108, "complete standalone replay matrix");
    for hard in [false, true] {
        let stage_count = if hard { 14 } else { 13 };
        for stage in 0..stage_count {
            for players in 1..=4 {
                assert!(
                    stages.contains(&(stage, hard, players)),
                    "missing completed stage: stage={stage} hard={hard} players={players}"
                );
            }
        }
    }
}

#[test]
fn legal_inputs_enter_the_boss_arena_through_the_solid_doorway() {
    let recording: Replay = serde_json::from_str(include_str!(
        "fixtures/arena_entry_normal_stage02_player1.json"
    ))
    .unwrap();
    let assets = Arc::new(Assets::load(&default_root()).unwrap());
    let mut game = Game::new(assets);
    game.start(recording.stage, recording.players, recording.hard)
        .unwrap();
    for buttons in recording.buttons {
        game.update(buttons);
    }
    let world = game.world.as_ref().unwrap();
    assert_eq!(game.screen, Screen::Playing);
    assert_eq!(world.lives, 3, "entrance route must not rely on respawning");
    assert_eq!(world.camera.position.y, 0.0);
    assert!(
        world.players[0].body.center().y < 200.0,
        "legal jumps should reach the trigger and scripted arena lift"
    );
    assert!(
        world
            .enemies
            .iter()
            .any(|e| matches!(e.kind, micro_mages_re::enemy::EnemyKind::Boss(_)) && e.health > 0)
    );
}
