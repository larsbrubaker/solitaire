//! Background-threaded generator for the bundled Spider / Klondike
//! winnable-seed pools. Driven by the Debug → Generate Seed Games
//! menu action: a single click spawns one worker per variant, both
//! sweep consecutive `u64` seeds, run the in-process solver, and
//! append every winner to the corresponding `solitaire-core/assets/
//! *_winnable_seeds.bin` file. The workers run until either the
//! target count is reached or the user clicks Stop.
//!
//! Progress + a rolling log are exposed through `Arc<Mutex<SeedGenStatus>>`
//! so the on-screen window can poll it without coupling to the
//! worker. The mutex is short-held (status update only) — solver
//! time is spent outside the lock.
//!
//! Concurrency: each variant has a single status singleton; the
//! `running` flag inside it doubles as the start-once / stop-from-
//! UI signal. Worker checks the flag every iteration and exits
//! cleanly when it flips false.

#[cfg(not(target_arch = "wasm32"))]
use std::collections::VecDeque;
#[cfg(not(target_arch = "wasm32"))]
use std::fs::OpenOptions;
#[cfg(not(target_arch = "wasm32"))]
use std::io::Write;
use std::sync::{Arc, Mutex};

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use crate::games::klondike::Klondike;
#[cfg(not(target_arch = "wasm32"))]
use crate::games::klondike_solver;
#[cfg(not(target_arch = "wasm32"))]
use crate::games::spider::Spider;
#[cfg(not(target_arch = "wasm32"))]
use crate::games::spider_solver;
#[cfg(not(target_arch = "wasm32"))]
use crate::session::GameSession;

/// Target count of verified winnable seeds per variant. Workers
/// stop once the bundled file holds this many entries.
pub const TARGET_SEED_COUNT: u64 = 8_000;

/// Maximum log lines retained in the rolling buffer (older drops
/// off the front). Keeps the mutex payload bounded.
const LOG_CAPACITY: usize = 200;

/// Per-seed solver budgets. Spider 4-suit is the slowest patience
/// variant in published research (Solvitaire's JAIR paper notes
/// Spider as the only game its solver reports with wider-than-0.1 %
/// confidence intervals) so it gets a much bigger budget than
/// Klondike. Node caps stop a single pathological deal from
/// stalling the whole sweep.
#[cfg(not(target_arch = "wasm32"))]
const SPIDER_PER_SEED_DURATION: std::time::Duration = std::time::Duration::from_secs(120);
#[cfg(not(target_arch = "wasm32"))]
const SPIDER_PER_SEED_MAX_NODES: u64 = 50_000_000;
#[cfg(not(target_arch = "wasm32"))]
const KLONDIKE_PER_SEED_DURATION: std::time::Duration = std::time::Duration::from_secs(30);
#[cfg(not(target_arch = "wasm32"))]
const KLONDIKE_PER_SEED_MAX_NODES: u64 = 10_000_000;

#[derive(Clone, Debug, Default)]
pub struct SeedGenStatus {
    pub running: bool,
    pub finished: bool,
    pub current_seed: u64,
    pub won: u64,
    pub lost: u64,
    pub timed_out: u64,
    pub target: u64,
    /// Rolling log of recent events ("seed 47 WON in 312 ms" /
    /// "seed 48 timed out", etc.). UI renders the tail.
    pub log: VecDequeWrapper,
}

/// Rolling string buffer with a hard cap. Wrapped in a newtype so
/// `SeedGenStatus` can derive `Default` without a custom impl.
#[derive(Clone, Debug, Default)]
pub struct VecDequeWrapper(
    #[cfg(not(target_arch = "wasm32"))] pub VecDeque<String>,
    #[cfg(target_arch = "wasm32")] pub Vec<String>,
);

