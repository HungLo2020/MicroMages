//! Linux desktop presentation, fullscreen, and physical keyboard input.
use micro_mages_re::{
    display::{DisplaySettings, Scaling, Viewport},
    video::{HEIGHT, WIDTH},
};
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::Scancode,
    pixels::{Color, PixelFormatEnum},
    rect::Rect,
    render::{Canvas, ScaleMode, Texture, TextureCreator},
    video::{FullscreenType, Window, WindowContext},
};
use std::collections::HashSet;

type Error = Box<dyn std::error::Error>;

pub struct NativeWindow {
    canvas: Canvas<Window>,
    events: sdl2::EventPump,
    held: HashSet<Scancode>,
    pressed: HashSet<Scancode>,
    open: bool,
    settings: DisplaySettings,
    _sdl: sdl2::Sdl,
}

pub fn create(settings: DisplaySettings) -> Result<NativeWindow, Error> {
    settings.validate()?;
    let version = sdl2::version::version();
    if version
        < (sdl2::version::Version {
            major: 2,
            minor: 0,
            patch: 12,
        })
    {
        return Err(format!("SDL2 2.0.12 or newer is required (found {version})").into());
    }
    sdl2::hint::set("SDL_VIDEO_X11_WMCLASS", "MicroMagesRust");
    sdl2::hint::set("SDL_APP_NAME", "Micro Mages");
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let (width, height) = DisplaySettings::windowed_size(settings.width, settings.height);
    let mut window = video
        .window(
            "Micro Mages | Esc: Settings | Alt+Enter: Fullscreen",
            width as u32,
            height as u32,
        )
        .position_centered()
        .resizable()
        .build()?;
    window.set_minimum_size(WIDTH as u32, HEIGHT as u32)?;
    let canvas = window.into_canvas().build()?;
    let events = sdl.event_pump()?;
    let mut result = NativeWindow {
        canvas,
        events,
        held: HashSet::new(),
        pressed: HashSet::new(),
        open: true,
        settings: DisplaySettings {
            fullscreen: false,
            width,
            height,
            ..settings
        },
        _sdl: sdl,
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
    pub fn texture_creator(&self) -> TextureCreator<WindowContext> {
        self.canvas.texture_creator()
    }
    pub fn create_texture(creator: &TextureCreator<WindowContext>) -> Result<Texture<'_>, Error> {
        Ok(creator.create_texture_streaming(
            PixelFormatEnum::ARGB8888,
            WIDTH as u32,
            HEIGHT as u32,
        )?)
    }
    pub fn poll(&mut self) {
        self.pressed.clear();
        for event in self.events.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::Window {
                    win_event: WindowEvent::Close,
                    ..
                } => self.open = false,
                Event::Window {
                    win_event: WindowEvent::FocusLost,
                    ..
                } => self.held.clear(),
                Event::KeyDown {
                    scancode: Some(key),
                    repeat: false,
                    ..
                } => {
                    self.held.insert(key);
                    self.pressed.insert(key);
                }
                Event::KeyUp {
                    scancode: Some(key),
                    ..
                } => {
                    self.held.remove(&key);
                }
                _ => {}
            }
        }
        if !self.settings.fullscreen {
            let (width, height) = self.canvas.window().size();
            let size = DisplaySettings::windowed_size(width as usize, height as usize);
            if size != (width as usize, height as usize) {
                let _ = self
                    .canvas
                    .window_mut()
                    .set_size(size.0 as u32, size.1 as u32);
            }
            if DisplaySettings::valid_size(size.0, size.1) {
                (self.settings.width, self.settings.height) = size;
            }
        }
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn is_key_pressed(&self, key: Scancode) -> bool {
        self.pressed.contains(&key)
    }
    pub fn get_keys(&self) -> Vec<Scancode> {
        self.held.iter().copied().collect()
    }
    pub fn fullscreen_shortcut(&self) -> bool {
        self.pressed.contains(&Scancode::Return)
            && (self.held.contains(&Scancode::LAlt) || self.held.contains(&Scancode::RAlt))
    }
    pub fn apply(&mut self, settings: DisplaySettings) -> Result<(), Error> {
        settings.validate()?;
        if settings.fullscreen {
            let window = self.canvas.window();
            let desktop = window
                .subsystem()
                .desktop_display_mode(window.display_index()?)?;
            if Viewport::fit(desktop.w as usize, desktop.h as usize).is_none() {
                return Err(
                    "screen is too narrow for full-height fullscreen; use windowed mode".into(),
                );
            }
            self.canvas
                .window_mut()
                .set_fullscreen(FullscreenType::Desktop)?;
        } else {
            self.canvas
                .window_mut()
                .set_fullscreen(FullscreenType::Off)?;
            let size = DisplaySettings::windowed_size(settings.width, settings.height);
            self.canvas
                .window_mut()
                .set_size(size.0 as u32, size.1 as u32)?;
        }
        let (width, height) = DisplaySettings::windowed_size(settings.width, settings.height);
        self.settings = DisplaySettings {
            width,
            height,
            ..settings
        };
        Ok(())
    }
    pub fn present(
        &mut self,
        texture: &mut Texture<'_>,
        pixels: &[u32],
        scaling: Scaling,
    ) -> Result<(), Error> {
        texture.set_scale_mode(match scaling {
            Scaling::Fit => ScaleMode::Nearest,
            Scaling::Smooth => ScaleMode::Linear,
        });
        texture.with_lock(None, |bytes, pitch| {
            for (source, row) in pixels
                .as_chunks::<WIDTH>()
                .0
                .iter()
                .zip(bytes.chunks_mut(pitch))
            {
                for (pixel, destination) in source.iter().zip(row.as_chunks_mut::<4>().0.iter_mut())
                {
                    destination.copy_from_slice(&(*pixel | 0xff00_0000).to_ne_bytes());
                }
            }
        })?;
        let (width, height) = self.canvas.output_size()?;
        if width == 0 || height == 0 {
            return Ok(());
        }
        // A resize may be pending in the compositor. Keep the previous frame
        // until its corrected size arrives rather than crop or letterbox it.
        let Some(viewport) = Viewport::fit(width as usize, height as usize) else {
            return Ok(());
        };
        self.canvas.set_draw_color(Color::RGB(0, 0, 0));
        self.canvas.clear();
        self.canvas.copy(
            texture,
            None,
            Rect::new(
                viewport.x as i32,
                0,
                viewport.width as u32,
                viewport.height as u32,
            ),
        )?;
        self.canvas.present();
        Ok(())
    }
}
