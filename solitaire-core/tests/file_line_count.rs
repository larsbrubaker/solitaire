//! Guardrail for keeping AI-touchable source files small enough to reason about.
//!
//! This deliberately checks more than Rust files. Any text source or checked-in
//! build artifact that an AI assistant may need to inspect or edit should stay
//! under the same limit, including JavaScript/HTML glue such as wasm-bindgen
//! package output. If a file fails this test, split or refactor it instead of
//! hiding that source type from the scan.

use std::fs;
use std::path::{Path, PathBuf};

const MAX_LINES: usize = 800;

// Keep exclusions limited to third-party mirrors, caches, generated output dirs,
// and external test suites. Do not exclude first-party source trees just because
// a file is generated-looking; if it is checked into a source/package directory
// and may be touched by an agent, it should be counted.
//
// Matching semantics (see `is_excluded`): entries with no '/' match ANY directory
// with that name at any depth (e.g. `node_modules` also covers `demo/node_modules`).
// Entries containing '/' are root-relative prefix matches (e.g. `demo/dist`).
const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".github",
    ".claude",
    "target",
    "node_modules",
    "demo/dist",
    "demo/public/pkg",
    "reference",
];

// Machine-generated lockfiles that are not first-party source. `package-lock.json`
// is the npm analog of `Cargo.lock` (which this scan already ignores because the
// "lock" extension isn't checked): it is regenerated on every `npm install` and
// splitting it is meaningless. Matched by file name at any depth. Do NOT add
// first-party JSON here — only genuinely machine-owned output.
const EXCLUDED_FILES: &[&str] = &["package-lock.json"];

// Text formats that are part of the project surface area for humans and AI
// agents. This intentionally includes JS/HTML/CSS/TS, so wasm package glue and
// web demo sources are held to the same size limit as Rust modules.
const CHECKED_EXTENSIONS: &[&str] = &[
    "css", "html", "js", "json", "md", "rs", "toml", "ts", "tsx", "yaml", "yml",
];

#[test]
fn first_party_project_files_stay_under_line_limit() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("solitaire-core crate should live under the workspace root");

    let mut offenders = Vec::new();
    visit_files(workspace_root, workspace_root, &mut offenders);

    if !offenders.is_empty() {
        offenders.sort();
        panic!(
            "project files must stay at or below {MAX_LINES} lines; offenders:\n{}",
            offenders
                .into_iter()
                .map(|(lines, path)| format!("{lines:>5}  {}", path.display()))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

fn visit_files(root: &Path, dir: &Path, offenders: &mut Vec<(usize, PathBuf)>) {
    if is_excluded(root, dir) {
        return;
    }

    let entries = fs::read_dir(dir).unwrap_or_else(|err| {
        panic!("failed to read directory {}: {err}", dir.display());
    });

    for entry in entries {
        let entry = entry.unwrap_or_else(|err| {
            panic!("failed to read directory entry in {}: {err}", dir.display());
        });
        let path = entry.path();
        let file_type = entry.file_type().unwrap_or_else(|err| {
            panic!("failed to read file type for {}: {err}", path.display());
        });

        if file_type.is_dir() {
            visit_files(root, &path, offenders);
        } else if file_type.is_file() && should_check_file(&path) {
            let lines = count_lines(&path);
            if lines > MAX_LINES {
                let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                offenders.push((lines, rel));
            }
        }
    }
}

fn is_excluded(root: &Path, path: &Path) -> bool {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let rel = rel.to_string_lossy().replace('\\', "/");
    EXCLUDED_DIRS.iter().any(|excluded| {
        if excluded.contains('/') {
            // Root-relative prefix match (e.g. "demo/dist").
            rel == *excluded || rel.starts_with(&format!("{excluded}/"))
        } else {
            // Bare directory name: match any path component at any depth.
            rel.split('/').any(|component| component == *excluded)
        }
    })
}

fn should_check_file(path: &Path) -> bool {
    if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
        if EXCLUDED_FILES.contains(&name) {
            return false;
        }
    }
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            CHECKED_EXTENSIONS
                .iter()
                .any(|checked| ext.eq_ignore_ascii_case(checked))
        })
        .unwrap_or(false)
}

fn count_lines(path: &Path) -> usize {
    let text = fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("failed to read {} as UTF-8 text: {err}", path.display());
    });
    text.lines().count()
}
