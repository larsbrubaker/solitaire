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
        let mut piles = Vec::with_capacity(slots.len());
        for s in slots {
            piles.push(Pile::new(s.id, s.kind, s.layout, s.origin_x, s.origin_y));
        }
        // Sanity: ids should be 0..n in declaration order so we can index
        // directly without a hashmap. Game rules build slot tables this way.
        for (i, p) in piles.iter().enumerate() {
            debug_assert_eq!(p.id as usize, i, "PileSlot ids must be contiguous 0..n");
        }
        Self { piles }
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
