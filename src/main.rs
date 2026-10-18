use micro_mages_re::{
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
};

mod menu;
mod window;

fn screenshot(path: &Path, pixels: &[u32]) -> std::io::Result<()> {
    let mut file = File::create(path)?;
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

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut headless = false;
    let mut frames = None;
    let mut demo = false;
    let mut sound = true;
    let mut capture = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--headless" => {
                headless = true;
                sound = false;
            }
            "--mute" => sound = false,
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
                    args.next().ok_or("--screenshot needs a PPM path")?,
                ))
            }
            "--help" => {
                println!(
                    "Micro Mages — native Rust reconstruction\n\nArrow keys: move | Z/X: action buttons | Enter: confirm | Tab: select\nEsc/P or gamepad Start: settings (resume, audio, controls, restart, quit)\n\nOptions: --mute --headless --frames N --demo --screenshot FILE.ppm\nGraphics, game data, and audio are supplied by the repository assets directory.\nGameplay is compiled to native Rust; no opcode interpreter is included."
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    if headless && frames.is_none() {
        return Err("--headless requires --frames N".into());
    }
    let assets = PathBuf::from(env!("MICRO_MAGES_BUILD_ASSETS"));
    let mut game = Game::new(&assets, sound);
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
    loop {
        if frames.is_some_and(|limit| ticks >= limit) {
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
        }
        if demo {
            if (120..126).contains(&ticks) || (240..246).contains(&ticks) {
                inputs[0] |= 1;
            }
            if ticks > 300 {
                inputs[0] |= if ticks % 240 < 120 { 0x80 } else { 0x40 };
                if ticks % 45 < 25 {
                    inputs[0] |= 1;
                }
                if ticks % 18 < 9 {
                    inputs[0] |= 2;
                }
            }
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
                game.audio.take();
                game = Game::new(&assets, sound);
            }
            menu::Command::None => {}
        }
        if let Some(audio) = &mut game.audio {
            audio.settings(menu.music, menu.effects, menu.open);
        }
        if !ui.paused {
            // Demo is a deterministic game-logic test, not physical host input.
            game.buttons(if demo { inputs } else { ui.buttons });
            game.update().map_err(|e| format!("frame {ticks}: {e}"))?;
        }
        let audio_available = game.audio.is_some();
        let pixels = game.video.render();
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
    println!(
        "Native game: {} frames, {} compiled blocks executed",
        game.frame, game.blocks_executed
    );
    Ok(())
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
