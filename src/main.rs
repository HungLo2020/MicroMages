use micro_mages_re::{
    assets::{self, Assets},
    audio::Audio,
    controls::{Controls, KeyboardKey},
    display::DisplaySettings,
    events::GameEvent,
    game::Game,
    input::Pads,
    storage::{Paths, Preferences},
    video::{HEIGHT, WIDTH},
};
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
#[cfg(feature = "desktop")]
mod window;

/// One redraw/tick opportunity per display refresh, matching the previous loop.
#[cfg(feature = "desktop")]
const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);

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

fn navigation(keys: &[KeyboardKey]) -> [u8; 4] {
    let mut buttons = 0;
    for (key, bit) in [
        (KeyboardKey::Enter, 1),
        (KeyboardKey::Z, 1),
        (KeyboardKey::X, 2),
        (KeyboardKey::Backspace, 2),
        (KeyboardKey::Up, 16),
        (KeyboardKey::Down, 32),
        (KeyboardKey::Left, 64),
        (KeyboardKey::Right, 128),
    ] {
        if keys.contains(&key) {
            buttons |= bit;
        }
    }
    [buttons, 0, 0, 0]
}

#[derive(Clone, serde::Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flow {
    Continue,
    Done,
}

struct Args {
    headless: bool,
    frames: Option<u64>,
    demo: bool,
    sound: bool,
    capture: Option<PathBuf>,
    stage: Option<usize>,
    players: usize,
    hard: bool,
    replay: Option<Replay>,
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
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
                std::process::exit(0);
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
    if !(1..=4).contains(&players) {
        return Err("--players must be 1..4".into());
    }
    Ok(Args {
        headless,
        frames,
        demo,
        sound,
        capture,
        stage,
        players,
        hard,
        replay,
    })
}

/// Everything that can be loaded before a window exists.
struct Loaded {
    assets: Arc<Assets>,
    preferences_file: PathBuf,
    controls_file: PathBuf,
    display_file: PathBuf,
    preferences: Preferences,
    controls: Controls,
    display: DisplaySettings,
    game: Game,
    audio: Option<Audio>,
}

