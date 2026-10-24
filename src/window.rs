//! Linux desktop presentation, fullscreen policy and physical keyboard state.
//!
//! `quasar-window` owns the native window, desktop identity and device state;
//! `quasar-render-2d` presents the 256x240 software framebuffer through
//! VulkanicGAL. The game keeps its own scaling, window-size and input rules.
use micro_mages_re::{
    controls::KeyboardKey,
    display::{DisplaySettings, Scaling},
    video::{HEIGHT, WIDTH},
};
use quasar_input::Key;
use quasar_render_2d::{DisplayOptions, FramebufferPresenter, PresentOutcome};
use quasar_window::{
    DesktopConfig, GameWindow, WindowConfig, X11Identity,
    winit::{
        dpi::PhysicalSize,
        event::{DeviceEvent, WindowEvent},
        event_loop::ActiveEventLoop,
    },
};
use std::collections::HashSet;

type Error = Box<dyn std::error::Error>;

const TITLE: &str = "Micro Mages | Esc: Settings | Alt+Enter: Fullscreen";
const APPLICATION_ID: &str = "MicroMagesRust";

pub struct NativeWindow {
    /// Declared first so the GPU surface is released before its window.
    presenter: FramebufferPresenter,
    window: GameWindow,
    held: HashSet<KeyboardKey>,
    previous_escape: bool,
    pressed: HashSet<KeyboardKey>,
    escape_pressed: bool,
    open: bool,
    settings: DisplaySettings,
    applied_scaling: Scaling,
    surface: (u32, u32),
    rgba: Vec<u8>,
}

pub fn create(
    event_loop: &ActiveEventLoop,
    settings: DisplaySettings,
) -> Result<NativeWindow, Error> {
    settings.validate()?;
    let (width, height) = DisplaySettings::clamped_size(settings.width, settings.height);
    let config = WindowConfig {
        title: TITLE.into(),
        application_id: APPLICATION_ID.into(),
        size: (width as u32, height as u32),
        // Any window shape is acceptable: the Quasar viewport letterboxes or
        // pillarboxes the 16:15 image instead of restricting the window.
        minimum_size: (1, 1),
        fullscreen: false,
        resizable: true,
    };
    // Keep the historical X11 identity instead of inheriting an unrelated one.
    let desktop = DesktopConfig {
        x11: Some(X11Identity::with_names(APPLICATION_ID, APPLICATION_ID)),
        ..DesktopConfig::for_application(APPLICATION_ID)
    };
    let window = GameWindow::create_with_desktop(event_loop, &config, &desktop)?;
    // WindowConfig sizes are logical desktop units; the game stores physical
    // pixels, so request the exact framebuffer-era window size.
    let _ = window
        .native()
        .request_inner_size(PhysicalSize::new(width as u32, height as u32));
    let size = window.native().inner_size();
    let surface = (size.width.max(1), size.height.max(1));
    let presenter = FramebufferPresenter::new(
        window.shared_native(),
        surface,
        (WIDTH as u32, HEIGHT as u32),
        settings.options(),
    )?;
    let mut result = NativeWindow {
        presenter,
        window,
        held: HashSet::new(),
        previous_escape: false,
        pressed: HashSet::new(),
        escape_pressed: false,
        open: true,
        settings: DisplaySettings {
            fullscreen: false,
            width,
            height,
            ..settings
        },
        applied_scaling: settings.scaling,
        surface,
        rgba: vec![0; WIDTH * HEIGHT * 4],
    };
    if settings.fullscreen
        && let Err(error) = result.apply(settings)
    {
        eprintln!("Fullscreen unavailable; using windowed mode: {error}");
    }
    Ok(result)
}

impl NativeWindow {
    pub fn settings(&self) -> DisplaySettings {
        self.settings
    }

    /// Forward one native event; device state lives in the Quasar window.
    pub fn handle_event(&mut self, event: &WindowEvent) {
        let update = self.window.process(event);
        if update.close_requested {
            self.open = false;
        }
    }

    pub fn handle_device_event(&mut self, event: &DeviceEvent) {
        self.window.process_device(event);
    }

    /// Collect this frame's keyboard state and enforce the window-size policy.
    pub fn poll(&mut self) {
        self.pressed.clear();
        let input = &self.window.input;
        let mut held = HashSet::new();
        for &key in KeyboardKey::ALL {
            if input.held(key.native()) {
                held.insert(key);
            }
            // winit repeats physical presses while a key is held. Bindings and
            // menus are edge-triggered, so only the first press counts.
            if input.pressed(key.native()) && !self.held.contains(&key) {
                self.pressed.insert(key);
            }
        }
        let escape = input.pressed(Key::Escape) && !self.previous_escape;
        self.previous_escape = input.held(Key::Escape);
        self.escape_pressed = escape;
        self.held = held;
        self.window.input.consume_edges();

        // Remember a user-chosen windowed size without reshaping the window.
        if !self.settings.fullscreen {
            let size = self.window.native().inner_size();
            if DisplaySettings::valid_size(size.width as usize, size.height as usize) {
                (self.settings.width, self.settings.height) =
                    (size.width as usize, size.height as usize);
            }
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn escape_pressed(&self) -> bool {
        self.escape_pressed
    }

    pub fn get_keys(&self) -> Vec<KeyboardKey> {
        KeyboardKey::ALL
            .iter()
            .copied()
            .filter(|key| self.held.contains(key))
            .collect()
    }

    pub fn fullscreen_shortcut(&self) -> bool {
        self.pressed.contains(&KeyboardKey::Enter)
            && (self.held.contains(&KeyboardKey::LeftAlt)
                || self.held.contains(&KeyboardKey::RightAlt))
    }

    pub fn apply(&mut self, settings: DisplaySettings) -> Result<(), Error> {
        settings.validate()?;
        if settings.fullscreen {
            // Borderless fullscreen keeps the desktop mode; the viewport fits
            // the image inside the whole screen on either orientation.
            self.window.set_fullscreen(true);
        } else {
            self.window.set_fullscreen(false);
            let size = DisplaySettings::clamped_size(settings.width, settings.height);
            let _ = self
                .window
                .native()
                .request_inner_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
        }
        let (width, height) = DisplaySettings::clamped_size(settings.width, settings.height);
        self.settings = DisplaySettings {
            width,
            height,
            ..settings
        };
        Ok(())
    }

    /// Upload and present one 256x240 `0x00RRGGBB` frame.
    pub fn present(&mut self, pixels: &[u32], scaling: Scaling) -> Result<(), Error> {
        if !self.window.drawable() {
            return Ok(());
        }
        let size = self.window.native().inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        // Resizes, fullscreen transitions and DPI changes all arrive here as a
        // new physical surface size.
        if (size.width, size.height) != self.surface {
            self.surface = (size.width, size.height);
            self.presenter.resize(size.width, size.height);
        }
        if scaling != self.applied_scaling {
            self.applied_scaling = scaling;
            let options: DisplayOptions = scaling.options();
            self.presenter.set_options(options)?;
        }
        for (destination, &pixel) in self.rgba.as_chunks_mut::<4>().0.iter_mut().zip(pixels) {
            destination.copy_from_slice(&[
                (pixel >> 16) as u8,
                (pixel >> 8) as u8,
                pixel as u8,
                0xff,
            ]);
        }
        // `Deferred` means the surface is minimized or stale; the next frame
        // presents after the restore resize.
        match self.presenter.present(&self.rgba)? {
            PresentOutcome::Presented | PresentOutcome::Deferred => Ok(()),
        }
    }
}
