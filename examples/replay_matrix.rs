//! Reuse verified button recordings across player counts and full campaigns.
//! Only the public start/update API is used; failed outcomes are reported.
use micro_mages_re::{
    assets::{Assets, default_root},
    game::{Game, Screen},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
#[derive(Serialize, Deserialize)]
struct Replay {
    stage: usize,
    hard: bool,
    players: usize,
    buttons: Vec<[u8; 4]>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    campaign: bool,
}
fn path(root: &std::path::Path, hard: bool, stage: usize, players: usize) -> PathBuf {
    root.join(format!(
        "{}-stage{stage:02}-players{players}.json",
        if hard { "hard" } else { "normal" }
    ))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let campaign = std::env::args().any(|a| a == "--campaign");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/replays");
    let assets = Arc::new(Assets::load(&default_root())?);
    if std::env::args().any(|arg| arg == "--validate-all") {
        let mut files: Vec<_> = std::fs::read_dir(&root)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<_, _>>()?;
        files.retain(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        });
        files.sort();
        let mut failed = 0;
        for path in &files {
            let recording: Replay = serde_json::from_slice(&std::fs::read(path)?)?;
            let mut game = Game::new(assets.clone());
            game.start(recording.stage, recording.players, recording.hard)?;
            let mut ended_early = false;
            for buttons in recording.buttons {
                if game.screen != Screen::Playing
                    && !(recording.campaign && game.screen == Screen::StageClear)
                {
                    ended_early = true;
                    break;
                }
                game.update(buttons);
            }
            let expected = if recording.campaign {
                Screen::Ending
            } else {
                Screen::StageClear
            };
            if !ended_early && game.screen == expected {
                println!("PASS {}", path.display());
            } else {
                failed += 1;
                let world = game.world.as_ref().unwrap();
                eprintln!(
                    "FAIL {}: {:?}, lives {}, player {:?}",
                    path.display(),
                    game.screen,
                    world.lives,
                    world.players[0].body.position
                );
            }
        }
        println!(
            "Verified {}/{} existing recordings; this does not assert full matrix coverage",
            files.len() - failed,
            files.len()
        );
        return if failed == 0 && !files.is_empty() {
            Ok(())
        } else {
            Err(format!("{failed} failed recordings").into())
        };
    }
    let mut passed = 0;
    let mut attempted = 0;
    let mut campaigns = 0;
    for hard in [false, true] {
        for players in 1..=4 {
            let mut game = Game::new(assets.clone());
            if campaign {
                game.start(0, players, hard)?;
            }
            let mut all = Vec::new();
            let mut complete = true;
            for stage in 0..if hard { 14 } else { 13 } {
                let exact = path(&root, hard, stage, players);
                let source = if exact.exists() {
                    exact.clone()
                } else {
                    (1..=4)
                        .map(|count| path(&root, hard, stage, count))
                        .find(|candidate| candidate.exists())
                        .unwrap_or_else(|| path(&root, hard, stage, 1))
                };
                if !source.exists() {
                    eprintln!("MISSING hard={hard} stage={stage} players={players}");
                    complete = false;
                    if campaign {
                        break;
                    } else {
                        continue;
                    }
                }
                let recording: Replay = serde_json::from_slice(&std::fs::read(source)?)?;
                if !campaign {
                    game = Game::new(assets.clone());
                    game.start(stage, players, hard)?;
                }
                let mut buttons = Vec::new();
                attempted += 1;
                for original in recording.buttons {
                    let mut input = original;
                    if recording.players != players {
                        input = [0; 4];
                        input[..players].fill(original[0]);
                    }
                    game.update(input);
                    buttons.push(input);
                    if game.screen != Screen::Playing {
                        break;
                    }
                }
                if game.screen != Screen::StageClear {
                    eprintln!(
                        "FAIL hard={hard} stage={stage} players={players} outcome={:?}",
                        game.screen
                    );
                    complete = false;
                    if campaign {
                        break;
                    } else {
                        continue;
                    }
                }
                passed += 1;
                if !campaign {
                    std::fs::write(
                        exact,
                        serde_json::to_string(&Replay {
                            stage,
                            hard,
                            players,
                            buttons,
                            campaign: false,
                        })?,
                    )?;
                } else {
                    all.extend(buttons);
                    let mut waiting = 0;
                    while game.screen == Screen::StageClear && waiting < 512 {
                        let input = [0; 4];
                        game.update(input);
                        all.push(input);
                        waiting += 1;
                    }
                    if game.screen == Screen::StageClear {
                        return Err("transition never completed".into());
                    }
                }
                println!("PASS hard={hard} stage={stage} players={players} campaign={campaign}");
            }
            if campaign && complete && game.screen == Screen::Ending {
                let name = format!(
                    "campaign-{}-players{players}.json",
                    if hard { "hard" } else { "normal" }
                );
                std::fs::write(
                    root.join(name),
                    serde_json::to_string(&Replay {
                        stage: 0,
                        hard,
                        players,
                        buttons: all,
                        campaign: true,
                    })?,
                )?;
                campaigns += 1;
                println!("COMPLETE CAMPAIGN hard={hard} players={players}");
            }
        }
    }
    println!(
        "Verified outcomes {passed}/{attempted}; unavailable stage recordings are reported separately"
    );
    if campaign {
        if campaigns != 8 {
            return Err(format!("Only {campaigns}/8 continuous campaigns verified").into());
        }
    } else if passed != 108 {
        return Err(format!("Only {passed}/108 stage/mode/player combinations verified ({attempted} recordings attempted)").into());
    }
    Ok(())
}
