//! Regression guard for the cold-start budget (D-10: a 50ms hard target).
//!
//! Not a hypothetical: when community sources first landed, `sources::list()`
//! ran `git remote get-url` once per source. That function runs on every load,
//! and a spawn costs ~45ms on Windows, so three sources took it from 3ms to
//! 139ms. The widget restarts the process on every keypress, so that hurts.
//!
//! The threshold is loose on purpose: it catches order-of-magnitude
//! regressions without flaking on slow CI machines.

use jot_core::{Library, Paths};
use std::path::PathBuf;
use std::time::Instant;

const SOURCES: usize = 8;
/// Loose enough not to flake, tight enough to catch one-spawn-per-source.
/// For reference: normal is 5-10ms; a git spawn per source is ~45ms x 8.
const BUDGET_MS: f64 = 150.0;

fn setup() -> PathBuf {
    let root = std::env::temp_dir().join(format!("jot-budget-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for i in 0..SOURCES {
        let dir = root
            .join("notebooks")
            .join("sources")
            .join(format!("src{i}"))
            .join("notebooks");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("n.md"),
            format!("---\nname: n{i}\n---\n\n## Entry {i}\n\n```sh\necho {i}\n```\n"),
        )
        .unwrap();
    }
    root
}

#[test]
fn loading_stays_within_the_cold_start_budget() {
    let root = setup();
    let paths = Paths { root };

    // The first call seeds the built-in notebooks, which is not measured
    let warm = Library::load(&paths).unwrap();
    assert!(
        warm.notebooks.len() > SOURCES,
        "the built-in notebooks were not loaded, so this test proves nothing"
    );

    let t0 = Instant::now();
    let lib = Library::load(&paths).unwrap();
    let elapsed = t0.elapsed().as_secs_f64() * 1000.0;

    assert!(
        lib.notebooks
            .iter()
            .filter(|n| n.name.starts_with("n"))
            .count()
            >= SOURCES,
        "not all {SOURCES} external sources were loaded"
    );
    assert!(
        elapsed < BUDGET_MS,
        "loading with {SOURCES} sources took {elapsed:.0}ms, over the {BUDGET_MS:.0}ms budget.\n\
         The usual cause is a subprocess call on the load path (see D-10).",
    );
}
