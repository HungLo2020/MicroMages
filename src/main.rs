use micro_mages_re::{
    assets::{self, Assets},
    audio::Audio,
    controls::Controls,
    display::DisplaySettings,
    events::GameEvent,
    game::Game,
    storage::{Paths, Preferences},
    video::{HEIGHT, WIDTH},
};
use sdl2::keyboard::Scancode as Key;
use std::{
    env,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
    time::{Duration, Instant},
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

fn navigation(keys: &[Key]) -> [u8; 4] {
    let mut buttons = 0;
    for (key, bit) in [
        (Key::Return, 1),
        (Key::Z, 1),
        (Key::X, 2),
        (Key::Backspace, 2),
        (Key::Up, 16),
        (Key::Down, 32),
        (Key::Left, 64),
        (Key::Right, 128),
    ] {
        if keys.contains(&key) {
            buttons |= bit;
        }
    }
    [buttons, 0, 0, 0]
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
                    "Micro Mages — native Rust reconstruction\n\nPlayer 1 defaults: WASD + G/H | Player 2: Arrows + Z/X/Enter\nAlt+Enter: fullscreen | Esc or mapped Pause: settings (audio, remappable controls, display, restart, quit)\n\nOptions: --mute --headless --frames N --demo --screenshot FILE.png --stage N --players 1..4 --hard --replay FILE.json\nGraphics, game data, and audio are supplied by the repository assets directory.\nGameplay uses native Rust systems and editable PNG/JSON/Ogg assets."
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
    let paths = Paths::discover()?;
    paths.create_user_directories()?;
    let preferences_file = paths.preferences_file();
    let mut preferences = Preferences::load_or_create(&preferences_file)?;
    let controls_file = paths.controls_file();
    let mut controls = Controls::load_or_create(&controls_file)?;
    let display_file = paths.display_file();
    let mut display_settings = DisplaySettings::load_or_create(&display_file)?;
    let mut display_dirty = None;
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
    if let Some(audio) = &mut audio {
        audio.settings(preferences.music, preferences.effects, false);
    }
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
        Some(window::create(display_settings)?)
    };
    if let Some(window) = &window
        && window.settings() != display_settings
    {
        display_settings = window.settings();
        display_dirty = Some(Instant::now());
    }
    let texture_creator = window.as_ref().map(window::NativeWindow::texture_creator);
    let mut texture = texture_creator
        .as_ref()
        .map(window::NativeWindow::create_texture)
        .transpose()?;
    let mut pads = if headless {
        None
    } else {
        Some(micro_mages_re::input::Pads::new())
    };
    let mut menu = menu::Menu::new();
    menu.music = preferences.music;
    menu.effects = preferences.effects;
    menu.controls = controls.clone();
    menu.display = display_settings;
    let mut resume_guard = false;
    let mut fullscreen_guard = false;
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
        let mut held_keys = Vec::new();
        let mut toggle_menu = false;
        if let Some(window) = &mut window {
            window.poll();
            if !window.is_open() {
                break;
            }
            toggle_menu = window.is_key_pressed(Key::Escape);
            held_keys = window.get_keys();
            if window.fullscreen_shortcut() {
                let requested = DisplaySettings {
                    fullscreen: !window.settings().fullscreen,
                    ..window.settings()
                };
                if let Err(error) = window.apply(requested) {
                    eprintln!("Display change failed: {error}");
                    menu.status = "FULLSCREEN UNAVAILABLE".into();
                }
                fullscreen_guard = true;
                resume_guard = true;
            }
            if fullscreen_guard {
                fullscreen_guard = held_keys
                    .iter()
                    .any(|key| matches!(key, Key::Return | Key::LAlt | Key::RAlt));
                held_keys.clear();
            }
            let settings = window.settings();
            if settings != display_settings {
                let previous = display_settings;
                display_settings = settings;
                display_dirty = Some(Instant::now());
                menu.window_resized(previous, display_settings);
            }
        }
        let pad_state = pads.as_mut().map(|pads| pads.sample()).unwrap_or_default();
        let keyboard_masks = controls.keyboard_masks(&held_keys);
        let pad_masks = controls.gamepad_masks(&pad_state.buttons, pad_state.sticks);
        let mut inputs = keyboard_masks;
        for (mask, pad) in inputs.iter_mut().zip(pad_masks) {
            *mask |= pad;
        }
        if resume_guard && !menu.open {
            if inputs.iter().all(|&mask| mask == 0) {
                resume_guard = false;
            }
            inputs = [0; 4];
        }
        if menu.open {
            resume_guard = true;
            inputs = navigation(&held_keys);
            // Fixed navigation wins over overlapping gameplay rebinds. Other
            // mapped Pause keys/buttons can still close the settings menu.
            if inputs[0] == 0 {
                for (mask, keyboard) in inputs.iter_mut().zip(keyboard_masks) {
                    *mask |= keyboard & 8;
                }
            }
            for (id, pad) in pad_state.navigation().into_iter().enumerate() {
                inputs[id] |= if pad == 0 { pad_masks[id] & 8 } else { pad };
            }
        }
        let ui = menu.handle_input(
            if demo { [0; 4] } else { inputs },
            toggle_menu,
            &held_keys,
            &pad_state.buttons,
        );
        if ui.paused {
            resume_guard = true;
        }
        if menu.controls != controls {
            menu.controls.save(&controls_file)?;
            controls = menu.controls.clone();
        }
        if display_dirty.is_some_and(|time: Instant| time.elapsed() >= Duration::from_millis(500)) {
            display_settings.save(&display_file)?;
            display_dirty = None;
        }
        if (menu.music, menu.effects) != (preferences.music, preferences.effects) {
            preferences.music = menu.music;
            preferences.effects = menu.effects;
            preferences.save(&preferences_file)?;
        }
        match ui.command {
            menu::Command::Quit => break,
            menu::Command::Restart => {
                game = Game::new(assets.clone());
                if let Some(audio) = &mut audio {
                    audio.handle(GameEvent::StopMusic);
                }
            }
            menu::Command::ApplyDisplay(settings) => {
                if let Some(window) = &mut window {
                    match window.apply(settings) {
                        Ok(()) => {
                            display_settings = window.settings();
                            menu.display = display_settings;
                            display_settings.save(&display_file)?;
                            display_dirty = None;
                            menu.status = "DISPLAY SAVED".into();
                        }
                        Err(error) => {
                            eprintln!("Display change failed: {error}");
                            menu.status = "DISPLAY CHANGE FAILED".into();
                        }
                    }
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
        if let (Some(window), Some(texture)) = (&mut window, &mut texture) {
            window.present(texture, &displayed, display_settings.scaling)?;
            if let Some(remaining) =
                Duration::from_secs_f64(1.0 / 60.0).checked_sub(last_frame.elapsed())
            {
                std::thread::sleep(remaining);
            }
        }
        ticks += 1;
    }
    if display_dirty.is_some() {
        display_settings.save(&display_file)?;
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
