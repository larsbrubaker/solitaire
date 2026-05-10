//! `GameRules` — the seam between the four solitaire variants and the
//! shared engine. Each variant implements this trait; `GameWidget` holds
//! the active session behind a `DynGameSession` trait object.

pub mod freecell;
pub mod klondike;
pub mod moms;
pub mod spider;

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

pub trait GameRules: 'static {
    fn pile_layout(&self) -> &'static [PileSlot];
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

    /// After-move hook — called by `GameSession::try_apply` in a loop
    /// after every user move. Used by Spider to collapse complete K→A
    /// suited runs into a foundation. Returns `None` when no auto step
    /// applies.
    fn after_move(&self, _piles: &PileSet) -> Option<Move> {
        None
    }
}
