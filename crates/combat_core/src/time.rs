pub const TICK_HZ: u32 = 30;

/// Maximum ticks simulated per `advance` call.
const MAX_TICKS_PER_ADVANCE: u32 = 8;
/// Accumulator unit is microseconds * TICK_HZ, so one tick costs exactly
/// 1_000_000 units and 1/30 s needs no rounding.
const UNITS_PER_TICK: u64 = 1_000_000;

/// Simulation time in whole ticks (1 tick = 1/30 s). All combat timing is integer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Tick(pub u64);

/// Rounds up so a 2000 ms cast never ends early.
pub fn ms_to_ticks(ms: u32) -> u64 {
    (u64::from(ms) * u64::from(TICK_HZ)).div_ceil(1000)
}

pub fn ticks_to_ms(ticks: u64) -> u64 {
    ticks * 1000 / u64::from(TICK_HZ)
}

/// Converts variable render frame time into whole fixed ticks, keeping the
/// remainder exactly (integer, scaled by `TICK_HZ`).
#[derive(Debug, Default)]
pub struct FixedStepper {
    accumulator_us: u64,
    now: Tick,
}

impl FixedStepper {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns ticks to simulate this frame, capped at 8 per call as a
    /// spiral-of-death guard. Time beyond the cap is dropped (whole ticks only;
    /// the sub-tick remainder is kept), so the sim falls behind wall clock
    /// rather than stalling the frame.
    pub fn advance(&mut self, frame_us: u64) -> u32 {
        self.accumulator_us = self
            .accumulator_us
            .saturating_add(frame_us.saturating_mul(u64::from(TICK_HZ)));
        let due = self.accumulator_us / UNITS_PER_TICK;
        let ticks = due.min(u64::from(MAX_TICKS_PER_ADVANCE)) as u32;
        self.accumulator_us %= UNITS_PER_TICK;
        self.now.0 += u64::from(ticks);
        ticks
    }

    pub fn now(&self) -> Tick {
        self.now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ms_to_ticks_rounds_up() {
        assert_eq!(ms_to_ticks(2000), 60);
        assert_eq!(ms_to_ticks(1500), 45);
        assert_eq!(ms_to_ticks(400), 12);
        assert_eq!(ms_to_ticks(1), 1);
        assert_eq!(ms_to_ticks(0), 0);
    }

    #[test]
    fn ticks_to_ms_converts() {
        assert_eq!(ticks_to_ms(30), 1000);
        assert_eq!(ticks_to_ms(60), 2000);
    }

    /// Splits one second into `n` frames that sum to exactly 1_000_000 us.
    fn run_second(n: u64) -> u32 {
        let mut s = FixedStepper::new();
        let mut total = 0;
        for i in 0..n {
            total += s.advance(1_000_000 * (i + 1) / n - 1_000_000 * i / n);
        }
        assert_eq!(s.now(), Tick(u64::from(total)));
        total
    }

    #[test]
    fn one_second_is_30_ticks_at_any_frame_rate() {
        assert_eq!(run_second(60), 30);
        assert_eq!(run_second(144), 30);
        assert_eq!(run_second(7), 30);
        assert_eq!(run_second(1000), 30);
    }

    #[test]
    fn uneven_frames_total_30_ticks() {
        let mut s = FixedStepper::new();
        let frames = [
            1_000, 50_000, 3_333, 200_000, 99_999, 250_000, 145_668, 250_000,
        ];
        assert_eq!(frames.iter().sum::<u64>(), 1_000_000);
        let total: u32 = frames.iter().map(|&f| s.advance(f)).sum();
        assert_eq!(total, 30);
    }

    #[test]
    fn caps_at_eight_and_drops_excess() {
        let mut s = FixedStepper::new();
        assert_eq!(s.advance(5_000_000), 8);
        assert_eq!(s.now(), Tick(8));
        assert_eq!(s.advance(0), 0);
    }

    #[test]
    fn zero_frame_yields_no_ticks() {
        assert_eq!(FixedStepper::new().advance(0), 0);
    }
}
