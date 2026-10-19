//! Fixed 60 Hz simulation independent of redraw frequency.
use std::time::Duration;
#[derive(Default)]
pub struct FrameClock {
    accumulated: u128,
}
impl FrameClock {
    /// Limit catch-up after a suspended window to avoid a long unresponsive loop.
    pub fn advance(&mut self, elapsed: Duration, paused: bool) -> usize {
        if paused {
            self.accumulated = 0;
            return 0;
        }
        self.accumulated += elapsed.min(Duration::from_millis(250)).as_nanos() * 60;
        let steps = self.accumulated / 1_000_000_000;
        self.accumulated %= 1_000_000_000;
        steps.min(8) as usize
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn different_render_rates_produce_same_simulation_ticks() {
        for frames in [30, 60, 120, 144] {
            let mut clock = FrameClock::default();
            let mut ticks = 0;
            // Distribute exactly one second, including division remainder.
            for i in 0..frames {
                let end = (i + 1) * 1_000_000_000 / frames;
                let start = i * 1_000_000_000 / frames;
                ticks += clock.advance(Duration::from_nanos(end - start), false);
            }
            assert_eq!(ticks, 60);
        }
    }
    #[test]
    fn pause_discards_elapsed_time_and_long_stalls_are_bounded() {
        let mut clock = FrameClock::default();
        assert_eq!(clock.advance(Duration::from_secs(10), true), 0);
        assert_eq!(clock.advance(Duration::ZERO, false), 0);
        assert_eq!(clock.advance(Duration::from_secs(10), false), 8);
    }
}
