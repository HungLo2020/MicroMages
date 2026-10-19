use micro_mages_re::{
    assets::{self, Assets},
    audio::Audio,
    events::GameEvent,
    game::Game,
    video::{HEIGHT, WIDTH},
};
use minifb::{Key, KeyRepeat, Scale, Window, WindowOptions};
use std::{
    env,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
    time::Instant,
};

mod menu;
mod window;

fn screenshot(path: &Path, pixels: &[u32]) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    if path.extension().is_some_and(|extension| extension == "png") {
        let mut encoder = png::Encoder::new(file, WIDTH as u32, HEIGHT as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        let bytes: Vec<_> = pixels
            .iter()
            .flat_map(|&pixel| [(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8])
            .collect();
        writer.write_image_data(&bytes)?;
        return Ok(());
    }
    write!(file, "P6\n{WIDTH} {HEIGHT}\n255\n")?;
    for &pixel in pixels {
        file.write_all(&[(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8])?;
    }
    Ok(())
}

fn keys(window: &Window, map: [Key; 8]) -> u8 {
    map.into_iter().enumerate().fold(0, |mask, (bit, key)| {
        mask | ((window.is_key_down(key) as u8) << bit)
    })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Replay {
    /// Omit the stage to exercise the ordinary startup and title flow.
    #[serde(default)]
    stage: Option<usize>,
    hard: bool,
    players: usize,
    buttons: Vec<[u8; 4]>,
    #[serde(default, rename = "campaign")]
    _campaign: bool,
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut headless = false;
    let mut frames = None;
    let mut demo = false;
    let mut sound = true;
    let mut capture = None;
    let mut stage = None;
    let mut players = 1;
    let mut hard = false;
    let mut replay_path = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--headless" => {
                headless = true;
                sound = false;
            }
            "--mute" => sound = false,
            "--stage" => {
                stage = Some(
                    args.next()
                        .ok_or("--stage needs an index")?
                        .parse::<usize>()?,
                )
            }
            "--players" => {
                players = args
                    .next()
                    .ok_or("--players needs a count")?
                    .parse::<usize>()?
            }
            "--hard" => hard = true,
            "--replay" => {
                replay_path = Some(PathBuf::from(
                    args.next().ok_or("--replay needs a JSON path")?,
                ))
            }
            "--demo" => demo = true,
            "--frames" => {
                frames = Some(
                    args.next()
                        .ok_or("--frames needs a count")?
                        .parse::<u64>()?,
                )
            }
            "--screenshot" => {
                capture = Some(PathBuf::from(
                    args.next().ok_or("--screenshot needs a PNG path")?,
                ))
            }
            "--help" => {
                println!(
                    "Micro Mages — native Rust reconstruction\n\nArrow keys: move | Z/X: action buttons | Enter: confirm | Tab: select\nEsc/P or gamepad Start: settings (resume, audio, controls, restart, quit)\n\nOptions: --mute --headless --frames N --demo --screenshot FILE.png --stage N --players 1..4 --hard --replay FILE.json\nGraphics, game data, and audio are supplied by the repository assets directory.\nGameplay uses native Rust systems and editable PNG/JSON/Ogg assets."
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    let replay: Option<Replay> = replay_path
        .map(|path| assets::read_json(&path))
        .transpose()?;
    if let Some(replay) = &replay {
        if replay.buttons.is_empty() {
            return Err("replay contains no inputs".into());
        }
        stage = replay.stage;
        players = replay.players;
        hard = replay.hard;
        frames = Some(
            frames
                .unwrap_or(replay.buttons.len() as u64)
                .min(replay.buttons.len() as u64),
        );
    }
    if headless && frames.is_none() {
        return Err("--headless requires --frames N".into());
    }
    let assets = Arc::new(Assets::load(&assets::default_root())?);
    let mut game = Game::new(assets.clone());
    if !(1..=4).contains(&players) {
        return Err("--players must be 1..4".into());
    }
    game.player_count = players;
    game.hard = hard;
    let mut audio = if sound {
        match Audio::new(&assets.root) {
            Ok(audio) => Some(audio),
            Err(error) => {
                eprintln!("Audio unavailable: {error}");
                None
            }
        }
    } else {
        None
    };
    let initial = if let Some(stage) = stage {
        game.start(stage, players, hard)?
    } else {
        Vec::new()
    };
    if let Some(audio) = &mut audio {
        for event in initial {
            audio.handle(event);
        }
    }
    let mut video = micro_mages_re::video::Video::new();
    let mut window = if headless {
        None
    } else {
        let mut window = Window::new(
            "Micro Mages — Rust | Arrows + Z/X | Enter confirm | Esc / Start settings",
            WIDTH,
            HEIGHT,
            WindowOptions {
                scale: Scale::X4,
                resize: true,
                ..WindowOptions::default()
            },
        )?;
        window::configure(&window)?;
        window.set_target_fps(60);
        Some(window)
    };
    let mut pads = if headless {
        None
    } else {
        Some(micro_mages_re::input::Pads::new())
    };
    let mut menu = menu::Menu::new();
    let mut displayed = vec![0; WIDTH * HEIGHT];
    let mut ticks = 0;
    let mut replay_tick = 0usize;
    let mut last_frame = Instant::now();
    let mut clock = micro_mages_re::timing::FrameClock::default();
    loop {
        if frames.is_some_and(|limit| {
            if replay.is_some() {
                replay_tick as u64 >= limit
            } else {
                ticks >= limit
            }
        }) {
            break;
        }
        let mut inputs = [0; 4];
        let mut toggle_menu = false;
        if let Some(window) = &mut window {
            if !window.is_open() {
                break;
            }
            toggle_menu = window.is_key_pressed(Key::Escape, KeyRepeat::No)
                || window.is_key_pressed(Key::P, KeyRepeat::No);
            inputs[0] = keys(
                window,
                [
                    Key::Z,
                    Key::X,
                    Key::Tab,
                    Key::Enter,
                    Key::Up,
                    Key::Down,
                    Key::Left,
                    Key::Right,
                ],
            );
            // Enter confirms the game's "press A" prompt. Physical Start belongs
            // to host settings, as advertised by the original title screen.
            inputs[0] &= !8;
            if window.is_key_down(Key::Enter) {
                inputs[0] |= 1;
            }
            inputs[1] = keys(
                window,
                [
                    Key::G,
                    Key::H,
                    Key::T,
                    Key::Y,
                    Key::W,
                    Key::S,
                    Key::A,
                    Key::D,
                ],
            );
            inputs[2] = keys(
                window,
                [
                    Key::N,
                    Key::M,
                    Key::U,
                    Key::O,
                    Key::I,
                    Key::K,
                    Key::J,
                    Key::L,
                ],
            );
            inputs[3] = keys(
                window,
                [
                    Key::NumPad0,
                    Key::NumPadDot,
                    Key::NumPad5,
                    Key::NumPadEnter,
                    Key::NumPad8,
                    Key::NumPad2,
                    Key::NumPad4,
                    Key::NumPad6,
                ],
            );
        }
        if let Some(pads) = &mut pads {
            for (mask, pad) in inputs.iter_mut().zip(pads.poll()) {
                *mask |= pad;
            }
        }
        let ui = menu.handle(if demo { [0; 4] } else { inputs }, toggle_menu);
        match ui.command {
            menu::Command::Quit => break,
            menu::Command::Restart => {
                game = Game::new(assets.clone());
                if let Some(audio) = &mut audio {
                    audio.handle(GameEvent::StopMusic);
                }
            }
            menu::Command::None => {}
        }
        if let Some(audio) = &mut audio {
            audio.settings(menu.music, menu.effects, menu.open);
        }
        let now = Instant::now();
        let elapsed = now.duration_since(last_frame);
        last_frame = now;
        let steps = if headless {
            usize::from(!ui.paused)
        } else {
            clock.advance(elapsed, ui.paused)
        };
        for _ in 0..steps {
            // Demo is a deterministic game-logic test, not physical host input.
            let buttons = if let Some(replay) = &replay {
                if replay_tick >= frames.unwrap() as usize {
                    break;
                }
                let buttons = replay.buttons[replay_tick];
                replay_tick += 1;
                buttons
            } else if demo {
                demo_buttons(game.screen, game.frame)
            } else {
                ui.buttons
            };
            let events = game.update(buttons);
            if let Some(audio) = &mut audio {
                for event in events {
                    audio.handle(event);
                }
                audio.update();
            }
        }
        let audio_available = audio.is_some();
        let pixels = video.render(&game, &assets);
        displayed.copy_from_slice(if menu.open {
            menu.render(pixels, audio_available)
        } else {
            pixels
        });
        if let Some(window) = &mut window {
            window.update_with_buffer(&displayed, WIDTH, HEIGHT)?;
        }
        ticks += 1;
    }
    if let Some(path) = capture {
        screenshot(&path, &displayed)?;
    }
    println!("Native game: {} simulation ticks", game.frame);
    println!("Screen: {:?}", game.screen);
    if let Some(world) = &game.world {
        println!(
            "Run: stage {}, players {}, hard {}",
            world.level.id,
            world.players.len(),
            world.level.hard
        );
    }
    Ok(())
}

fn demo_buttons(screen: micro_mages_re::game::Screen, tick: u64) -> [u8; 4] {
    use micro_mages_re::game::Screen;
    let mut buttons = [0; 4];
    match screen {
        Screen::Intro | Screen::Title | Screen::PlayerSelect | Screen::Password => {
            if tick % 120 < 6 {
                buttons[0] = 1;
            }
        }
        Screen::Playing => {
            buttons[0] = if tick % 240 < 120 { 0x80 } else { 0x40 };
            if tick % 45 < 25 {
                buttons[0] |= 1;
            }
            if tick % 18 < 9 {
                buttons[0] |= 2;
            }
        }
        _ => {}
    }
    buttons
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
