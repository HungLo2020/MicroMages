//! Shared tower viewport and world-to-screen coordinates.
//! The viewport advances upward and also defines the gameplay visibility bounds.
use crate::physics::Vec2;
#[derive(Clone, Copy, Debug, Default)]
pub struct Camera {
    pub position: Vec2,
}
impl Camera {
    pub fn at_player(player: Vec2, world_height: f32) -> Self {
        Self {
            position: Vec2::new(
                0.0,
                (player.y - 160.0).clamp(0.0, (world_height - 240.0).max(0.0)),
            ),
        }
    }
    pub fn follow(&mut self, target: Vec2, world_height: f32) {
        let desired = (target.y - 120.0).clamp(0.0, (world_height - 240.0).max(0.0));
        // The tower camera advances upward; falling never reveals earlier rooms.
        self.position.y = self.position.y.min(desired);
    }
    pub fn screen(&self, world: Vec2) -> Vec2 {
        Vec2::new(world.x - self.position.x, world.y - self.position.y)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_rooms_remain_fixed_and_world_to_screen_is_consistent() {
        let mut camera = Camera::at_player(Vec2::new(64.0, 180.0), 200.0);
        assert_eq!(camera.position, Vec2::new(0.0, 0.0));
        camera.follow(Vec2::new(64.0, 12.0), 200.0);
        assert_eq!(
            camera.screen(Vec2::new(64.0, 180.0)),
            Vec2::new(64.0, 180.0)
        );
        let camera = Camera::at_player(Vec2::new(128.0, 956.0), 1024.0);
        assert_eq!(
            camera.screen(Vec2::new(128.0, 956.0)),
            Vec2::new(128.0, 172.0)
        );
    }
    #[test]
    fn camera_clamps_and_does_not_scroll_down_after_fall() {
        let mut camera = Camera::at_player(Vec2::new(128.0, 956.0), 1024.0);
        assert_eq!(camera.position.y, 784.0);
        camera.follow(Vec2::new(128.0, 500.0), 1024.0);
        assert_eq!(camera.position.y, 380.0);
        camera.follow(Vec2::new(128.0, 900.0), 1024.0);
        assert_eq!(camera.position.y, 380.0);
        camera.follow(Vec2::new(128.0, 0.0), 1024.0);
        assert_eq!(camera.position.y, 0.0);
    }
}
