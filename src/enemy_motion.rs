//! Source-derived fixed-point movement primitives expressed in world coordinates.
use crate::physics::Vec2;

fn coordinate(value: f32) -> u8 {
    value.floor() as i32 as u8
}

/// Proportional aim used by the original bosses and aimed orbs. The source
/// preserves fractional seed bits during its power-of-two division.
pub fn proportional_aim(position: Vec2, target: Vec2, fast: bool) -> Vec2 {
    fn component(from: u8, to: u8, seed: u8, fast: bool) -> i16 {
        let delta = i16::from(to) - i16::from(from);
        let magnitude = delta.unsigned_abs();
        let shift = if fast { 3 } else { 4 } + u32::from(magnitude >= 32);
        let units = ((magnitude << 8) | u16::from(seed)) >> shift;
        if delta < 0 {
            -(units as i16)
        } else {
            units as i16
        }
    }
    let y = component(
        coordinate(position.y),
        coordinate(target.y),
        coordinate(target.y),
        fast,
    );
    let x = component(coordinate(position.x), coordinate(target.x), y as u8, fast);
    Vec2 {
        x: f32::from(x) / 256.,
        y: f32::from(y) / 256.,
    }
}

pub fn steer(position: Vec2, velocity: &mut Vec2, target: Vec2, acceleration: u8, maximum: f32) {
    let amount = f32::from(acceleration) / 256.;
    velocity.x = (velocity.x
        + if coordinate(position.x) < coordinate(target.x) {
            amount
        } else {
            -amount
        })
    .clamp(-maximum, maximum);
    velocity.y = (velocity.y
        + if coordinate(position.y) < coordinate(target.y) {
            amount
        } else {
            -amount
        })
    .clamp(-maximum, maximum);
}

pub fn friction(velocity: &mut Vec2, amount: u8) {
    fn axis(value: f32, step: f32) -> f32 {
        if value < 0. {
            (value + step).min(0.)
        } else {
            (value - step).max(0.)
        }
    }
    let step = f32::from(amount) / 256.;
    velocity.x = axis(velocity.x, step);
    velocity.y = axis(velocity.y, step);
}

pub fn manhattan(position: Vec2, target: Vec2) -> u16 {
    let dx = coordinate(position.x).wrapping_sub(coordinate(target.x)) as i8;
    let dy = coordinate(position.y).wrapping_sub(coordinate(target.y)) as i8;
    (u16::from(dx.unsigned_abs()) + u16::from(dy.unsigned_abs())).min(255)
}