fn load(args: &Args) -> Result<Loaded, Box<dyn std::error::Error>> {
    let assets = Arc::new(Assets::load(&assets::default_root())?);
    let paths = Paths::discover()?;
    paths.create_user_directories()?;
    let preferences_file = paths.preferences_file();
    let preferences = Preferences::load_or_create(&preferences_file)?;
    let controls_file = paths.controls_file();
    let controls = Controls::load_or_create(&controls_file)?;
    let display_file = paths.display_file();
    let display = DisplaySettings::load_or_create(&display_file)?;
    let mut game = Game::new(assets.clone());
    game.player_count = args.players;
    game.hard = args.hard;
    let mut audio = if args.sound {
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
    if let Some(stage) = args.stage {
        let initial = game.start(stage, args.players, args.hard)?;
        if let Some(audio) = &mut audio {
            for event in initial {
                audio.handle(event);
            }
        }
    }
    Ok(Loaded {
        assets,
        preferences_file,
        controls_file,
        display_file,
        preferences,
        controls,
        display,
        game,
        audio,
    })
}

/// Window-facing facts collected once per frame.
struct WindowFrame {
    closed: bool,
    toggle_menu: bool,
    held_keys: Vec<KeyboardKey>,
    fullscreen_shortcut: bool,
    settings: DisplaySettings,
}

struct Session {
    #[cfg(feature = "desktop")]
    window: Option<window::NativeWindow>,
    assets: Arc<Assets>,
    preferences_file: PathBuf,
    controls_file: PathBuf,
    display_file: PathBuf,
    preferences: Preferences,
    controls: Controls,
    display_settings: DisplaySettings,
    display_dirty: Option<Instant>,
    game: Game,
    audio: Option<Audio>,
    video: micro_mages_re::video::Video,
    pads: Option<Pads>,
    menu: menu::Menu,
    resume_guard: bool,
    fullscreen_guard: bool,
    displayed: Vec<u32>,
    ticks: u64,
    replay_tick: usize,
    replay: Option<Replay>,
    demo: bool,
    frames: Option<u64>,
    capture: Option<PathBuf>,
    headless: bool,
    last_frame: Instant,
    clock: micro_mages_re::timing::FrameClock,
}

impl Session {
    fn new(
        args: &Args,
        loaded: Loaded,
        #[cfg(feature = "desktop")] window: Option<window::NativeWindow>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let Loaded {
            assets,
            preferences_file,
            controls_file,
            display_file,
            preferences,
            controls,
            display,
            game,
            audio,
        } = loaded;
        #[cfg(feature = "desktop")]
        let (display_settings, display_dirty) = match window.as_ref() {
            Some(window) if window.settings() != display => {
                (window.settings(), Some(Instant::now()))
            }
            _ => (display, None),
        };
        #[cfg(not(feature = "desktop"))]
        let (display_settings, display_dirty) = (display, None);
        let mut menu = menu::Menu::new();
        menu.music = preferences.music;
        menu.effects = preferences.effects;
        menu.controls = controls.clone();
        menu.display = display_settings;
        Ok(Self {
            #[cfg(feature = "desktop")]
            window,
            assets,
            preferences_file,
            controls_file,
            display_file,
            preferences,
            controls,
            display_settings,
            display_dirty,
            game,
            audio,
            video: micro_mages_re::video::Video::new(),
            pads: if args.headless {
                None
            } else {
                Some(Pads::new())
            },
            menu,
            resume_guard: false,
            fullscreen_guard: false,
            displayed: vec![0; WIDTH * HEIGHT],
            ticks: 0,
            replay_tick: 0,
            replay: args.replay.clone(),
            demo: args.demo,
            frames: args.frames,
            capture: args.capture.clone(),
            headless: args.headless,
            last_frame: Instant::now(),
            clock: micro_mages_re::timing::FrameClock::default(),
        })
    }

    #[cfg(feature = "desktop")]
    fn window_event(&mut self, event: &quasar_window::winit::event::WindowEvent) {
        if let Some(window) = &mut self.window {
            window.handle_event(event);
        }
    }

    #[cfg(feature = "desktop")]
    fn device_event(&mut self, event: &quasar_window::winit::event::DeviceEvent) {
        if let Some(window) = &mut self.window {
            window.handle_device_event(event);
        }
    }

    fn window_frame(&mut self) -> Option<WindowFrame> {
        #[cfg(feature = "desktop")]
        {
            let window = self.window.as_mut()?;
            window.poll();
            Some(WindowFrame {
                closed: !window.is_open(),
                toggle_menu: window.escape_pressed(),
                held_keys: window.get_keys(),
                fullscreen_shortcut: window.fullscreen_shortcut(),
                settings: window.settings(),
            })
        }
        #[cfg(not(feature = "desktop"))]
        {
            None
        }
    }

    fn has_window(&self) -> bool {
        #[cfg(feature = "desktop")]
        {
            self.window.is_some()
        }
        #[cfg(not(feature = "desktop"))]
        {
            false
        }
    }

    fn apply_display(
        &mut self,
        settings: DisplaySettings,
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(feature = "desktop")]
        if let Some(window) = &mut self.window {
            window.apply(settings)?;
            return Ok(());
        }
        let _ = settings;
        Ok(())
    }

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(feature = "desktop")]
        if let Some(window) = &mut self.window {
            window.present(&self.displayed, self.display_settings.scaling)?;
        }
        Ok(())
    }

    fn frame(&mut self) -> Result<Flow, Box<dyn std::error::Error>> {
        if self.frames.is_some_and(|limit| {
            if self.replay.is_some() {
                self.replay_tick as u64 >= limit
            } else {
                self.ticks >= limit
            }
        }) {
            return Ok(Flow::Done);
        }
        let mut held_keys = Vec::new();
        let mut toggle_menu = false;
        if let Some(frame) = self.window_frame() {
            if frame.closed {
                return Ok(Flow::Done);
            }
            toggle_menu = frame.toggle_menu;
            held_keys = frame.held_keys;
            if frame.fullscreen_shortcut {
                let requested = DisplaySettings {
                    fullscreen: !frame.settings.fullscreen,
                    ..frame.settings
                };
                if let Err(error) = self.apply_display(requested) {
                    eprintln!("Display change failed: {error}");
                    self.menu.status = "FULLSCREEN UNAVAILABLE".into();
                }
                self.fullscreen_guard = true;
                self.resume_guard = true;
            }
            if self.fullscreen_guard {
                self.fullscreen_guard = held_keys.iter().any(|key| {
                    matches!(
                        key,
                        KeyboardKey::Enter | KeyboardKey::LeftAlt | KeyboardKey::RightAlt
                    )
                });
                held_keys.clear();
            }
            if frame.settings != self.display_settings {
                let previous = self.display_settings;
                self.display_settings = frame.settings;
                self.display_dirty = Some(Instant::now());
                self.menu.window_resized(previous, self.display_settings);
            }
        }
        let pad_state = self
            .pads
            .as_mut()
            .map(|pads| pads.sample())
            .unwrap_or_default();
        let keyboard_masks = self.controls.keyboard_masks(&held_keys);
        let pad_masks = self
            .controls
            .gamepad_masks(&pad_state.buttons, pad_state.sticks);
        let mut inputs = keyboard_masks;
        for (mask, pad) in inputs.iter_mut().zip(pad_masks) {
            *mask |= pad;
        }
        if self.resume_guard && !self.menu.open {
            if inputs.iter().all(|&mask| mask == 0) {
                self.resume_guard = false;
            }
            inputs = [0; 4];
        }
        if self.menu.open {
            self.resume_guard = true;
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
        let ui = self.menu.handle_input(
            if self.demo { [0; 4] } else { inputs },
            toggle_menu,
            &held_keys,
            &pad_state.buttons,
        );
        if ui.paused {
            self.resume_guard = true;
        }
        if self.menu.controls != self.controls {
            self.menu.controls.save(&self.controls_file)?;
            self.controls = self.menu.controls.clone();
        }
        if self
            .display_dirty
            .is_some_and(|time| time.elapsed() >= Duration::from_millis(500))
        {
            self.display_settings.save(&self.display_file)?;
            self.display_dirty = None;
        }
        if (self.menu.music, self.menu.effects)
            != (self.preferences.music, self.preferences.effects)
        {
            self.preferences.music = self.menu.music;
            self.preferences.effects = self.menu.effects;
            self.preferences.save(&self.preferences_file)?;
        }
        match ui.command {
            menu::Command::Quit => return Ok(Flow::Done),
            menu::Command::Restart => {
                self.game = Game::new(self.assets.clone());
                if let Some(audio) = &mut self.audio {
                    audio.handle(GameEvent::StopMusic);
                }
            }
            menu::Command::ApplyDisplay(settings) => {
                let result = if self.has_window() {
                    self.apply_display(settings)
                } else {
                    Ok(())
                };
                match result {
                    Ok(()) if self.has_window() => {
                        #[cfg(feature = "desktop")]
                        if let Some(window) = &self.window {
                            self.display_settings = window.settings();
                        }
                        self.menu.display = self.display_settings;
                        self.display_settings.save(&self.display_file)?;
                        self.display_dirty = None;
                        self.menu.status = "DISPLAY SAVED".into();
                    }
                    Ok(()) => {}
                    Err(error) => {
                        eprintln!("Display change failed: {error}");
                        self.menu.status = "DISPLAY CHANGE FAILED".into();
                    }
                }
            }
            menu::Command::None => {}
        }
        if let Some(audio) = &mut self.audio {
            audio.settings(self.menu.music, self.menu.effects, self.menu.open);
        }
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame);
        self.last_frame = now;
        let steps = if self.headless {
            usize::from(!ui.paused)
        } else {
            self.clock.advance(elapsed, ui.paused)
        };
        for _ in 0..steps {
            // Demo is a deterministic game-logic test, not physical host input.
            let buttons = if let Some(replay) = &self.replay {
                if self.replay_tick >= self.frames.unwrap() as usize {
                    break;
                }
                let buttons = replay.buttons[self.replay_tick];
                self.replay_tick += 1;
                buttons
            } else if self.demo {
                demo_buttons(self.game.screen, self.game.frame)
            } else {
                ui.buttons
            };
            let events = self.game.update(buttons);
            if let Some(audio) = &mut self.audio {
                for event in events {
                    audio.handle(event);
                }
                audio.update();
            }
        }
        let audio_available = self.audio.is_some();
        let pixels = self.video.render(&self.game, &self.assets);
        self.displayed.copy_from_slice(if self.menu.open {
            self.menu.render(pixels, audio_available)
        } else {
            pixels
        });
        self.present()?;
        self.ticks += 1;
        Ok(Flow::Continue)
    }

    fn finish(mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.display_dirty.is_some() {
            self.display_settings.save(&self.display_file)?;
        }
        if let Some(path) = self.capture.take() {
            screenshot(&path, &self.displayed)?;
        }
        println!("Native game: {} simulation ticks", self.game.frame);
        println!("Screen: {:?}", self.game.screen);
        if let Some(world) = &self.game.world {
            println!(
                "Run: stage {}, players {}, hard {}",
                world.level.id,
                world.players.len(),
                world.level.hard
            );
        }
        Ok(())
    }
}

