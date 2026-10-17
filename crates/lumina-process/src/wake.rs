//! Notices that the computer slept.
//!
//! No OS power API is needed: a timer ticks every [`TICK`] and the wall clock
//! is read on each tick. While the computer is suspended the wall clock keeps
//! going but the program does not, so the gap between two ticks becomes far
//! longer than [`TICK`]. This works the same on Windows, macOS and Linux. A
//! clock set forward by hand looks like a wake too; that only costs a check.

use std::time::{Duration, SystemTime};

use tokio::time::sleep;

/// How often the wall clock is read.
pub const TICK: Duration = Duration::from_secs(10);

/// How much longer than [`TICK`] a gap must be to count as a sleep. Generous,
/// so a busy system or a debugger pause is not mistaken for one.
pub const GRACE: Duration = Duration::from_secs(30);

/// Whether `wall` (wall-clock time between two ticks `tick` apart) shows that
/// the computer was suspended. Returns the time asleep, roughly.
pub fn slept(tick: Duration, wall: Duration) -> Option<Duration> {
    (wall > tick + GRACE).then(|| wall - tick)
}

/// Runs forever, calling `on_wake` with the time asleep after each sleep.
pub async fn watch(mut on_wake: impl FnMut(Duration) + Send) {
    let mut last = SystemTime::now();
    loop {
        sleep(TICK).await;
        let now = SystemTime::now();
        // A clock set back gives an error: not a sleep.
        if let Ok(wall) = now.duration_since(last)
            && let Some(asleep) = slept(TICK, wall)
        {
            on_wake(asleep);
        }
        last = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_gaps_are_not_sleeps() {
        assert_eq!(slept(TICK, TICK), None);
        assert_eq!(slept(TICK, TICK + Duration::from_secs(5)), None);
        assert_eq!(slept(TICK, TICK + GRACE), None);
    }

    #[test]
    fn long_gaps_are_sleeps() {
        let gap = TICK + Duration::from_secs(3600);
        assert_eq!(slept(TICK, gap), Some(Duration::from_secs(3600)));
    }
}
