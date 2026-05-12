//! `PileSet` — owns every pile in the active game session.

use super::hit::{HitResult, PileSlot};
use super::pile::Pile;
use super::PileId;

#[derive(Clone, Debug, Default)]
pub struct PileSet {
    piles: Vec<Pile>,
}

impl PileSet {
    pub fn from_slots(slots: &[PileSlot]) -> Self {
        let mut piles: Vec<Pile> = slots.iter().map(Pile::from_slot).collect();
        // Sanity: ids should be 0..n in declaration order so we can index
        // directly without a hashmap. Game rules build slot tables this way.
        for (i, p) in piles.iter_mut().enumerate() {
            debug_assert_eq!(p.id as usize, i, "PileSlot ids must be contiguous 0..n");
        }
        Self { piles }
    }

    /// Re-apply a fresh slot table to the existing piles WITHOUT
    /// resetting their card stacks. `slots` must match the piles by
    /// id (same length, same id order — guaranteed by the
    /// `GameRules::pile_layout` contract).
    pub fn update_layout(&mut self, slots: &[PileSlot]) {
        debug_assert_eq!(self.piles.len(), slots.len());
        for (pile, slot) in self.piles.iter_mut().zip(slots) {
            pile.apply_slot(slot);
        }
    }

    pub fn len(&self) -> usize {
        self.piles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.piles.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Pile> {
        self.piles.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Pile> {
        self.piles.iter_mut()
    }

    pub fn get(&self, id: PileId) -> &Pile {
        &self.piles[id as usize]
    }

    pub fn get_mut(&mut self, id: PileId) -> &mut Pile {
        &mut self.piles[id as usize]
    }

    /// Hit-test the entire playfield. Iterates piles in order; the first
    /// hit wins. Tableau columns are tested before stock/waste/foundations
    /// only by virtue of their declaration order in the variant's slot
    /// table — but in practice the bounding rects don't overlap so order
    /// doesn't matter.
    pub fn hit_test(&self, x: f64, y: f64) -> Option<HitResult> {
        for p in &self.piles {
            if let Some(hit) = p.hit_test(x, y) {
                return Some(hit);
            }
        }
        None
    }
}
