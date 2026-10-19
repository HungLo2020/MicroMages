//! Broad mode/player-count coverage supplements recorded completion replays.
//! These are stress scenarios, not evidence that a campaign was completed.
use micro_mages_re::{
    assets::{Assets, default_root},
    game::{Game, Screen},
    video::{HEIGHT, Video, WIDTH},
};
use std::sync::Arc;

#[test]
fn every_stage_mode_and_player_count_survives_distinct_inputs_and_rendering() {
    let assets = Arc::new(Assets::load(&default_root()).unwrap());
    let mut video = Video::new();
    for hard in [false, true] {
        for stage in 0..if hard { 14 } else { 13 } {
            for players in 1..=4 {
                let mut game = Game::new(assets.clone());
                game.start(stage, players, hard).unwrap();
                for tick in 0..1200usize {
                    let buttons = std::array::from_fn(|id| {
                        if id >= players {
                            return 0;
                        }
                        let time = tick + id * 19;
                        (if time % 160 < 80 { 128 } else { 64 })
                            | u8::from(time % 43 < 25)
                            | if time % 95 < 70 { 2 } else { 0 }
                            | if time % 240 < 40 { 16 } else { 0 }
                    });
                    game.update(buttons);
                    if tick % 30 == 0 {
                        let frame = video.render(&game, &assets);
                        assert_eq!(frame.len(), WIDTH * HEIGHT);
                        assert!(frame.iter().all(|pixel| pixel & 0xff00_0000 == 0));
                    }
                    if let Some(world) = &game.world {
                        for player in &world.players {
                            assert!(player.body.position.x.is_finite());
                            assert!(player.body.position.y.is_finite());
                            assert!(player.body.velocity.x.is_finite());
                            assert!(player.body.velocity.y.is_finite());
                            assert!(
                                player.body.center().x >= 4.0 && player.body.center().x < 253.0,
                                "stage={stage} hard={hard} players={players} tick={tick} id={} center={:?}",
                                player.id,
                                player.body.center()
                            );
                        }
                    }
                    if game.screen == Screen::GameOver {
                        break;
                    }
                }
            }
        }
    }
}
