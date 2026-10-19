//! Search legal idle timing when joining a campaign prefix to a known route.
use micro_mages_re::{
    assets::{Assets, default_root},
    game::{Game, Screen},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[derive(Serialize, Deserialize)]
struct Replay {
    stage: usize,
    hard: bool,
    players: usize,
    buttons: Vec<[u8; 4]>,
    #[serde(default)]
    campaign: bool,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: splice_replays PREFIX ROUTE OUTPUT_TRACE (partial campaign, not completion fixture)".into());
    }
    let prefix: Replay = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let route: Replay = serde_json::from_slice(&std::fs::read(&args[2])?)?;
    if prefix.hard != route.hard || prefix.players != route.players {
        return Err("mode/player mismatch".into());
    }
    let assets = Arc::new(Assets::load(&default_root())?);
    for offset in (1..=240).step_by(12) {
        let split = prefix.buttons.len().saturating_sub(offset);
        let mut initial = Game::new(assets.clone());
        initial.start(prefix.stage, prefix.players, prefix.hard)?;
        for &input in &prefix.buttons[..split] {
            if !matches!(initial.screen, Screen::Playing | Screen::StageClear) {
                return Err("prefix ended before splice point".into());
            }
            initial.update(input);
        }
        for delay in 0..256 {
            let mut game = initial.clone();
            let mut suffix = vec![[0; 4]; delay];
            suffix.extend_from_slice(&prefix.buttons[split..]);
            for &input in &suffix {
                if !matches!(game.screen, Screen::Playing | Screen::StageClear) {
                    break;
                }
                game.update(input);
            }
            if game.screen != Screen::StageClear {
                continue;
            }
            for _ in 0..2048 {
                if game.screen != Screen::StageClear {
                    break;
                }
                game.update([0; 4]);
                suffix.push([0; 4]);
            }
            if game.screen != Screen::Playing
                || game.world.as_ref().unwrap().level.id != route.stage
            {
                continue;
            }
            for &input in &route.buttons {
                if game.screen != Screen::Playing {
                    break;
                }
                game.update(input);
                suffix.push(input);
            }
            if game.screen == Screen::StageClear {
                let mut buttons = prefix.buttons[..split].to_vec();
                buttons.extend(suffix);
                let result = Replay {
                    stage: prefix.stage,
                    hard: prefix.hard,
                    players: prefix.players,
                    buttons,
                    campaign: true,
                };
                std::fs::write(&args[3], serde_json::to_vec(&result)?)?;
                println!(
                    "Legal prefix reaches stage {} clear at {} ticks: inserted {} idle ticks, {} before previous exit",
                    route.stage,
                    result.buttons.len(),
                    delay,
                    offset
                );
                return Ok(());
            }
        }
    }
    Err("No successful legal splice found".into())
}
