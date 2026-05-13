//! `GameRules` — the seam between the four solitaire variants and the
//! shared engine. Each variant implements this trait; `GameWidget` holds
//! the active session behind a `DynGameSession` trait object.

pub mod freecell;
pub mod klondike;
pub mod moms;
pub mod spider;

use agg_gui::geometry::Rect;
use rand::rngs::StdRng;

use crate::piles::{PileId, PileSet, PileSlot};
use crate::session::Move;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameKind {
    Klondike,
    FreeCell,
    Spider,
    MomsSolitaire,
}

impl GameKind {
    pub fn slug(self) -> &'static str {
        match self {
            GameKind::Klondike => "klondike",
            GameKind::FreeCell => "freecell",
            GameKind::Spider => "spider",
            GameKind::MomsSolitaire => "moms",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            GameKind::Klondike => "Klondike",
            GameKind::FreeCell => "FreeCell",
            GameKind::Spider => "Spider",
            GameKind::MomsSolitaire => "Mom's Solitaire",
        }
    }
}

/// Standard playing-card aspect ratio (height / width). Every variant
/// picks `card_h = card_w * CARD_ASPECT` so the on-screen cards keep a
/// recognisable shape regardless of available pixels.
pub const CARD_ASPECT: f64 = 7.0 / 5.0;

pub trait GameRules: 'static {
    /// Compute pile positions, sizes, and rendering config for the
    /// given playfield rect in SCREEN coordinates. The game picks a
    /// card size that fits its column count + worst-case fan within
    /// `rect`, then places each pile's bottom-left card origin.
    /// `GameWidget` calls this on every viewport change and re-
    /// applies the result via `PileSet::update_layout` (card state
    /// is preserved).
    fn pile_layout(&self, rect: Rect) -> Vec<PileSlot>;
    fn deal(&self, piles: &mut PileSet, rng: &mut StdRng);
    fn legal_move(&self, piles: &PileSet, m: &Move) -> bool;
    fn auto_complete_step(&self, piles: &PileSet) -> Option<Move>;
    fn is_won(&self, piles: &PileSet) -> bool;
    fn game_slug(&self) -> &'static str;

    /// Click handler — used by stock-tap interactions (Klondike stock
    /// dispense/recycle; Spider broadcast-deal). Returns the
    /// sequence of moves to apply in order. Empty vec means the click
    /// produced no moves.
    fn on_pile_click(&self, _piles: &PileSet, _pile: PileId) -> Vec<Move> {
        Vec::new()
    }

    /// Single-click card move helper. Variants that support click-to-
    /// move can return the move they want applied for `card_idx` in
    /// `pile`; the UI only calls this for a press/release with no drag.
    fn single_click_move(&self, _piles: &PileSet, _pile: PileId, _card_idx: usize) -> Option<Move> {
        None
    }

    /// After-move hook — called by `GameSession::try_apply` in a loop
    /// after every user move. Used by Spider to collapse complete K→A
    /// suited runs into a foundation. Returns `None` when no auto step
    /// applies.
    fn after_move(&self, _piles: &PileSet) -> Option<Move> {
        None
    }
}
