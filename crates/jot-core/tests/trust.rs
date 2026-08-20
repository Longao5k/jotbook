//! The D-09 security property: a source's `from: shell` is arbitrary code
//!
//! execution, so it must be disabled by default. This matters more than any
//! feature: the product claim is that jot never executes anything, and a
//! community notebook being able to run commands would make that false.

use jot_core::{Config, Library, Paths};
use std::path::{Path, PathBuf};

const NOTEBOOK: &str = "---\n\
name: risky\n\
vars:\n\
\x20 target:\n\
\x20   from: shell\n\
\x20   cmd: echo pwned\n\
---\n\
\n\
## A command with a dynamic variable\n\
\n\
```sh\n\
echo {{target}}\n\
```\n";

fn setup(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("jot-trust-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let src = root.join("notebooks").join("sources").join("community");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("risky.md"), NOTEBOOK).unwrap();
    root
}

fn load(root: &Path) -> Library {
    Library::load(&Paths {
        root: root.to_path_buf(),
    })
    .unwrap()
}

fn risky_entry(lib: &Library) -> jot_core::Entry {
    lib.entries()
        .into_iter()
        .find(|e| e.notebook == "risky")
        .expect("the external source's notebook was not loaded")
        .clone()
}

#[test]
fn external_sources_are_untrusted_by_default() {
    let root = setup("default");
    let lib = load(&root);
    assert!(
        !risky_entry(&lib).trusted,
        "external entries are trusted by default, so from: shell would run"
    );
}

#[test]
fn builtin_and_local_notebooks_stay_trusted() {
    let root = setup("local");
    let local = root.join("notebooks").join("local");
    std::fs::create_dir_all(&local).unwrap();
    std::fs::write(local.join("mine.md"), NOTEBOOK.replace("risky", "mine")).unwrap();

    let lib = load(&root);
    let mine = lib
        .entries()
        .into_iter()
        .find(|e| e.notebook == "mine")
        .expect("the local notebook was not loaded")
        .clone();
    assert!(
        mine.trusted,
        "a notebook you wrote yourself must not be downgraded to untrusted"
    );
}

#[test]
fn explicit_trust_promotes_the_source() {
    let root = setup("trust");
    let paths = Paths { root: root.clone() };

    let mut cfg = Config::load(&paths);
    cfg.trusted_sources = vec!["community".to_string()];
    cfg.save(&paths).unwrap();

    assert!(
        risky_entry(&load(&root)).trusted,
        "the entry is still untrusted after being trusted explicitly"
    );
}

#[test]
fn trusting_a_different_source_does_not_leak() {
    let root = setup("leak");
    let paths = Paths { root: root.clone() };

    let mut cfg = Config::load(&paths);
    cfg.trusted_sources = vec!["some other source".to_string()];
    cfg.save(&paths).unwrap();

    assert!(
        !risky_entry(&load(&root)).trusted,
        "trusting source A also let source B through"
    );
}

#[test]
fn boilerplate_files_are_not_loaded_as_notebooks() {
    let root = setup("boilerplate");
    let src = root.join("notebooks").join("sources").join("community");
    // A README with a heading and a code block, which parses fine structurally
    std::fs::write(
        src.join("README.md"),
        "---\nname: readme\n---\n\n## Install\n\n```sh\ncargo install x\n```\n",
    )
    .unwrap();

    let lib = load(&root);
    assert!(
        !lib.notebooks.iter().any(|n| n.name == "readme"),
        "the repository README was picked up as a notebook"
    );
}
