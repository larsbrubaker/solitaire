//! Hint computation and Mom's-shuffle helpers for [`AppModel`].
//!
//! Extracted from `app_model.rs` to keep that file under the 800-line
//! limit. These are all `&mut self` methods on `AppModel`; they reach
//! back into the parent module for the `wallclock_seed` free function
//! and the shared game/hint types via `super`.

use super::*;

impl AppModel {
    /// Compute and store a Spider hint for the active board. No-op when
    /// the active game isn't Spider.
    pub fn show_spider_hint(&mut self) {
        let is_spider = self
            .session
            .as_ref()
            .map(|s| s.game_slug() == "spider")
            .unwrap_or(false);
        if !is_spider {
            return;
        }
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let hint = best_spider_hint(session.piles());
        match hint {
            Some(SpiderHint::Move { .. }) => {
                self.spider_hint = hint;
                // Bump even when the recommended move is unchanged so
                // the GameWidget re-plays the ghost preview animation.
                self.spider_hint_seq = self.spider_hint_seq.wrapping_add(1);
            }
            Some(SpiderHint::StockDeal { .. }) => {
                // The yellow rect on the stock pile is too quiet on
                // its own — players read "ring around a pile" as a
                // valid drop target, not as a next-action prompt.
                // Pair it with a toast so the recommendation is
                // unambiguous.
                self.spider_hint = hint;
                self.spider_hint_seq = self.spider_hint_seq.wrapping_add(1);
                self.show_toast("Deal more cards");
            }
            None => {
                // No legal cascade move and no legal stock deal — show
                // a toast so the user sees feedback for the Hint press
                // rather than silence.
                self.spider_hint = None;
                self.show_toast("No moves");
            }
        }
    }

    /// Compute and store a Klondike hint. No-op when the active
    /// game isn't Klondike. Lives in the same `spider_hint` /
    /// `spider_hint_seq` slot as the Spider hint — they're
    /// mutually-exclusive since only one game is active at a time.
    /// The rendering code reads the slot blind, so the name's a
    /// holdover until a wider rename.
    pub fn show_klondike_hint(&mut self) {
        let is_klondike = self
            .session
            .as_ref()
            .map(|s| s.game_slug() == "klondike")
            .unwrap_or(false);
        if !is_klondike {
            return;
        }
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let rules = crate::games::klondike::Klondike::with_draw_count(self.klondike_draw_count);
        let hint = crate::games::klondike_hint::best_klondike_hint(&rules, session.piles());
        match hint {
            Some(SpiderHint::Move { .. }) => {
                self.spider_hint = hint;
                self.spider_hint_seq = self.spider_hint_seq.wrapping_add(1);
            }
            Some(SpiderHint::StockDeal { .. }) => {
                self.spider_hint = hint;
                self.spider_hint_seq = self.spider_hint_seq.wrapping_add(1);
                self.show_toast("Draw from stock");
            }
            None => {
                self.spider_hint = None;
                self.show_toast("No moves");
            }
        }
    }

    /// Variant-aware hint dispatcher used by the HUD Hint button +
    /// 'h' hotkey + Game-menu "Hint" entry.
    pub fn show_hint(&mut self) {
        match self.kind {
            Some(GameKind::Spider) => self.show_spider_hint(),
            Some(GameKind::Klondike) => self.show_klondike_hint(),
            _ => {}
        }
    }

    /// Drop any pending Spider hint. Called by every move/undo path so
    /// the highlight never lingers past the board state it was computed
    /// for.
    pub fn clear_spider_hint(&mut self) {
        self.spider_hint = None;
    }

    /// Mom's Solitaire: shuffle the out-of-order cells in place,
    /// increment the on-screen shuffle counter, and clear any pending
    /// king-pickup. No-op on any other variant. Returns `true` if at
    /// least one swap was performed.
    pub fn try_moms_shuffle(&mut self) -> bool {
        if !matches!(self.kind, Some(GameKind::MomsSolitaire)) {
            return false;
        }
        let Some(session) = self.session.as_mut() else {
            return false;
        };
        let mut rng = StdRng::seed_from_u64(wallclock_seed());
        let swaps = crate::games::moms::compute_shuffle_swaps(session.piles(), &mut rng);
        if swaps.is_empty() {
            return false;
        }
        // Shuffle swaps never satisfy Mom's user-facing `legal_move`
        // (which requires the destination to be an Ace gap matching
        // its left neighbour). Use the unchecked path; the swaps
        // still land on the undo stack.
        for (a, b) in swaps {
            session.apply_forced(crate::session::Move::swap(a, b));
        }
        self.moms_shuffles += 1;
        self.moms_waiting_king_at = None;
        true
    }
}