impl VecDequeWrapper {
    pub fn push(&mut self, line: String) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.0.len() >= LOG_CAPACITY {
                self.0.pop_front();
            }
            self.0.push_back(line);
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.0.push(line);
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.0.iter()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Singleton handle to the Spider status block.
pub fn spider_status() -> Arc<Mutex<SeedGenStatus>> {
    use std::sync::OnceLock;
    static S: OnceLock<Arc<Mutex<SeedGenStatus>>> = OnceLock::new();
    S.get_or_init(|| Arc::new(Mutex::new(SeedGenStatus::default())))
        .clone()
}

pub fn klondike_status() -> Arc<Mutex<SeedGenStatus>> {
    use std::sync::OnceLock;
    static S: OnceLock<Arc<Mutex<SeedGenStatus>>> = OnceLock::new();
    S.get_or_init(|| Arc::new(Mutex::new(SeedGenStatus::default())))
        .clone()
}

/// Kick off Spider + Klondike workers if neither is already
/// running. Idempotent — a second click while a sweep is alive is
/// a no-op (`running` flag guards). No-op on wasm.
pub fn start_seed_generation() {
    #[cfg(target_arch = "wasm32")]
    {
        // No disk to write to; surface a hint via the log buffer.
        let st = spider_status();
        if let Ok(mut s) = st.lock() {
            s.log.push("wasm target: generator unavailable".into());
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        spawn_worker(spider_status(), Variant::Spider);
        spawn_worker(klondike_status(), Variant::Klondike);
    }
}

/// Set the `running` flag to false on both status blocks. The
/// workers see it on the next iteration and exit.
pub fn stop_seed_generation() {
    for st in [spider_status(), klondike_status()] {
        if let Ok(mut s) = st.lock() {
            s.running = false;
        }
    }
}

/// True if either worker is mid-sweep.
pub fn seed_generation_running() -> bool {
    for st in [spider_status(), klondike_status()] {
        if let Ok(s) = st.lock() {
            if s.running {
                return true;
            }
        }
    }
    false
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
enum Variant {
    Spider,
    Klondike,
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_worker(status: Arc<Mutex<SeedGenStatus>>, variant: Variant) {
    {
        let mut s = status.lock().expect("status lock");
        if s.running {
            return; // already running
        }
        s.running = true;
        s.finished = false;
        s.target = TARGET_SEED_COUNT;
        let label = match variant {
            Variant::Spider => "spider",
            Variant::Klondike => "klondike",
        };
        s.log.push(format!("[{label}] starting sweep…"));
    }
    let st = status.clone();
    let name = match variant {
        Variant::Spider => "spider-seed-gen",
        Variant::Klondike => "klondike-seed-gen",
    };
    std::thread::Builder::new()
        .name(name.into())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || run(st, variant))
        .expect("spawn worker");
}

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
fn run(status: Arc<Mutex<SeedGenStatus>>, variant: Variant) {
    let (file_name, label) = match variant {
        Variant::Spider => ("spider_winnable_seeds.bin", "spider"),
        Variant::Klondike => ("klondike_winnable_seeds.bin", "klondike"),
    };
    let path = assets_path(file_name);
    let existing = existing_seeds(&path);
    let already_won = existing.len() as u64;
    let mut next_seed = existing.iter().copied().max().map_or(0, |m| m + 1);

    {
        let mut s = status.lock().expect("status lock");
        s.won = already_won;
        s.lost = 0;
        s.timed_out = 0;
        s.log.push(format!(
            "[{label}] resuming at seed {next_seed} (existing winners: {already_won})"
        ));
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .expect("open seed file");

    loop {
        // Stop check + target check.
        let (running, won_count) = {
            let s = status.lock().expect("status lock");
            (s.running, s.won)
        };
        if !running {
            log_line(&status, format!("[{label}] stopped at seed {next_seed}"));
            break;
        }
        if won_count >= TARGET_SEED_COUNT {
            log_line(
                &status,
                format!(
                    "[{label}] target reached ({won_count}/{TARGET_SEED_COUNT}) — sweep complete"
                ),
            );
            break;
        }

        {
            let mut s = status.lock().expect("status lock");
            s.current_seed = next_seed;
        }
        let seed = next_seed;
        next_seed = next_seed.wrapping_add(1);

        let t = std::time::Instant::now();
        let result: SolveOutcome = match variant {
            Variant::Spider => {
                let session = GameSession::new(Spider::four_suit(), seed);
                let budget = spider_solver::SolverBudget::from_duration(
                    SPIDER_PER_SEED_DURATION,
                    SPIDER_PER_SEED_MAX_NODES,
                );
                spider_solver::solve(&session.piles, budget).into()
            }
            Variant::Klondike => {
                let session = GameSession::new(Klondike::with_draw_count(1), seed);
                let budget = klondike_solver::SolverBudget::from_duration(
                    KLONDIKE_PER_SEED_DURATION,
                    KLONDIKE_PER_SEED_MAX_NODES,
                    1,
                );
                solve_klondike(&session.piles, budget)
            }
        };
        let elapsed = t.elapsed();
        match result {
            SolveOutcome::Won => {
                file.write_all(&seed.to_le_bytes()).expect("append seed");
                file.flush().ok();
                let mut s = status.lock().expect("status lock");
                s.won += 1;
                let (won, lost, timed_out) = (s.won, s.lost, s.timed_out);
                s.log.push(format!(
                    "[{label}] seed {seed:>8} WON     in {ms:>5} ms  (won {won}/lost {lost}/timeout {timed_out})",
                    ms = elapsed.as_millis(),
                ));
            }
            SolveOutcome::Exhausted => {
                let mut s = status.lock().expect("status lock");
                s.lost += 1;
                let (won, lost, timed_out) = (s.won, s.lost, s.timed_out);
                s.log.push(format!(
                    "[{label}] seed {seed:>8} unwin   in {ms:>5} ms  (won {won}/lost {lost}/timeout {timed_out})",
                    ms = elapsed.as_millis(),
                ));
            }
            SolveOutcome::Timeout => {
                let mut s = status.lock().expect("status lock");
                s.timed_out += 1;
                let (won, lost, timed_out) = (s.won, s.lost, s.timed_out);
                s.log.push(format!(
                    "[{label}] seed {seed:>8} timeout in {ms:>5} ms  (won {won}/lost {lost}/timeout {timed_out})",
                    ms = elapsed.as_millis(),
                ));
            }
        }
    }

    let mut s = status.lock().expect("status lock");
    s.running = false;
    s.finished = true;
}

#[cfg(not(target_arch = "wasm32"))]
fn log_line(status: &Arc<Mutex<SeedGenStatus>>, line: String) {
    if let Ok(mut s) = status.lock() {
        s.log.push(line);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SolveOutcome {
    Won,
    Exhausted,
    Timeout,
}

#[cfg(not(target_arch = "wasm32"))]
fn solve_klondike(
    piles: &crate::piles::PileSet,
    budget: klondike_solver::SolverBudget,
) -> SolveOutcome {
    match klondike_solver::solve(piles, budget) {
        klondike_solver::SolveResult::Won => SolveOutcome::Won,
        klondike_solver::SolveResult::Exhausted => SolveOutcome::Exhausted,
        klondike_solver::SolveResult::Timeout => SolveOutcome::Timeout,
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<spider_solver::SolveResult> for SolveOutcome {
    fn from(r: spider_solver::SolveResult) -> Self {
        match r {
            spider_solver::SolveResult::Won => SolveOutcome::Won,
            spider_solver::SolveResult::Exhausted => SolveOutcome::Exhausted,
            spider_solver::SolveResult::Timeout => SolveOutcome::Timeout,
        }
    }
}
