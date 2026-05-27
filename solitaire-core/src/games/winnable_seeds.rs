//! Bundled lists of solver-verified winnable seeds, plus the
//! tiny pickers the deal pipeline uses when the player has
//! "Winnable deals only" turned on for a given variant.
//!
//! - FreeCell uses Microsoft's classic Jim Horne deal numbering
//!   (1..32_000 verified except #11982). No solver run is needed —
//!   the historical Microsoft set is the authoritative source.
//! - Spider + Klondike use `.bin` files of little-endian `u64`s
//!   produced by the offline Solvitaire pipeline (see
//!   `docs/winnable-seeds.md`). The shipped files start empty;
//!   the picker falls through to the wallclock seed in that case.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Microsoft FreeCell game numbers known to be unwinnable. Don
/// Woods' 1994 sweep validated all of the original 32,000 except
/// #11982; the remaining seven are the additional unwinnables
/// found inside Microsoft Solitaire Collection's 1..1,000,000
/// range — i.e. **eight unwinnable games out of a million**, every
/// other deal solves.
pub const MS_FREECELL_UNWINNABLE_LIST: &[u32] = &[
    11_982, 146_692, 186_216, 455_889, 495_505, 512_118, 517_776, 781_948,
];

/// Legacy alias: the single unwinnable in the classic 32k set.
pub const MS_FREECELL_UNWINNABLE: u32 = 11_982;

/// Highest Microsoft FreeCell game number we hand out from the
/// winnable pool. Microsoft Solitaire Collection's range
/// (1..1,000,000) is the wide pool — only eight deals in that
/// range fail, and we blocklist every one of them.
pub const MS_FREECELL_MAX: u32 = 1_000_000;

/// Raw little-endian `u64`s, eight bytes per seed.
const SPIDER_SEEDS_BIN: &[u8] = include_bytes!("../../assets/spider_winnable_seeds.bin");
const KLONDIKE_SEEDS_BIN: &[u8] = include_bytes!("../../assets/klondike_winnable_seeds.bin");

pub fn spider_seeds() -> Vec<u64> {
    decode_seeds(SPIDER_SEEDS_BIN)
}

pub fn klondike_seeds() -> Vec<u64> {
    decode_seeds(KLONDIKE_SEEDS_BIN)
}

fn decode_seeds(bytes: &[u8]) -> Vec<u64> {
    bytes
        .chunks_exact(8)
        .map(|c| {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(c);
            u64::from_le_bytes(buf)
        })
        .collect()
}

/// Pick a Spider seed at wallclock-random from the bundled list.
/// Falls back to the wallclock seed when the list is empty so the
/// feature can ship with an unpopulated file (the offline pipeline
/// can grow it without blocking the rest of the integration).
pub fn pick_spider_winnable(fallback_seed: u64) -> u64 {
    pick_from(SPIDER_SEEDS_BIN, fallback_seed)
}

pub fn pick_klondike_winnable(fallback_seed: u64) -> u64 {
    pick_from(KLONDIKE_SEEDS_BIN, fallback_seed)
}

fn pick_from(bytes: &[u8], fallback_seed: u64) -> u64 {
    let seeds = decode_seeds(bytes);
    if seeds.is_empty() {
        return fallback_seed;
    }
    let mut rng = StdRng::seed_from_u64(fallback_seed);
    seeds[rng.gen_range(0..seeds.len())]
}

/// Pick a Microsoft FreeCell game number in `[1, MS_FREECELL_MAX]`
/// excluding every known-unwinnable from
/// `MS_FREECELL_UNWINNABLE_LIST`. Uses `fallback_seed` as RNG state
/// so the pick is reproducible.
pub fn pick_ms_freecell_winnable(fallback_seed: u64) -> u32 {
    let mut rng = StdRng::seed_from_u64(fallback_seed);
    loop {
        let n = rng.gen_range(1..=MS_FREECELL_MAX);
        if !MS_FREECELL_UNWINNABLE_LIST.contains(&n) {
            return n;
        }
    }
}

/// Returns true if `n` is in the published list of unwinnable
/// Microsoft FreeCell deals.
pub fn is_ms_freecell_unwinnable(n: u32) -> bool {
    MS_FREECELL_UNWINNABLE_LIST.contains(&n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_seed_lists_are_well_formed() {
        // Every entry must be a full eight bytes; partial trailing
        // bytes would mean a corrupt build artifact.
        assert_eq!(SPIDER_SEEDS_BIN.len() % 8, 0);
        assert_eq!(KLONDIKE_SEEDS_BIN.len() % 8, 0);
    }

    #[test]
    fn picker_returns_a_listed_seed_when_list_nonempty() {
        let seeds = spider_seeds();
        if seeds.is_empty() {
            return; // CI sanity: skip when the list hasn't been populated yet.
        }
        let picked = pick_spider_winnable(12345);
        assert!(seeds.contains(&picked));
    }

    #[test]
    fn ms_freecell_picker_skips_known_unwinnable() {
        for trial in 0..200 {
            let n = pick_ms_freecell_winnable(trial);
            assert!(!MS_FREECELL_UNWINNABLE_LIST.contains(&n));
            assert!((1..=MS_FREECELL_MAX).contains(&n));
        }
    }

    #[test]
    fn ms_freecell_unwinnable_list_contains_known_examples() {
        // The classic 32k set's lone unwinnable.
        assert!(is_ms_freecell_unwinnable(11_982));
        // One from the extended 1M-deal Microsoft Solitaire Collection.
        assert!(is_ms_freecell_unwinnable(781_948));
        // Known-winnable Game #1 should NOT be on the blocklist.
        assert!(!is_ms_freecell_unwinnable(1));
    }
}
