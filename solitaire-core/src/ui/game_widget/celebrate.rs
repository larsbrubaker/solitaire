//! Win-celebration trigger latch.
//!
//! `GameWidget` fires a one-shot [`agg_gui::confetti::ConfettiSystem`]
//! burst when the board first enters its won state. `WinLatch` turns the
//! per-frame "are we won?" query into a single not-won -> won *edge* so
//! the burst fires exactly once per win, and it tells the widget when to
//! drop a live burst so a stale one can never linger.
//!
//! The latch re-arms whenever the board leaves the won state. In
//! practice the app only leaves `Screen::Won` via a new deal, restart,
//! or game switch (all of which reset back to `Screen::Game`, or to
//! `Screen::Title` where the session is gone entirely — both report
//! not-won), so a fresh win after any of those celebrates again.

/// What the widget should do with its confetti this frame, decided from
/// the current won state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CelebrationAction {
    /// Leave the current confetti (if any) alone — a burst that is
    /// already playing keeps animating.
    Keep,
    /// Spawn a fresh burst: this frame is the not-won -> won edge.
    Fire,
    /// Drop any live burst and reset its timing — the board is no
    /// longer in a won state (normal play, a new deal, or we left the
    /// playfield entirely), so nothing should be celebrating.
    Drop,
}

/// Edge detector for the win celebration.
#[derive(Default)]
pub(super) struct WinLatch {
    /// Whether we've already fired for the current won episode.
    fired: bool,
}

impl WinLatch {
    /// Decide this frame's confetti action from the current won state.
    ///
    /// * not won -> [`CelebrationAction::Drop`] and re-arm, so any burst
    ///   from a previous win is cleared and the next win fires again.
    /// * first won frame -> [`CelebrationAction::Fire`].
    /// * still won after firing -> [`CelebrationAction::Keep`].
    pub fn observe(&mut self, won: bool) -> CelebrationAction {
        if !won {
            self.fired = false;
            return CelebrationAction::Drop;
        }
        if self.fired {
            return CelebrationAction::Keep;
        }
        self.fired = true;
        CelebrationAction::Fire
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_once_on_transition() {
        let mut latch = WinLatch::default();
        // Not won yet: no burst, nothing to keep.
        assert_eq!(latch.observe(false), CelebrationAction::Drop);
        // First won frame: fire.
        assert_eq!(latch.observe(true), CelebrationAction::Fire);
    }

    #[test]
    fn keeps_while_still_won() {
        let mut latch = WinLatch::default();
        assert_eq!(latch.observe(true), CelebrationAction::Fire);
        // Subsequent won frames keep the running burst, never re-fire.
        assert_eq!(latch.observe(true), CelebrationAction::Keep);
        assert_eq!(latch.observe(true), CelebrationAction::Keep);
    }

    #[test]
    fn drops_and_rearms_after_leaving_won_state() {
        let mut latch = WinLatch::default();
        assert_eq!(latch.observe(true), CelebrationAction::Fire);
        assert_eq!(latch.observe(true), CelebrationAction::Keep);
        // Leaving the won state (new deal / restart / switch / Home)
        // drops any live burst and re-arms.
        assert_eq!(latch.observe(false), CelebrationAction::Drop);
        // Winning again fires a fresh celebration.
        assert_eq!(latch.observe(true), CelebrationAction::Fire);
    }

    #[test]
    fn stays_dropped_while_never_won() {
        let mut latch = WinLatch::default();
        for _ in 0..5 {
            assert_eq!(latch.observe(false), CelebrationAction::Drop);
        }
    }
}
