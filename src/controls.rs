//! Portable, named keyboard and gamepad bindings for each player.
use gilrs::Button;
use sdl2::keyboard::Scancode as Key;
use serde::{Deserialize, Serialize};
use std::{io, path::Path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Jump,
    Fire,
    Pause,
}
impl Action {
    pub const ALL: [Self; 7] = [
        Self::Up,
        Self::Down,
        Self::Left,
        Self::Right,
        Self::Jump,
        Self::Fire,
        Self::Pause,
    ];
    pub fn bit(self) -> u8 {
        match self {
            Self::Jump => 1,
            Self::Fire => 2,
            Self::Pause => 8,
            Self::Up => 16,
            Self::Down => 32,
            Self::Left => 64,
            Self::Right => 128,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Jump => "JUMP",
            Self::Fire => "SHOOT",
            Self::Pause => "PAUSE",
            Self::Up => "UP",
            Self::Down => "DOWN",
            Self::Left => "LEFT",
            Self::Right => "RIGHT",
        }
    }
}

macro_rules! named_buttons {
    ($name:ident, $native:ident, [$($button:ident => $native_button:ident),+ $(,)?]) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($button),+ }
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$button),+];
            pub fn native(self) -> $native { match self { $(Self::$button => $native::$native_button),+ } }
            pub fn from_native(value: $native) -> Option<Self> {
                match value { $($native::$native_button => Some(Self::$button),)+ _ => None }
            }
            pub fn label(self) -> String { format!("{self:?}").to_uppercase() }
        }
    }
}

