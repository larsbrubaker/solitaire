//! Background-threaded generator that runs the in-process Spider /
//! Klondike solvers over consecutive seed ranges and appends each
//! winnable seed to the bundled `.bin` file. Driven by the
//! Debug → Generate ___ Seeds menu entries.
//!
//! The bundled files live in `solitaire-core/assets/*.bin`; the
//! generator writes through the compile-time
//! `CARGO_MANIFEST_DIR` path so a dev running this from the repo
//! root lands on the right file. The wasm build no-ops — there's
//! no disk to write to, and the user will only ever run the
//! generator on native.
//!
//! Concurrency: a single `AtomicBool` guards each variant so a
//! second click while the first run is alive is a polite no-op
//! (logged to stdout). The worker thread uses a 64 MB stack — the
//! solvers can hit deep DFS frames on hard Spider boards.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::games::klondike::Klondike;
use crate::games::klondike_solver;
use crate::games::spider::Spider;
use crate::games::spider_solver;
use crate::session::GameSession;

static SPIDER_RUNNING: AtomicBool = AtomicBool::new(false);
static KLONDIKE_RUNNING: AtomicBool = AtomicBool::new(false);

/// Per-seed solver budget. Generous on time but capped on nodes so
/// a single pathological deal can't stall the whole sweep.
#[cfg(not(target_arch = "wasm32"))]
const PER_SEED_DURATION: std::time::Duration = std::time::Duration::from_secs(15);
#[cfg(not(target_arch = "wasm32"))]
const PER_SEED_MAX_NODES: u64 = 2_000_000;

/// How many seeds the generator sweeps from `start` per invocation.
/// Re-clicking the menu entry resumes from the next un-tested seed
/// (we read the existing `.bin` to recover the highest seed seen).
#[cfg(not(target_arch = "wasm32"))]
const SEEDS_PER_RUN: u64 = 200;

#[cfg(not(target_arch = "wasm32"))]
fn assets_path(file: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("assets");
    p.push(file);
    p
}

#[cfg(not(target_arch = "wasm32"))]
fn existing_seeds(path: &PathBuf) -> std::collections::HashSet<u64> {
    let Ok(bytes) = std::fs::read(path) else {
        return Default::default();
    };
    bytes
        .chunks_exact(8)
        .map(|c| {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(c);
            u64::from_le_bytes(buf)
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn next_seed_to_try(existing: &std::collections::HashSet<u64>) -> u64 {
    // Resume from one past the highest verified seed so re-runs
    // extend the pool instead of revisiting the same range.
    existing.iter().copied().max().map_or(0, |m| m + 1)
}

/// Kick off Spider generation in a background thread. No-op on wasm
/// or if a previous run is still going.
pub fn start_spider_generation() {
    #[cfg(target_arch = "wasm32")]
    {
        println!("[seed_generator] spider: wasm target, no disk");
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if SPIDER_RUNNING.swap(true, Ordering::SeqCst) {
            println!("[seed_generator] spider: already running");
            return;
        }
        std::thread::Builder::new()
            .name("spider-seed-gen".into())
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                run_spider();
                SPIDER_RUNNING.store(false, Ordering::SeqCst);
            })
            .expect("spawn spider seed generator");
    }
}

pub fn start_klondike_generation() {
    #[cfg(target_arch = "wasm32")]
    {
        println!("[seed_generator] klondike: wasm target, no disk");
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if KLONDIKE_RUNNING.swap(true, Ordering::SeqCst) {
            println!("[seed_generator] klondike: already running");
            return;
        }
        std::thread::Builder::new()
            .name("klondike-seed-gen".into())
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                run_klondike();
                KLONDIKE_RUNNING.store(false, Ordering::SeqCst);
            })
            .expect("spawn klondike seed generator");
    }
}

pub fn spider_generation_running() -> bool {
    SPIDER_RUNNING.load(Ordering::SeqCst)
}

pub fn klondike_generation_running() -> bool {
    KLONDIKE_RUNNING.load(Ordering::SeqCst)
}

#[cfg(not(target_arch = "wasm32"))]
fn run_spider() {
    let path = assets_path("spider_winnable_seeds.bin");
    let existing = existing_seeds(&path);
    let start = next_seed_to_try(&existing);
    println!(
        "[seed_generator] spider: scanning {} seeds from {} (existing winners: {})",
        SEEDS_PER_RUN,
        start,
        existing.len()
    );
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .expect("open spider seed file");
    let mut won = 0u64;
    let mut lost = 0u64;
    let mut timed_out = 0u64;
    let t0 = std::time::Instant::now();
    for off in 0..SEEDS_PER_RUN {
        let seed = start + off;
        let session = GameSession::new(Spider::four_suit(), seed);
        let budget = spider_solver::SolverBudget::from_duration(PER_SEED_DURATION, PER_SEED_MAX_NODES);
        let t = std::time::Instant::now();
        let result = spider_solver::solve(&session.piles, budget);
        let elapsed = t.elapsed();
        match result {
            spider_solver::SolveResult::Won => {
                won += 1;
                file.write_all(&seed.to_le_bytes()).expect("append seed");
                file.flush().ok();
                println!(
                    "[seed_generator] spider seed {:>8} WON in {:>5}ms (running: won {} / lost {} / timeout {})",
                    seed,
                    elapsed.as_millis(),
                    won,
                    lost,
                    timed_out
                );
            }
            spider_solver::SolveResult::Exhausted => lost += 1,
            spider_solver::SolveResult::Timeout => timed_out += 1,
        }
    }
    println!(
        "[seed_generator] spider done: won={} lost={} timeout={} elapsed={:.1}s",
        won,
        lost,
        timed_out,
        t0.elapsed().as_secs_f64()
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn run_klondike() {
    let path = assets_path("klondike_winnable_seeds.bin");
    let existing = existing_seeds(&path);
    let start = next_seed_to_try(&existing);
    println!(
        "[seed_generator] klondike: scanning {} seeds from {} (existing winners: {})",
        SEEDS_PER_RUN,
        start,
        existing.len()
    );
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .expect("open klondike seed file");
    let mut won = 0u64;
    let mut lost = 0u64;
    let mut timed_out = 0u64;
    let t0 = std::time::Instant::now();
    for off in 0..SEEDS_PER_RUN {
        let seed = start + off;
        let session = GameSession::new(Klondike::with_draw_count(1), seed);
        let budget = klondike_solver::SolverBudget::from_duration(
            PER_SEED_DURATION,
            PER_SEED_MAX_NODES,
            1,
        );
        let t = std::time::Instant::now();
        let result = klondike_solver::solve(&session.piles, budget);
        let elapsed = t.elapsed();
        match result {
            klondike_solver::SolveResult::Won => {
                won += 1;
                file.write_all(&seed.to_le_bytes()).expect("append seed");
                file.flush().ok();
                println!(
                    "[seed_generator] klondike seed {:>8} WON in {:>5}ms (running: won {} / lost {} / timeout {})",
                    seed,
                    elapsed.as_millis(),
                    won,
                    lost,
                    timed_out
                );
            }
            klondike_solver::SolveResult::Exhausted => lost += 1,
            klondike_solver::SolveResult::Timeout => timed_out += 1,
        }
    }
    println!(
        "[seed_generator] klondike done: won={} lost={} timeout={} elapsed={:.1}s",
        won,
        lost,
        timed_out,
        t0.elapsed().as_secs_f64()
    );
}
