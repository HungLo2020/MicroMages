//! Fixed 60 Hz simulation independent of redraw frequency.
//!
//! `quasar-core` owns the accumulator and its bounded catch-up policy. The game
//! keeps its own 250 ms stall guard and pause reset so a suspended window never
//! simulates a long backlog.
use quasar_core::FixedStep;
use std::time::Duration;

const SIMULATION_HZ: f64 = 60.0;
/// Match the previous clamp: at most eight simulation steps per rendered frame.
const MAX_STEPS: u32 = 8;
const STALL_LIMIT: Duration = Duration::from_millis(250);

pub struct FrameClock {
    step: FixedStep,
}
impl Default for FrameClock {
    fn default() -> Self {
        Self {
            step: FixedStep::new(SIMULATION_HZ, MAX_STEPS)
                .expect("60 Hz with an eight-step limit is a valid fixed step"),
        }
    }
}
impl FrameClock {
    /// Limit catch-up after a suspended window to avoid a long unresponsive loop.
    pub fn advance(&mut self, elapsed: Duration, paused: bool) -> usize {
        if paused {
            self.step.reset();
            return 0;
        }
        self.step.advance(elapsed.min(STALL_LIMIT));
        let mut steps = 0;
        while steps < MAX_STEPS as usize && self.step.take_tick() {
            steps += 1;
        }
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn different_render_rates_produce_the_same_simulation_rate() {
        for frames in [30, 60, 120, 144] {
            let mut clock = FrameClock::default();
            let mut ticks = 0;
            // Distribute exactly one second, including division remainder.
            for i in 0..frames {
                let end = (i + 1) * 1_000_000_000 / frames;
                let start = i * 1_000_000_000 / frames;
                ticks += clock.advance(Duration::from_nanos(end - start), false);
            }
            // A nanosecond-rounded step is 60.0 Hz to within one tick per second.
            assert!((59..=60).contains(&ticks), "{frames} frames: {ticks} ticks");
        }
    }
    #[test]
    fn long_runs_hold_the_simulation_rate() {
        let mut clock = FrameClock::default();
        let frame = Duration::from_nanos(1_000_000_000 / 120);
        let mut ticks = 0;
        for _ in 0..120 * 3600 {
            ticks += clock.advance(frame, false);
        }
        // One hour at 60 Hz, allowing the duration's nanosecond rounding.
        assert!((215_999..=216_000).contains(&ticks), "{ticks} ticks");
    }
    #[test]
    fn pause_discards_elapsed_time_and_long_stalls_are_bounded() {
        let mut clock = FrameClock::default();
        assert_eq!(clock.advance(Duration::from_secs(10), true), 0);
        assert_eq!(clock.advance(Duration::ZERO, false), 0);
        assert_eq!(clock.advance(Duration::from_secs(10), false), 8);
        assert_eq!(clock.advance(Duration::from_secs(10), false), 8);
    }
}
