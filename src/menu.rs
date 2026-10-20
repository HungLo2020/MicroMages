//! Native pause/settings UI, input rebinding, and window configuration.
use gilrs::Button;
use micro_mages_re::{
    controls::{Action, Controls, GamepadButton, KeyboardKey},
    display::DisplaySettings,
    font::glyph,
    video::{HEIGHT, WIDTH},
};
use sdl2::keyboard::Scancode as Key;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    None,
    Restart,
    Quit,
    ApplyDisplay(DisplaySettings),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Settings,
    Controls,
    Display,
    Restart,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Device {
    Keyboard,
    Gamepad,
}
#[derive(Clone, Copy)]
struct Capture {
    action: Action,
    armed: bool,
}

pub struct Frame {
    pub paused: bool,
    pub buttons: [u8; 4],
    pub command: Command,
}

pub struct Menu {
    pub open: bool,
    pub music: u8,
    pub effects: u8,
    pub controls: Controls,
    pub display: DisplaySettings,
    pub status: String,
    page: Page,
    selection: usize,
    player: usize,
    device: Device,
    capture: Option<Capture>,
    previous: [u8; 4],
    suppressed: [u8; 4],
    pixels: Vec<u32>,
}
impl Menu {
    pub fn new() -> Self {
        Self {
            open: false,
            music: 100,
            effects: 100,
            controls: Controls::default(),
            display: DisplaySettings::default(),
            status: String::new(),
            page: Page::Settings,
            selection: 0,
            player: 0,
            device: Device::Keyboard,
            capture: None,
            previous: [0; 4],
            suppressed: [0; 4],
            pixels: vec![0; WIDTH * HEIGHT],
        }
    }
    #[cfg(test)]
    pub fn handle(&mut self, held: [u8; 4], escape: bool) -> Frame {
        self.handle_input(held, escape, &[], &std::array::from_fn(|_| Vec::new()))
    }
    pub fn handle_input(
        &mut self,
        held: [u8; 4],
        escape: bool,
        keys: &[Key],
        pads: &[Vec<Button>; 4],
    ) -> Frame {
        let mut pressed = 0;
        for (i, &buttons) in held.iter().enumerate() {
            pressed |= buttons & !self.previous[i];
            self.suppressed[i] &= buttons;
        }
        self.previous = held;
        if let Some(mut capture) = self.capture {
            self.suppressed = held;
            // The cancellation action cannot be a valid new binding for this
            // action because it is already assigned elsewhere on this pad.
            let cancel_action = if capture.action == Action::Pause {
                Action::Fire
            } else {
                Action::Pause
            };
            let pad_cancel = capture.armed
                && self.device == Device::Gamepad
                && self.controls.gamepad[self.player]
                    .get(cancel_action)
                    .iter()
                    .any(|button| pads[self.player].contains(&button.native()));
            if escape || pad_cancel {
                self.capture = None;
                self.status = "CANCELLED".into();
            } else if !capture.armed {
                if keys.is_empty() && pads[self.player].is_empty() {
                    capture.armed = true;
                    self.capture = Some(capture);
                }
            } else {
                let result = match self.device {
                    Device::Keyboard => keys
                        .first()
                        .and_then(|&key| KeyboardKey::from_native(key))
                        .map(|key| self.controls.bind_key(self.player, capture.action, key)),
                    Device::Gamepad => pads[self.player]
                        .first()
                        .and_then(|&button| GamepadButton::from_native(button))
                        .map(|button| {
                            self.controls
                                .bind_button(self.player, capture.action, button)
                        }),
                };
                if let Some(result) = result {
                    match result {
                        Ok(()) => {
                            self.capture = None;
                            self.status = "BINDING SAVED".into();
                        }
                        Err(message) => {
                            self.status = message.into();
                            capture.armed = false;
                            self.capture = Some(capture);
                        }
                    }
                }
            }
            return Frame {
                paused: true,
                buttons: [0; 4],
                command: Command::None,
            };
        }
        let was_open = self.open;
        let mut command = Command::None;
        if escape || pressed & 8 != 0 {
            if self.open && self.page != Page::Settings {
                self.back();
            } else {
                self.open = !self.open;
                self.page = Page::Settings;
                self.selection = 0;
                self.status.clear();
            }
            self.suppressed = held;
        } else if self.open {
            if pressed & 2 != 0 {
                if self.page == Page::Settings {
                    self.open = false;
                } else {
                    self.back();
                }
            } else {
                let count = match self.page {
                    Page::Settings => 7,
                    Page::Controls => 11,
                    Page::Display => 6,
                    Page::Restart => 2,
                };
                if pressed & 16 != 0 {
                    self.selection = (self.selection + count - 1) % count;
                    self.status.clear();
                }
                if pressed & 32 != 0 {
                    self.selection = (self.selection + 1) % count;
                    self.status.clear();
                }
                let direction = if pressed & 64 != 0 {
                    -1
                } else if pressed & 128 != 0 {
                    1
                } else {
                    0
                };
                match self.page {
                    Page::Settings => {
                        if direction != 0 && (self.selection == 1 || self.selection == 2) {
                            let volume = if self.selection == 1 {
                                &mut self.music
                            } else {
                                &mut self.effects
                            };
                            *volume = (isize::from(*volume) + direction * 10).clamp(0, 100) as u8;
                        }
                        if pressed & 1 != 0 {
                            match self.selection {
                                0 => self.open = false,
                                3 => {
                                    self.page = Page::Controls;
                                    self.selection = 0;
                                }
                                4 => {
                                    self.page = Page::Display;
                                    self.selection = 0;
                                }
                                5 => {
                                    self.page = Page::Restart;
                                    self.selection = 0;
                                }
                                6 => command = Command::Quit,
                                _ => {}
                            }
                        }
                    }
                    Page::Controls => {
                        if self.selection == 0 && direction != 0 {
                            self.player = (self.player as isize + direction).rem_euclid(4) as usize;
                        }
                        if self.selection == 1 && (direction != 0 || pressed & 1 != 0) {
                            self.device = if self.device == Device::Keyboard {
                                Device::Gamepad
                            } else {
                                Device::Keyboard
                            };
                        } else if pressed & 1 != 0 {
                            match self.selection {
                                2..=8 => {
                                    self.capture = Some(Capture {
                                        action: Action::ALL[self.selection - 2],
                                        armed: false,
                                    });
                                    self.status.clear();
                                }
                                9 => {
                                    let result = match self.device {
                                        Device::Keyboard => {
                                            self.controls.reset_keyboard(self.player)
                                        }
                                        Device::Gamepad => {
                                            self.controls.reset_gamepad(self.player);
                                            Ok(())
                                        }
                                    };
                                    self.status = result.map_or_else(str::to_owned, |()| {
                                        "DEFAULTS RESTORED".into()
                                    });
                                }
                                10 => self.back(),
                                _ => {}
                            }
                        }
                    }
                    Page::Display => {
                        if direction != 0 {
                            match self.selection {
                                0 => self.display.cycle_size(direction),
                                1 => self.display.scaling.toggle(),
                                2 => self.display.fullscreen = !self.display.fullscreen,
                                _ => {}
                            }
                        }
                        if pressed & 1 != 0 {
                            match self.selection {
                                2 => self.display.fullscreen = !self.display.fullscreen,
                                3 => command = Command::ApplyDisplay(self.display),
                                4 => {
                                    self.display = DisplaySettings::default();
                                    self.status = "SELECT APPLY TO SAVE".into();
                                }
                                5 => self.back(),
                                _ => {}
                            }
                        }
                    }
                    Page::Restart => {
                        if pressed & 1 != 0 {
                            if self.selection == 1 {
                                command = Command::Restart;
                                self.open = false;
                            }
                            self.page = Page::Settings;
                            self.selection = 0;
                        }
                    }
                }
            }
            self.suppressed = held;
        }
        let paused = was_open || self.open;
        let mut buttons = [0; 4];
        if !paused {
            for i in 0..4 {
                buttons[i] = held[i] & !self.suppressed[i] & !8;
            }
        }
        Frame {
            paused,
            buttons,
            command,
        }
    }
    fn back(&mut self) {
        self.selection = match self.page {
            Page::Controls => 3,
            Page::Display => 4,
            _ => 0,
        };
        self.page = Page::Settings;
        self.status.clear();
    }
    pub fn window_resized(&mut self, previous: DisplaySettings, settings: DisplaySettings) {
        if self.page != Page::Display || self.display == previous {
            self.display = settings;
        }
    }
    pub fn render(&mut self, game: &[u32], audio_available: bool) -> &[u32] {
        self.pixels.copy_from_slice(game);
        for pixel in &mut self.pixels {
            *pixel = (*pixel & 0xfcfcfc) >> 2;
        }
        self.rect(12, 12, 232, 216, 0x556d99);
        self.rect(14, 14, 228, 212, 0x101725);
        self.text(80, 27, "SETTINGS", 0xf4dc82, 2);
        if let Some(capture) = self.capture {
            self.text(
                32,
                72,
                &format!(
                    "PLAYER {} - {}",
                    self.player + 1,
                    if self.device == Device::Keyboard {
                        "KEYBOARD"
                    } else {
                        "GAMEPAD"
                    }
                ),
                0xe1e7f2,
                1,
            );
            self.text(
                32,
                98,
                &format!("BIND {}", capture.action.label()),
                0xf4dc82,
                2,
            );
            self.text(
                32,
                128,
                if !capture.armed {
                    "RELEASE HELD KEYS/BUTTONS"
                } else if self.device == Device::Keyboard {
                    "PRESS A KEY"
                } else {
                    "PRESS A GAMEPAD BUTTON"
                },
                0xe1e7f2,
                1,
            );
            self.text(32, 166, &self.status.clone(), 0xf4dc82, 1);
            let cancel = if self.device == Device::Gamepad {
                let action = if capture.action == Action::Pause {
                    Action::Fire
                } else {
                    Action::Pause
                };
                let buttons: Vec<_> = self.controls.gamepad[self.player]
                    .get(action)
                    .iter()
                    .map(|button| button.label())
                    .collect();
                format!("ESC OR {}: CANCEL", buttons.join("/"))
            } else {
                "ESC: CANCEL".into()
            };
            self.text(32, 207, &cancel, 0x9baecb, 1);
            return &self.pixels;
        }
        match self.page {
            Page::Settings => {
                let music = format!("MUSIC       < {:3}% >", self.music);
                let effects = format!("EFFECTS     < {:3}% >", self.effects);
                for (i, label) in [
                    "RESUME",
                    &music,
                    &effects,
                    "CONTROLS",
                    "DISPLAY",
                    "RESTART GAME",
                    "QUIT GAME",
                ]
                .into_iter()
                .enumerate()
                {
                    self.row(i, 59 + i * 18, label);
                }
                self.text(26, 194, "UP/DOWN SELECT  LEFT/RIGHT EDIT", 0x9baecb, 1);
                self.text(26, 207, "ENTER: OK  ESC/START: BACK", 0x9baecb, 1);
                if !audio_available {
                    self.text(26, 47, "AUDIO UNAVAILABLE / MUTED", 0xcb9d83, 1);
                }
            }
            Page::Controls => {
                let mut rows = vec![
                    format!("PLAYER       < {} >", self.player + 1),
                    format!(
                        "DEVICE   < {} >",
                        if self.device == Device::Keyboard {
                            "KEYBOARD"
                        } else {
                            "GAMEPAD"
                        }
                    ),
                ];
                for action in Action::ALL {
                    let labels: Vec<_> = match self.device {
                        Device::Keyboard => self.controls.keyboard[self.player]
                            .get(action)
                            .iter()
                            .map(|key| key.label())
                            .collect(),
                        Device::Gamepad => self.controls.gamepad[self.player]
                            .get(action)
                            .iter()
                            .map(|button| button.label())
                            .collect(),
                    };
                    rows.push(format!("{:<6} {}", action.label(), labels.join("/")));
                }
                rows.extend(["RESET PLAYER".into(), "BACK".into()]);
                for (i, label) in rows.into_iter().enumerate() {
                    let y = 54 + i * 13;
                    if i == self.selection {
                        self.rect(22, y - 2, 212, 12, 0x293d60);
                    }
                    self.text(
                        27,
                        y,
                        if i == self.selection { ">" } else { " " },
                        0xf4dc82,
                        1,
                    );
                    self.text(
                        39,
                        y,
                        &label,
                        if i == self.selection {
                            0xf4dc82
                        } else {
                            0xe1e7f2
                        },
                        1,
                    );
                }
                self.text(26, 196, &self.status.clone(), 0xf4dc82, 1);
                self.text(26, 209, "ENTER: REBIND  ESC: BACK", 0x9baecb, 1);
            }
            Page::Display => {
                let size = format!("WINDOW < {}X{} >", self.display.width, self.display.height);
                let scaling = format!("SCALE < {} >", self.display.scaling.label());
                let fullscreen = format!(
                    "FULLSCREEN < {} >",
                    if self.display.fullscreen { "ON" } else { "OFF" }
                );
                for (i, label) in [
                    &size,
                    &scaling,
                    &fullscreen,
                    "APPLY",
                    "RESTORE DEFAULTS",
                    "BACK",
                ]
                .into_iter()
                .enumerate()
                {
                    self.row(i, 61 + i * 20, label);
                }
                self.text(26, 181, "FULL HEIGHT - ORIGINAL ASPECT", 0x9baecb, 1);
                self.text(26, 196, &self.status.clone(), 0xf4dc82, 1);
                self.text(26, 209, "LEFT/RIGHT EDIT  ENTER: OK", 0x9baecb, 1);
            }
            Page::Restart => {
                self.text(44, 75, "RESTART FROM THE BEGINNING?", 0xe1e7f2, 1);
                self.text(44, 94, "CURRENT PROGRESS IS LOST.", 0x9baecb, 1);
                self.row(0, 128, "CANCEL");
                self.row(1, 152, "RESTART GAME");
                self.text(26, 207, "ENTER: OK  ESC/START: BACK", 0x9baecb, 1);
            }
        }
        &self.pixels
    }
    fn row(&mut self, index: usize, y: usize, label: &str) {
        if index == self.selection {
            self.rect(22, y - 4, 212, 17, 0x293d60);
            self.text(28, y, ">", 0xf4dc82, 1);
        }
        self.text(
            42,
            y,
            label,
            if index == self.selection {
                0xf4dc82
            } else {
                0xe1e7f2
            },
            1,
        );
    }
    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        for row in y..(y + h).min(HEIGHT) {
            for col in x..(x + w).min(WIDTH) {
                self.pixels[row * WIDTH + col] = color;
            }
        }
    }
    fn text(&mut self, x: usize, y: usize, text: &str, color: u32, scale: usize) {
        for (index, ch) in text.chars().enumerate() {
            for (row, bits) in glyph(ch).into_iter().enumerate() {
                for col in 0..5 {
                    if bits & (1 << (4 - col)) != 0 {
                        self.rect(
                            x + (index * 6 + col) * scale,
                            y + row * scale,
                            scale,
                            scale,
                            color,
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn press(menu: &mut Menu, player: usize, value: u8) -> Frame {
        menu.handle([0; 4], false);
        let mut masks = [0; 4];
        masks[player] = value;
        menu.handle(masks, false)
    }
    #[test]
    fn escape_and_every_players_start_open_without_quitting_or_leaking() {
        for player in 0..4 {
            let mut menu = Menu::new();
            let opened = press(&mut menu, player, 8);
            assert!(menu.open && opened.paused);
            assert_eq!(opened.command, Command::None);
            assert_eq!(opened.buttons, [0; 4]);
            let closed = press(&mut menu, player, 8);
            assert!(!menu.open && closed.paused);
        }
        let mut menu = Menu::new();
        assert!(menu.handle([0; 4], true).paused);
        assert_eq!(menu.handle([0; 4], true).command, Command::None);
        assert!(!menu.open);
    }
    #[test]
    fn resume_requires_release_before_game_receives_confirmation() {
        let mut menu = Menu::new();
        menu.handle([0; 4], true);
        assert!(press(&mut menu, 0, 1).paused);
        assert!(!menu.open);
        assert_eq!(menu.handle([1, 0, 0, 0], false).buttons, [0; 4]);
        assert_eq!(press(&mut menu, 0, 1).buttons, [1, 0, 0, 0]);
    }
    #[test]
    fn volume_bounds_restart_cancel_and_explicit_quit() {
        let mut menu = Menu::new();
        menu.handle([0; 4], true);
        press(&mut menu, 0, 32);
        for _ in 0..15 {
            press(&mut menu, 0, 64);
        }
        assert_eq!(menu.music, 0);
        for _ in 0..15 {
            press(&mut menu, 0, 128);
        }
        assert_eq!(menu.music, 100);
        for _ in 0..4 {
            press(&mut menu, 0, 32);
        }
        assert_eq!(press(&mut menu, 0, 1).command, Command::None);
        assert_eq!(menu.page, Page::Restart);
        assert_eq!(press(&mut menu, 0, 1).command, Command::None);
        assert_eq!(menu.page, Page::Settings);
        press(&mut menu, 0, 16);
        assert_eq!(press(&mut menu, 0, 1).command, Command::Quit);
    }

    #[test]
    fn rebinding_waits_for_release_and_consumes_keyboard_and_gamepad_input() {
        let mut menu = Menu::new();
        menu.open = true;
        menu.page = Page::Controls;
        menu.selection = 6; // Jump.
        let empty_pads = std::array::from_fn(|_| Vec::new());
        press(&mut menu, 0, 1);
        menu.handle_input([1, 0, 0, 0], false, &[Key::Return], &empty_pads);
        assert_eq!(menu.controls, Controls::default());
        assert!(!menu.capture.unwrap().armed);
        menu.handle_input([0; 4], false, &[], &empty_pads);
        let frame = menu.handle_input([0; 4], false, &[Key::Space], &empty_pads);
        assert!(frame.paused && menu.capture.is_none());
        assert_eq!(frame.buttons, [0; 4]);
        assert_eq!(menu.controls.keyboard_masks(&[Key::Space]), [1, 0, 0, 0]);

        menu.player = 2;
        menu.device = Device::Gamepad;
        menu.selection = 7; // Shoot.
        press(&mut menu, 0, 1);
        menu.handle_input([0; 4], false, &[], &empty_pads);
        let mut pads = empty_pads.clone();
        pads[2] = vec![Button::RightTrigger];
        let frame = menu.handle_input([0; 4], false, &[], &pads);
        assert!(frame.paused && menu.capture.is_none());
        assert_eq!(frame.buttons, [0; 4]);
        assert_eq!(
            menu.controls.gamepad_masks(&pads, [[0.; 2]; 4]),
            [0, 0, 2, 0]
        );

        let previous = menu.controls.clone();
        menu.selection = 6;
        press(&mut menu, 0, 1);
        menu.handle_input([0; 4], false, &[], &empty_pads);
        menu.handle_input([0; 4], false, &[], &pads);
        assert!(menu.capture.is_some());
        assert_eq!(menu.status, "BUTTON ALREADY IN USE");
        menu.handle_input([0; 4], true, &[Key::Escape], &empty_pads);
        assert!(menu.capture.is_none() && menu.open);
        assert_eq!(menu.controls, previous);

        press(&mut menu, 0, 1);
        menu.handle_input([0; 4], false, &[], &empty_pads);
        pads[2] = vec![Button::Start];
        menu.handle_input([0; 4], false, &[], &pads);
        assert!(menu.capture.is_none() && menu.open);
        assert_eq!(menu.controls, previous);
    }

    #[test]
    fn display_edits_require_apply_and_do_not_resume_the_game() {
        let mut menu = Menu::new();
        menu.open = true;
        menu.page = Page::Display;
        let original = menu.display;
        assert_eq!(press(&mut menu, 0, 128).command, Command::None);
        assert_ne!(menu.display, original);
        press(&mut menu, 0, 32);
        press(&mut menu, 0, 128);
        assert_eq!(
            menu.display.scaling,
            micro_mages_re::display::Scaling::Smooth
        );
        press(&mut menu, 0, 32);
        press(&mut menu, 0, 128);
        assert!(menu.display.fullscreen);
        press(&mut menu, 0, 32);
        let frame = press(&mut menu, 0, 1);
        assert_eq!(frame.command, Command::ApplyDisplay(menu.display));
        assert!(frame.paused && menu.open);
        menu.handle([0; 4], true);
        assert_eq!(menu.page, Page::Settings);
        assert_eq!(menu.selection, 4);
    }

    #[test]
    fn manual_resize_refreshes_the_display_page_without_overwriting_pending_edits() {
        let mut menu = Menu::new();
        menu.page = Page::Display;
        let previous = menu.display;
        let resized = DisplaySettings {
            width: 1000,
            height: 600,
            ..previous
        };
        menu.window_resized(previous, resized);
        assert_eq!(menu.display, resized);
        menu.display.cycle_size(1);
        let pending = menu.display;
        menu.window_resized(resized, previous);
        assert_eq!(menu.display, pending);
    }
}
