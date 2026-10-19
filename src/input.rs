//! Native four-player gamepad input. Keyboard input can be ORed into each mask.
use gilrs::{Axis, Button, GamepadId, Gilrs};

pub struct Pads {
    backend: Option<Gilrs>,
    slots: [Option<GamepadId>; 4],
}

impl Default for Pads {
    fn default() -> Self {
        Self::new()
    }
}

impl Pads {
    pub fn new() -> Self {
        let backend = match Gilrs::new() {
            Ok(backend) => Some(backend),
            Err(error) => {
                eprintln!(
                    "Gamepad input unavailable ({error}); keyboard controls remain available."
                );
                None
            }
        };
        Self {
            backend,
            slots: [None; 4],
        }
    }

    /// Poll cached native input once per frame. Disconnected players always
    /// return zero; existing players never shift slots when another unplugs.
    pub fn poll(&mut self) -> [u8; 4] {
        let Some(backend) = self.backend.as_mut() else {
            return [0; 4];
        };
        // next_event applies the default mapping filters and updates cached state.
        while backend.next_event().is_some() {}
        let connected: Vec<_> = backend.gamepads().map(|(id, _)| id).collect();
        reconcile_slots(&mut self.slots, &connected);
        let mut masks = [0; 4];
        for (slot, mask) in self.slots.iter().zip(masks.iter_mut()) {
            let Some(id) = slot else {
                continue;
            };
            if !connected.contains(id) {
                continue;
            }
            let pad = backend.gamepad(*id);
            *mask = button_mask(
                |button| pad.is_pressed(button),
                pad.value(Axis::LeftStickX),
                pad.value(Axis::LeftStickY),
            );
        }
        masks
    }
}

fn reconcile_slots<T: Copy + PartialEq>(slots: &mut [Option<T>; 4], connected: &[T]) {
    for &id in connected {
        if slots.contains(&Some(id)) {
            continue;
        }
        // Keep disconnected IDs reserved until a new device needs their slot.
        // A reconnect therefore recovers the old player number when possible.
        let vacant = slots.iter().position(Option::is_none).or_else(|| {
            slots
                .iter()
                .position(|slot| slot.is_some_and(|old| !connected.contains(&old)))
        });
        if let Some(index) = vacant {
            slots[index] = Some(id);
        }
    }
}

fn button_mask(pressed: impl Fn(Button) -> bool, x: f32, y: f32) -> u8 {
    let mut mask = 0;
    let bindings = [
        (Button::South, 1),
        (Button::West, 2),
        (Button::East, 2),
        (Button::Select, 4),
        (Button::Start, 8),
        (Button::DPadUp, 16),
        (Button::DPadDown, 32),
        (Button::DPadLeft, 64),
        (Button::DPadRight, 128),
    ];
    for (button, bit) in bindings {
        if pressed(button) {
            mask |= bit;
        }
    }
    // Gilrs uses positive Y for up. A substantial dead zone prevents drift.
    if y > 0.35 {
        mask |= 16;
    }
    if y < -0.35 {
        mask |= 32;
    }
    if x < -0.35 {
        mask |= 64;
    }
    if x > 0.35 {
        mask |= 128;
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnect_does_not_renumber_players_and_reconnect_recovers_slot() {
        let mut slots = [None; 4];
        reconcile_slots(&mut slots, &[10, 20]);
        assert_eq!(slots, [Some(10), Some(20), None, None]);
        reconcile_slots(&mut slots, &[20]);
        assert_eq!(slots[1], Some(20));
        reconcile_slots(&mut slots, &[20, 10]);
        assert_eq!(slots[0], Some(10));
        reconcile_slots(&mut slots, &[20, 30, 40, 50]);
        assert_eq!(slots, [Some(50), Some(20), Some(30), Some(40)]);
    }

    #[test]
    fn controls_combine_and_stick_dead_zone_is_neutral() {
        assert_eq!(button_mask(|_| false, 0.2, -0.2), 0);
        assert_eq!(button_mask(|_| false, -1.0, 1.0), 16 | 64);
        assert_eq!(button_mask(|_| false, 1.0, -1.0), 32 | 128);
        assert_eq!(
            button_mask(
                |b| matches!(b, Button::South | Button::East | Button::Start),
                0.0,
                0.0
            ),
            11
        );
    }

    #[test]
    fn missing_backend_has_no_pressed_buttons() {
        let mut pads = Pads {
            backend: None,
            slots: [None; 4],
        };
        assert_eq!(pads.poll(), [0; 4]);
    }
}

/// Gameplay actions, independent of keyboard bindings and controller protocols.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerInput {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub jump: bool,
    pub fire: bool,
}
impl PlayerInput {
    pub fn from_buttons(buttons: u8) -> Self {
        Self {
            jump: buttons & 1 != 0,
            fire: buttons & 2 != 0,
            up: buttons & 16 != 0,
            down: buttons & 32 != 0,
            left: buttons & 64 != 0,
            right: buttons & 128 != 0,
        }
    }
    fn without(self, other: Self) -> Self {
        Self {
            left: self.left && !other.left,
            right: self.right && !other.right,
            up: self.up && !other.up,
            down: self.down && !other.down,
            jump: self.jump && !other.jump,
            fire: self.fire && !other.fire,
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct InputFrame {
    pub held: PlayerInput,
    pub pressed: PlayerInput,
    pub released: PlayerInput,
}
impl InputFrame {
    pub fn between(previous: PlayerInput, held: PlayerInput) -> Self {
        Self {
            held,
            pressed: held.without(previous),
            released: previous.without(held),
        }
    }
}
