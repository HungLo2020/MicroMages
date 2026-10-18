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
                    "Micro Mages — native Rust reconstruction\n\nArrow keys: move | Z/X: action buttons | Enter: start | Tab: select\nP: pause | R: restart | Esc: quit\n\nOptions: --mute --headless --frames N --demo --screenshot FILE.ppm\nThe original installed game supplies graphics/data/audio during the build.\nGameplay is compiled to native Rust; no opcode interpreter is included."
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    if headless && frames.is_none() {
        return Err("--headless requires --frames N".into());
    }
    let assets = env::var_os("MICRO_MAGES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("MICRO_MAGES_BUILD_ASSETS")));
    let mut game = Game::new(&assets, sound);
    let mut window = if headless {
        None
    } else {
        let mut window = Window::new(
            "Micro Mages — Rust | Arrows + Z/X | Enter start | P pause | Esc quit",
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
    let mut paused = false;
    let mut ticks = 0;
    loop {
        if frames.is_some_and(|limit| ticks >= limit) {
            break;
        }
        let mut inputs = [0; 4];
        if let Some(window) = &mut window {
            if !window.is_open() || window.is_key_down(Key::Escape) {
                break;
            }
            if window.is_key_pressed(Key::P, KeyRepeat::No) {
                paused = !paused;
            }
            if window.is_key_pressed(Key::R, KeyRepeat::No) {
                game = Game::new(&assets, sound);
            }
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
                inputs[0] |= 8;
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
        if !paused {
            game.buttons(inputs);
            game.update().map_err(|e| format!("frame {ticks}: {e}"))?;
        }
        let pixels = game.video.render();
        if let Some(window) = &mut window {
            window.update_with_buffer(pixels, WIDTH, HEIGHT)?;
        }
        ticks += 1;
    }
    if let Some(path) = capture {
        screenshot(&path, game.video.render())?;
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