// Escape is always available to leave settings, including after a bad rebind.
named_buttons!(KeyboardKey, Key, [
    Key0 => Num0,
    Key1 => Num1,
    Key2 => Num2,
    Key3 => Num3,
    Key4 => Num4,
    Key5 => Num5,
    Key6 => Num6,
    Key7 => Num7,
    Key8 => Num8,
    Key9 => Num9,
    A => A,
    B => B,
    C => C,
    D => D,
    E => E,
    F => F,
    G => G,
    H => H,
    I => I,
    J => J,
    K => K,
    L => L,
    M => M,
    N => N,
    O => O,
    P => P,
    Q => Q,
    R => R,
    S => S,
    T => T,
    U => U,
    V => V,
    W => W,
    X => X,
    Y => Y,
    Z => Z,
    F1 => F1,
    F2 => F2,
    F3 => F3,
    F4 => F4,
    F5 => F5,
    F6 => F6,
    F7 => F7,
    F8 => F8,
    F9 => F9,
    F10 => F10,
    F11 => F11,
    F12 => F12,
    F13 => F13,
    F14 => F14,
    F15 => F15,
    Down => Down,
    Left => Left,
    Right => Right,
    Up => Up,
    Apostrophe => Apostrophe,
    Backquote => Grave,
    Backslash => Backslash,
    Comma => Comma,
    Equal => Equals,
    LeftBracket => LeftBracket,
    Minus => Minus,
    Period => Period,
    RightBracket => RightBracket,
    Semicolon => Semicolon,
    Slash => Slash,
    Backspace => Backspace,
    Delete => Delete,
    End => End,
    Enter => Return,
    Home => Home,
    Insert => Insert,
    Menu => Menu,
    PageDown => PageDown,
    PageUp => PageUp,
    Pause => Pause,
    Space => Space,
    Tab => Tab,
    NumLock => NumLockClear,
    CapsLock => CapsLock,
    ScrollLock => ScrollLock,
    LeftShift => LShift,
    RightShift => RShift,
    LeftCtrl => LCtrl,
    RightCtrl => RCtrl,
    NumPad0 => Kp0,
    NumPad1 => Kp1,
    NumPad2 => Kp2,
    NumPad3 => Kp3,
    NumPad4 => Kp4,
    NumPad5 => Kp5,
    NumPad6 => Kp6,
    NumPad7 => Kp7,
    NumPad8 => Kp8,
    NumPad9 => Kp9,
    NumPadDot => KpPeriod,
    NumPadSlash => KpDivide,
    NumPadAsterisk => KpMultiply,
    NumPadMinus => KpMinus,
    NumPadPlus => KpPlus,
    NumPadEnter => KpEnter,
    LeftAlt => LAlt,
    RightAlt => RAlt,
    LeftSuper => LGui,
    RightSuper => RGui,
]);
named_buttons!(GamepadButton, Button, [
    South => South,
    East => East,
    North => North,
    West => West,
    C => C,
    Z => Z,
    LeftTrigger => LeftTrigger,
    LeftTrigger2 => LeftTrigger2,
    RightTrigger => RightTrigger,
    RightTrigger2 => RightTrigger2,
    Select => Select,
    Start => Start,
    Mode => Mode,
    LeftThumb => LeftThumb,
    RightThumb => RightThumb,
    DPadUp => DPadUp,
    DPadDown => DPadDown,
    DPadLeft => DPadLeft,
    DPadRight => DPadRight,
]);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bindings<T> {
    pub up: Vec<T>,
    pub down: Vec<T>,
    pub left: Vec<T>,
    pub right: Vec<T>,
    pub jump: Vec<T>,
    pub fire: Vec<T>,
    pub pause: Vec<T>,
}
impl<T> Bindings<T> {
    pub fn get(&self, action: Action) -> &[T] {
        match action {
            Action::Up => &self.up,
            Action::Down => &self.down,
            Action::Left => &self.left,
            Action::Right => &self.right,
            Action::Jump => &self.jump,
            Action::Fire => &self.fire,
            Action::Pause => &self.pause,
        }
    }
    fn get_mut(&mut self, action: Action) -> &mut Vec<T> {
        match action {
            Action::Up => &mut self.up,
            Action::Down => &mut self.down,
            Action::Left => &mut self.left,
            Action::Right => &mut self.right,
            Action::Jump => &mut self.jump,
            Action::Fire => &mut self.fire,
            Action::Pause => &mut self.pause,
        }
    }
}
impl<T: Copy + Eq> Bindings<T> {
    fn validate(&self) -> io::Result<()> {
        let mut seen = Vec::new();
        for action in Action::ALL {
            let buttons = self.get(action);
            if buttons.is_empty() || buttons.len() > 4 {
                return Err(io::Error::other("each action needs one to four bindings"));
            }
            for &button in buttons {
                if seen.contains(&button) {
                    return Err(io::Error::other("a button is assigned more than once"));
                }
                seen.push(button);
            }
        }
        Ok(())
    }
    fn mask(&self, held: impl Fn(T) -> bool) -> u8 {
        Action::ALL.into_iter().fold(0, |mask, action| {
            mask | if self.get(action).iter().copied().any(&held) {
                action.bit()
            } else {
                0
            }
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Controls {
    pub version: u32,
    pub keyboard: [Bindings<KeyboardKey>; 4],
    pub gamepad: [Bindings<GamepadButton>; 4],
}
impl Default for Controls {
    fn default() -> Self {
        use KeyboardKey::*;
        Self {
            version: 1,
            keyboard: [
                Bindings {
                    up: vec![W],
                    down: vec![S],
                    left: vec![A],
                    right: vec![D],
                    jump: vec![G],
                    fire: vec![H],
                    pause: vec![Y],
                },
                Bindings {
                    up: vec![Up],
                    down: vec![Down],
                    left: vec![Left],
                    right: vec![Right],
                    jump: vec![Z, Enter],
                    fire: vec![X],
                    pause: vec![P],
                },
                Bindings {
                    up: vec![I],
                    down: vec![K],
                    left: vec![J],
                    right: vec![L],
                    jump: vec![N],
                    fire: vec![M],
                    pause: vec![O],
                },
                Bindings {
                    up: vec![NumPad8],
                    down: vec![NumPad2],
                    left: vec![NumPad4],
                    right: vec![NumPad6],
                    jump: vec![NumPad0],
                    fire: vec![NumPadDot],
                    pause: vec![NumPadEnter],
                },
            ],
            gamepad: std::array::from_fn(|_| {
                use GamepadButton::*;
                Bindings {
                    up: vec![DPadUp],
                    down: vec![DPadDown],
                    left: vec![DPadLeft],
                    right: vec![DPadRight],
                    jump: vec![South],
                    fire: vec![West, East],
                    pause: vec![Start],
                }
            }),
        }
    }
}
impl Controls {
    pub fn load_or_create(path: &Path) -> Result<Self, String> {
        crate::storage::load_settings(path, Self::validate)
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        self.validate()?;
        crate::storage::atomic_write(path, &serde_json::to_vec_pretty(self)?)
    }
    pub fn validate(&self) -> io::Result<()> {
        if self.version != 1 {
            return Err(io::Error::other("unsupported controls version"));
        }
        let mut keys = Vec::new();
        for bindings in &self.keyboard {
            bindings.validate()?;
            for action in Action::ALL {
                for key in bindings.get(action) {
                    if keys.contains(key) {
                        return Err(io::Error::other(
                            "keyboard binding is shared by multiple players",
                        ));
                    }
                    keys.push(*key);
                }
            }
        }
        for bindings in &self.gamepad {
            bindings.validate()?;
        }
        Ok(())
    }
    pub fn keyboard_masks(&self, held: &[Key]) -> [u8; 4] {
        std::array::from_fn(|id| self.keyboard[id].mask(|key| held.contains(&key.native())))
    }
    pub fn gamepad_masks(&self, held: &[Vec<Button>; 4], sticks: [[f32; 2]; 4]) -> [u8; 4] {
        std::array::from_fn(|id| {
            self.gamepad[id].mask(|button| held[id].contains(&button.native()))
                | stick_mask(sticks[id])
        })
    }
    pub fn bind_key(
        &mut self,
        player: usize,
        action: Action,
        key: KeyboardKey,
    ) -> Result<(), &'static str> {
        if self.keyboard.iter().enumerate().any(|(id, bindings)| {
            Action::ALL.into_iter().any(|other| {
                (id != player || other != action) && bindings.get(other).contains(&key)
            })
        }) {
            return Err("KEY ALREADY IN USE");
        }
        *self.keyboard[player].get_mut(action) = vec![key];
        Ok(())
    }
    pub fn bind_button(
        &mut self,
        player: usize,
        action: Action,
        button: GamepadButton,
    ) -> Result<(), &'static str> {
        if Action::ALL
            .into_iter()
            .any(|other| other != action && self.gamepad[player].get(other).contains(&button))
        {
            return Err("BUTTON ALREADY IN USE");
        }
        *self.gamepad[player].get_mut(action) = vec![button];
        Ok(())
    }
    pub fn reset_keyboard(&mut self, player: usize) -> Result<(), &'static str> {
        let mut next = self.clone();
        next.keyboard[player] = Self::default().keyboard[player].clone();
        if next.validate().is_err() {
            return Err("DEFAULT KEY IN USE");
        }
        *self = next;
        Ok(())
    }
    pub fn reset_gamepad(&mut self, player: usize) {
        self.gamepad[player] = Self::default().gamepad[player].clone();
    }
}

pub fn stick_mask([x, y]: [f32; 2]) -> u8 {
    (u8::from(y > 0.35) * 16)
        | (u8::from(y < -0.35) * 32)
        | (u8::from(x < -0.35) * 64)
        | (u8::from(x > 0.35) * 128)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_keyboard_layouts_assign_wasd_to_player_one() {
        let controls = Controls::default();
        controls.validate().unwrap();
        assert_eq!(
            controls.keyboard_masks(&[Key::Return, Key::H, Key::I, Key::KpEnter]),
            [2, 1, 16, 8]
        );
        assert_eq!(
            controls.keyboard_masks(&[Key::W, Key::H, Key::Y]),
            [16 | 2 | 8, 0, 0, 0]
        );
        assert_eq!(
            controls.keyboard_masks(&[Key::Up, Key::X, Key::P]),
            [0, 16 | 2 | 8, 0, 0]
        );
        let pads = std::array::from_fn(|_| vec![Button::South, Button::East]);
        assert_eq!(controls.gamepad_masks(&pads, [[0., 0.]; 4]), [3; 4]);
        assert_eq!(stick_mask([-1., 1.]), 16 | 64);
        assert_eq!(stick_mask([0.2, -0.2]), 0);
    }
    #[test]
    fn rebinds_replace_old_actions_and_reject_conflicts_without_changes() {
        let mut controls = Controls::default();
        controls
            .bind_key(0, Action::Jump, KeyboardKey::Space)
            .unwrap();
        assert_eq!(controls.keyboard_masks(&[Key::G]), [0; 4]);
        assert_eq!(
            controls.keyboard_masks(&[Key::Z, Key::Return]),
            [0, 1, 0, 0]
        );
        assert_eq!(controls.keyboard_masks(&[Key::Space]), [1, 0, 0, 0]);
        let previous = controls.clone();
        assert!(controls.bind_key(0, Action::Jump, KeyboardKey::Z).is_err());
        assert_eq!(controls, previous);
        controls
            .bind_button(2, Action::Fire, GamepadButton::RightTrigger)
            .unwrap();
        let mut held = std::array::from_fn(|_| Vec::new());
        held[2] = vec![Button::RightTrigger];
        assert_eq!(controls.gamepad_masks(&held, [[0., 0.]; 4]), [0, 0, 2, 0]);
        assert!(
            controls
                .bind_button(2, Action::Fire, GamepadButton::South)
                .is_err()
        );
    }
    #[test]
    fn portable_names_round_trip_and_invalid_or_reserved_bindings_are_rejected() {
        let mut controls = Controls::default();
        controls
            .bind_key(3, Action::Pause, KeyboardKey::F8)
            .unwrap();
        let bytes = serde_json::to_vec(&controls).unwrap();
        assert_eq!(
            serde_json::from_slice::<Controls>(&bytes).unwrap(),
            controls
        );
        assert!(serde_json::from_str::<KeyboardKey>("\"escape\"").is_err());
        assert!(serde_json::from_str::<KeyboardKey>("\"unknown\"").is_err());
        controls.keyboard[1].jump = controls.keyboard[0].jump.clone();
        assert!(controls.validate().is_err());
    }
}
