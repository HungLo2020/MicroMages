//! Native pause/settings UI. Start is consumed here, never sent into game logic.
use micro_mages_re::video::{HEIGHT, WIDTH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    None,
    Restart,
    Quit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Settings,
    Controls,
    Restart,
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
    page: Page,
    selection: usize,
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
            page: Page::Settings,
            selection: 0,
            previous: [0; 4],
            suppressed: [0; 4],
            pixels: vec![0; WIDTH * HEIGHT],
        }
    }

    pub fn handle(&mut self, held: [u8; 4], escape: bool) -> Frame {
        let mut pressed = 0;
        for (i, &buttons) in held.iter().enumerate() {
            // Detect edges separately: one player holding a key must not mask another.
            pressed |= buttons & !self.previous[i];
            self.suppressed[i] &= buttons;
        }
        self.previous = held;
        let was_open = self.open;
        let mut command = Command::None;
        if escape || pressed & 8 != 0 {
            if self.open && self.page != Page::Settings {
                self.page = Page::Settings;
                self.selection = 0;
            } else {
                self.open = !self.open;
                self.page = Page::Settings;
                self.selection = 0;
            }
            self.suppressed = held;
        } else if self.open {
            if pressed & 2 != 0 {
                if self.page == Page::Settings {
                    self.open = false;
                } else {
                    self.page = Page::Settings;
                    self.selection = 0;
                }
            } else if self.page == Page::Controls {
                if pressed & 1 != 0 {
                    self.page = Page::Settings;
                    self.selection = 3;
                }
            } else {
                let count = if self.page == Page::Restart { 2 } else { 6 };
                if pressed & 16 != 0 {
                    self.selection = (self.selection + count - 1) % count;
                }
                if pressed & 32 != 0 {
                    self.selection = (self.selection + 1) % count;
                }
                if self.page == Page::Settings && (self.selection == 1 || self.selection == 2) {
                    let value = if self.selection == 1 {
                        &mut self.music
                    } else {
                        &mut self.effects
                    };
                    if pressed & 64 != 0 {
                        *value = value.saturating_sub(10);
                    }
                    if pressed & 128 != 0 {
                        *value = value.saturating_add(10).min(100);
                    }
                }
                if pressed & 1 != 0 {
                    if self.page == Page::Restart {
                        if self.selection == 1 {
                            command = Command::Restart;
                            self.open = false;
                        }
                        self.page = Page::Settings;
                        self.selection = 0;
                    } else {
                        match self.selection {
                            0 => self.open = false,
                            3 => self.page = Page::Controls,
                            4 => {
                                self.page = Page::Restart;
                                self.selection = 0;
                            }
                            5 => command = Command::Quit,
                            _ => {}
                        }
                    }
                }
            }
            // Held menu controls must be released before they can affect the game.
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

    pub fn render(&mut self, game: &[u32], audio_available: bool) -> &[u32] {
        self.pixels.copy_from_slice(game);
        for pixel in &mut self.pixels {
            *pixel = (*pixel & 0xfcfcfc) >> 2;
        }
        self.rect(12, 12, 232, 216, 0x556d99);
        self.rect(14, 14, 228, 212, 0x101725);
        self.text(80, 27, "SETTINGS", 0xf4dc82, 2);
        match self.page {
            Page::Settings => {
                let music = format!("MUSIC       < {:3}% >", self.music);
                let effects = format!("EFFECTS     < {:3}% >", self.effects);
                let rows = [
                    "RESUME",
                    music.as_str(),
                    effects.as_str(),
                    "CONTROLS",
                    "RESTART GAME",
                    "QUIT GAME",
                ];
                for (i, label) in rows.into_iter().enumerate() {
                    self.row(i, 67 + i * 20, label);
                }
                self.text(26, 194, "UP/DOWN SELECT  LEFT/RIGHT EDIT", 0x9baecb, 1);
                self.text(26, 207, "ENTER/Z: OK  ESC/START: BACK", 0x9baecb, 1);
                if !audio_available {
                    self.text(26, 49, "AUDIO UNAVAILABLE / MUTED", 0xcb9d83, 1);
                }
            }
            Page::Controls => {
                for (i, line) in [
                    "PLAYER 1  ARROWS + Z/X",
                    "PLAYER 2  WASD + G/H",
                    "PLAYER 3  IJKL + N/M",
                    "ENTER: CONFIRM / A ACTION",
                    "ESC/P: SETTINGS",
                    "GAMEPAD: D-PAD OR LEFT STICK",
                    "SOUTH: A   WEST/EAST: B",
                    "START: SETTINGS (ANY PLAYER)",
                    "UP TO FOUR GAMEPADS",
                ]
                .iter()
                .enumerate()
                {
                    self.text(26, 62 + i * 14, line, 0xe1e7f2, 1);
                }
                self.text(26, 207, "ENTER/Z OR ESC/START: BACK", 0xf4dc82, 1);
            }
            Page::Restart => {
                self.text(44, 75, "RESTART FROM THE BEGINNING?", 0xe1e7f2, 1);
                self.text(44, 94, "CURRENT PROGRESS IS LOST.", 0x9baecb, 1);
                self.row(0, 128, "CANCEL");
                self.row(1, 152, "RESTART GAME");
                self.text(26, 207, "ENTER/Z: OK  ESC/START: BACK", 0x9baecb, 1);
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

fn glyph(ch: char) -> [u8; 7] {
    match ch {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 14],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 25, 21, 19, 19, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '>' => [16, 8, 4, 2, 4, 8, 16],
        '<' => [1, 2, 4, 8, 4, 2, 1],
        ':' => [0, 4, 4, 0, 4, 4, 0],
        '%' => [25, 25, 2, 4, 8, 19, 19],
        '/' => [1, 1, 2, 4, 8, 16, 16],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '+' => [0, 4, 4, 31, 4, 4, 0],
        '?' => [14, 17, 1, 2, 4, 0, 4],
        '(' => [2, 4, 8, 8, 8, 4, 2],
        ')' => [8, 4, 2, 2, 2, 4, 8],
        '.' => [0, 0, 0, 0, 0, 4, 4],
        _ => [0; 7],
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
        for _ in 0..3 {
            press(&mut menu, 0, 32);
        }
        assert_eq!(press(&mut menu, 0, 1).command, Command::None);
        assert_eq!(menu.page, Page::Restart);
        assert_eq!(press(&mut menu, 0, 1).command, Command::None);
        assert_eq!(menu.page, Page::Settings);
        press(&mut menu, 0, 16);
        assert_eq!(press(&mut menu, 0, 1).command, Command::Quit);
    }
}
