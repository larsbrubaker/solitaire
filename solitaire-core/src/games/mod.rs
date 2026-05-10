//! `GameRules` — the seam between the four solitaire variants and the
//! shared engine. Each variant implements this trait; `GameWidget<R>` is
//! generic over `R: GameRules`.

pub mod klondike;

use rand::rngs::StdRng;

use crate::piles::{PileSet, PileSlot};
use crate::session::Move;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameKind {
    Klondike,
    FreeCell,
    Spider,
    Classic,
}

impl GameKind {
    pub fn slug(self) -> &'static str {
        match self {
            GameKind::Klondike => "klondike",
            GameKind::FreeCell => "freecell",
            GameKind::Spider => "spider",
            GameKind::Classic => "classic",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            GameKind::Klondike => "Klondike",
            GameKind::FreeCell => "FreeCell",
            GameKind::Spider => "Spider",
            GameKind::Classic => "Classic",
        }
    }
}

/// Rules + initial deal + win condition for one solitaire variant.
///
/// The trait is intentionally narrow: rules engines stay stateless beyond
/// the move history tracked by `session::UndoStack`.
pub trait GameRules: 'static {
    /// Layout of every pile this variant uses. Order in the slice
    /// determines `PileId`s — first slot is id 0, etc.
    fn pile_layout(&self) -> &'static [PileSlot];

    /// Populate the (already-empty) pile set with a fresh deal.
    fn deal(&self, piles: &mut PileSet, rng: &mut StdRng);

    /// Validate a candidate move against the current pile state.
    /// Returns `true` if the move is legal under this variant's rules.
    fn legal_move(&self, piles: &PileSet, m: &Move) -> bool;

    /// Optional auto-complete suggestion: when no face-down cards remain,
    /// return one auto-flush move at a time so the win can play out
    /// without manual dragging. Returns `None` when no auto step applies.
    fn auto_complete_step(&self, piles: &PileSet) -> Option<Move>;

    /// True when the player has won.
    fn is_won(&self, piles: &PileSet) -> bool;

    /// Database slug for `public.games.slug`.
    fn game_slug(&self) -> &'static str;

    /// "Drawer-tap" semantics — rules-defined click handler for
    /// non-drag interactions. Klondike uses this to emit the stock→waste
    /// dispense and the stock-recycle moves when the player clicks the
    /// stock pile.
    fn on_pile_click(&self, _piles: &PileSet, _pile: crate::piles::PileId) -> Option<Move> {
        None
    }
}