fn run_headless(args: Args, loaded: Loaded) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "desktop")]
    let mut session = Session::new(&args, loaded, None)?;
    #[cfg(not(feature = "desktop"))]
    let mut session = Session::new(&args, loaded)?;
    while session.frame()? == Flow::Continue {}
    session.finish()
}

#[cfg(feature = "desktop")]
type WindowFactory = Box<
    dyn FnOnce(
            &quasar_window::winit::event_loop::ActiveEventLoop,
        ) -> Result<window::NativeWindow, Box<dyn std::error::Error>>
        + 'static,
>;

#[cfg(feature = "desktop")]
fn run_desktop(
    args: Args,
    loaded: Loaded,
    create_window: impl FnOnce(
        &quasar_window::winit::event_loop::ActiveEventLoop,
    ) -> Result<window::NativeWindow, Box<dyn std::error::Error>>
    + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = quasar_window::winit::event_loop::EventLoop::new()?;
    let mut app = App {
        args,
        loaded: Some(loaded),
        create_window: Some(Box::new(create_window)),
        session: None,
        next_frame: None,
        failure: None,
    };
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.failure.take() {
        return Err(error);
    }
    Ok(())
}

#[cfg(feature = "desktop")]
struct App {
    args: Args,
    loaded: Option<Loaded>,
    create_window: Option<WindowFactory>,
    session: Option<Session>,
    next_frame: Option<Instant>,
    failure: Option<Box<dyn std::error::Error>>,
}

