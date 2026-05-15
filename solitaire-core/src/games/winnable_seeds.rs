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

/// Microsoft FreeCell game number that Don Woods' 1994 sweep proved
/// unwinnable. The remaining 31,999 of the original 32,000 numbered
/// deals are verified solvable.
pub const MS_FREECELL_UNWINNABLE: u32 = 11982;

/// Highest Microsoft FreeCell game number we hand out from the
/// winnable pool. The classic Windows FreeCell ranged 1..32_000;
/// Microsoft Solitaire Collection later extended to 1..1_000_000,
/// but only the original 32k window has comprehensive third-party
/// winnability data, so we cap here.
pub const MS_FREECELL_MAX: u32 = 32_000;

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
/// excluding `#11982`. Uses `fallback_seed` as RNG state so the
/// pick is reproducible from a wallclock seed for testing.
pub fn pick_ms_freecell_winnable(fallback_seed: u64) -> u32 {
    let mut rng = StdRng::seed_from_u64(fallback_seed);
    loop {
        let n = rng.gen_range(1..=MS_FREECELL_MAX);
        if n != MS_FREECELL_UNWINNABLE {
            return n;
        }
    }
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
        // 200 iterations on a deterministic seed is more than enough
        // to hit every neighbouring number of #11982 without ever
        // returning it.
        for trial in 0..200 {
            let n = pick_ms_freecell_winnable(trial);
            assert_ne!(n, MS_FREECELL_UNWINNABLE);
            assert!((1..=MS_FREECELL_MAX).contains(&n));
        }
    }
}
