//! Jotbook core: parsing notebooks, resolving variables, managing profiles.
//!
//! No terminal interaction lives here; both front ends share this crate.

pub mod builtin;
pub mod capture;
pub mod config;
pub mod i18n;
pub mod notebook;
pub mod resolve;
pub mod sources;
pub mod usage;
pub mod vars;

pub use config::{Config, Paths, Profiles};
pub use i18n::Lang;
pub use notebook::{Entry, Notebook, VarDecl};
pub use sources::Source;
pub use usage::Usage;
pub use vars::{Seg, VarRef};

use anyhow::Result;
use std::time::Instant;

/// Read the .md files in one directory. One broken file must not stop the tool starting.
fn load_dir(dir: &std::path::Path, trusted: bool, skip_boilerplate: bool, out: &mut Vec<Notebook>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<_> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "md").unwrap_or(false))
        .filter(|p| !(skip_boilerplate && sources::is_boilerplate(p)))
        .collect();
    files.sort();
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        match notebook::parse(&path, &text) {
            Ok(mut nb) => {
                if !trusted {
                    for e in &mut nb.entries {
                        e.trusted = false;
                    }
                }
                // Ordinary markdown in an external source often has no commands at all
                if !nb.entries.is_empty() {
                    out.push(nb);
                }
            }
            Err(e) => eprintln!(
                "{}",
                t!("jot: 跳过 {}: {e}", "jot: skipping {}: {e}", path.display())
            ),
        }
    }
}

/// Every notebook, plus how long one load took.
pub struct Library {
    pub notebooks: Vec<Notebook>,
    pub load_ms: f64,
}

impl Library {
    /// Load every notebook. The first run seeds the built-in ones.
    pub fn load(paths: &Paths) -> Result<Library> {
        // Seeding only happens on the first run and must not count towards steady-state load time
        paths.ensure()?;
        builtin::seed_if_missing(paths)?;

        let t0 = Instant::now();
        let trusted = Config::load(paths).trusted_sources;
        let mut notebooks = Vec::new();

        // Shipped and personal notebooks: fully trusted
        for dir in paths.notebook_dirs() {
            load_dir(&dir, true, false, &mut notebooks);
        }
        // External sources: untrusted, so from: shell is disabled (D-09)
        for src in sources::list(paths, &trusted) {
            let dir = sources::notebook_dir(&src.path);
            load_dir(&dir, src.trusted, true, &mut notebooks);
        }

        Ok(Library {
            notebooks,
            load_ms: t0.elapsed().as_secs_f64() * 1000.0,
        })
    }

    /// Every entry visible on the current platform.
    pub fn entries(&self) -> Vec<&Entry> {
        let plat = notebook::current_platform();
        self.notebooks
            .iter()
            .flat_map(|n| n.entries.iter())
            .filter(|e| e.visible_on(plat))
            .collect()
    }

    pub fn entry_count(&self) -> usize {
        self.entries().len()
    }

    /// The variable declarations of the notebook an entry belongs to.
    ///
    /// Matched by path rather than name: `builtin/git.md` and `local/git.md` may
    /// share a name, and matching by name would return the wrong declarations.
    pub fn decls_for(&self, entry: &Entry) -> &std::collections::BTreeMap<String, VarDecl> {
        static EMPTY: std::sync::OnceLock<std::collections::BTreeMap<String, VarDecl>> =
            std::sync::OnceLock::new();
        self.notebooks
            .iter()
            .find(|n| n.path == entry.source)
            .map(|n| &n.vars)
            .unwrap_or_else(|| EMPTY.get_or_init(Default::default))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Two same-named notebooks must keep their own variable declarations.
    #[test]
    fn same_named_notebooks_keep_their_own_vars() {
        let a = notebook::parse(
            Path::new("/builtin/git.md"),
            "---\nname: git\nvars:\n  x:\n    from: profile\n---\n\n## a\n\n```sh\necho {{x}}\n```\n",
        )
        .unwrap();
        let b = notebook::parse(
            Path::new("/local/git.md"),
            "---\nname: git\nvars:\n  x:\n    from: shell\n    cmd: echo hi\n---\n\n## b\n\n```sh\necho {{x}}\n```\n",
        )
        .unwrap();

        let entry_b = b.entries[0].clone();
        let lib = Library {
            notebooks: vec![a, b],
            load_ms: 0.0,
        };
        assert_eq!(
            lib.decls_for(&entry_b)["x"].source(),
            "shell",
            "picked up the variable declarations of a same-named notebook"
        );
    }
}