#[cfg(feature = "desktop")]
impl quasar_window::winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &quasar_window::winit::event_loop::ActiveEventLoop) {
        if self.session.is_some() {
            return;
        }
        let (Some(loaded), Some(create_window)) = (self.loaded.take(), self.create_window.take())
        else {
            return;
        };
        let window = match create_window(event_loop) {
            Ok(window) => window,
            Err(error) => {
                self.failure = Some(error);
                event_loop.exit();
                return;
            }
        };
        match Session::new(&self.args, loaded, Some(window)) {
            Ok(session) => self.session = Some(session),
            Err(error) => {
                self.failure = Some(error);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &quasar_window::winit::event_loop::ActiveEventLoop,
        _window_id: quasar_window::winit::window::WindowId,
        event: quasar_window::winit::event::WindowEvent,
    ) {
        if let Some(session) = &mut self.session {
            session.window_event(&event);
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &quasar_window::winit::event_loop::ActiveEventLoop,
        _device_id: quasar_window::winit::event::DeviceId,
        event: quasar_window::winit::event::DeviceEvent,
    ) {
        if let Some(session) = &mut self.session {
            session.device_event(&event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &quasar_window::winit::event_loop::ActiveEventLoop) {
        use quasar_window::winit::event_loop::ControlFlow;
        let start = Instant::now();
        // Native events can wake the loop early; keep the previous 60 Hz frame
        // pacing even when the compositor delivers unrelated traffic.
        if let Some(deadline) = self.next_frame
            && start < deadline
        {
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            return;
        }
        let Some(session) = &mut self.session else {
            return;
        };
        match session.frame() {
            Ok(Flow::Continue) => {}
            Ok(Flow::Done) => {
                event_loop.exit();
                return;
            }
            Err(error) => {
                self.failure = Some(error);
                event_loop.exit();
                return;
            }
        }
        let deadline = start + FRAME;
        self.next_frame = Some(deadline);
        event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
    }

    fn exiting(&mut self, _event_loop: &quasar_window::winit::event_loop::ActiveEventLoop) {
        if let Some(session) = self.session.take()
            && let Err(error) = session.finish()
        {
            eprintln!("{error}");
            self.failure = Some(error);
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;
    let loaded = load(&args)?;
    if args.headless {
        return run_headless(args, loaded);
    }
    #[cfg(feature = "desktop")]
    {
        let display = loaded.display;
        run_desktop(args, loaded, move |event_loop| {
            window::create(event_loop, display)
        })
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = loaded;
        Err("this build has no desktop presentation; rebuild with default features or use --headless"
            .into())
    }
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
