//! Notebooks shipped inside the binary, in both languages.
//!
//! On first run the set matching the active language is written to
//! `~/.jot/notebooks/builtin/`. After that they are ordinary files — if the
//! user edits one, it stays edited (files are the single source of truth,
//! see D-07). The directory is only rewritten when BUILTIN_VERSION changes
//! or the language is switched. `local/` is never touched.

use crate::config::{Config, Paths};
use crate::i18n::Lang;
use anyhow::Result;

/// Bump this whenever built-in notebook content changes, or existing
/// installs will never see the new content.
pub const BUILTIN_VERSION: &str = "0.3.0";

/// Adding a notebook is one line per language.
macro_rules! notebooks {
    ($dir:literal, $($name:literal),* $(,)?) => {
        &[$( ($name, include_str!(concat!("../../../notebooks/", $dir, "/", $name))) ),*]
    };
}

macro_rules! notebook_set {
    ($dir:literal) => {
        notebooks![
            $dir,
            "jot.md",
            // general
            "git.md",
            "linux.md",
            "macos.md",
            "powershell.md",
            "ssh.md",
            "tmux.md",
            // runtimes and package managers
            "docker.md",
            "kubectl.md",
            "nginx.md",
            "systemd.md",
            // languages and frameworks
            "dotnet.md",
            "flutter.md",
            "npm.md",
            "python.md",
            // databases
            "mssql.md",
            "mysql.md",
            "postgres.md",
            "redis.md",
        ]
    };
}

pub const BUILTIN_EN: &[(&str, &str)] = notebook_set!("en");
pub const BUILTIN_ZH: &[(&str, &str)] = notebook_set!("zh");

pub fn builtin_for(lang: Lang) -> &'static [(&'static str, &'static str)] {
    match lang {
        Lang::En => BUILTIN_EN,
        Lang::Zh => BUILTIN_ZH,
    }
}

/// Write the built-in notebooks to disk when needed. Returns how many changed.
///
/// Switching language replaces the directory wholesale: the other language's
/// files are removed first, so the user does not end up with both sets.
pub fn seed_if_missing(paths: &Paths) -> Result<usize> {
    let cfg = Config::load(paths);
    let lang = crate::i18n::lang();
    let dir = paths.builtin_dir();

    let version_ok = cfg.builtin_version.as_deref() == Some(BUILTIN_VERSION);
    let lang_ok = cfg.builtin_lang.as_deref() == Some(lang.code());
    if version_ok && lang_ok && dir.join("git.md").exists() {
        return Ok(0);
    }

    let set = builtin_for(lang);
    std::fs::create_dir_all(&dir)?;

    // A language switch must not leave the previous language's files behind
    if !lang_ok {
        let keep: Vec<&str> = set.iter().map(|(n, _)| *n).collect();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for path in rd.filter_map(|e| e.ok()).map(|e| e.path()) {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string());
                let is_md = path.extension().map(|e| e == "md").unwrap_or(false);
                if is_md && !name.map(|n| keep.contains(&n.as_str())).unwrap_or(false) {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }

    let mut written = 0;
    for (name, content) in set {
        let target = dir.join(name);
        // Leave files whose content already matches, so mtimes and the
        // user's own git diff stay quiet
        if let Ok(existing) = std::fs::read_to_string(&target) {
            if existing == *content {
                continue;
            }
        }
        std::fs::write(&target, content)?;
        written += 1;
    }

    let mut cfg = cfg;
    cfg.builtin_version = Some(BUILTIN_VERSION.to_string());
    cfg.builtin_lang = Some(lang.code().to_string());
    cfg.save(paths)?;
    Ok(written)
}

/// Give `jot save` somewhere to land on first use.
pub fn ensure_personal_notebook(paths: &Paths) -> Result<std::path::PathBuf> {
    let path = paths.local_dir().join("my.md");
    if !path.exists() {
        std::fs::create_dir_all(paths.local_dir())?;
        let body = crate::t!(
            "---\nname: my\ndescription: 我自己的命令\ntags: [personal]\n---\n\n",
            "---\nname: my\ndescription: My own commands\ntags: [personal]\n---\n\n"
        );
        std::fs::write(&path, body.as_ref())?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn all_sets() -> [(&'static str, &'static [(&'static str, &'static str)]); 2] {
        [("en", BUILTIN_EN), ("zh", BUILTIN_ZH)]
    }

    fn is_cjk(c: char) -> bool {
        ('\u{4e00}'..='\u{9fff}').contains(&c)
    }

    #[test]
    fn every_builtin_notebook_parses() {
        for (lang, set) in all_sets() {
            for (name, content) in set {
                let nb = crate::notebook::parse(Path::new(name), content)
                    .unwrap_or_else(|e| panic!("{lang}/{name} failed to parse: {e}"));
                assert!(!nb.entries.is_empty(), "{lang}/{name} produced no entries");
            }
        }
    }

    #[test]
    fn builtins_have_a_useful_amount_of_content() {
        for (lang, set) in all_sets() {
            let total: usize = set
                .iter()
                .map(|(n, c)| {
                    crate::notebook::parse(Path::new(n), c)
                        .unwrap()
                        .entries
                        .len()
                })
                .sum();
            assert!(total > 600, "{lang} only has {total} commands, too few");
        }
    }

    /// The two languages must stay structurally identical: same notebooks,
    /// same number of entries, same commands. Only prose differs.
    #[test]
    fn the_two_languages_stay_in_sync() {
        assert_eq!(BUILTIN_EN.len(), BUILTIN_ZH.len(), "notebook count differs");

        for ((en_name, en_src), (zh_name, zh_src)) in BUILTIN_EN.iter().zip(BUILTIN_ZH) {
            assert_eq!(en_name, zh_name, "notebook order differs");
            let en = crate::notebook::parse(Path::new(en_name), en_src).unwrap();
            let zh = crate::notebook::parse(Path::new(zh_name), zh_src).unwrap();

            assert_eq!(
                en.entries.len(),
                zh.entries.len(),
                "{en_name}: en has {} entries, zh has {}",
                en.entries.len(),
                zh.entries.len()
            );

            for (e, z) in en.entries.iter().zip(&zh.entries) {
                // Reference cheatsheets and commands carrying inline comments
                // are prose and do get translated; everything else must match
                // byte for byte, or the two languages have silently drifted.
                let zh_is_prose = z.command.chars().any(is_cjk);
                if !zh_is_prose {
                    assert_eq!(
                        e.command, z.command,
                        "{en_name}: the command itself must be identical across languages"
                    );
                }
                assert_eq!(
                    e.confirm, z.confirm,
                    "{en_name}/{}: @confirm differs",
                    e.title
                );
                assert_eq!(
                    e.platforms, z.platforms,
                    "{en_name}/{}: @platform differs",
                    e.title
                );
            }
        }
    }

    #[test]
    fn english_notebooks_contain_no_chinese() {
        for (name, content) in BUILTIN_EN {
            if let Some(line) = content.lines().find(|l| l.chars().any(is_cjk)) {
                panic!("en/{name} still contains Chinese: {line}");
            }
        }
    }

    #[test]
    fn every_entry_has_a_title_and_command() {
        for (lang, set) in all_sets() {
            for (name, content) in set {
                let nb = crate::notebook::parse(Path::new(name), content).unwrap();
                for e in &nb.entries {
                    assert!(
                        !e.title.trim().is_empty(),
                        "{lang}/{name} has an untitled entry"
                    );
                    assert!(
                        !e.command.trim().is_empty(),
                        "{lang}/{name}: \"{}\" has no command",
                        e.title
                    );
                }
            }
        }
    }

    #[test]
    fn declared_vars_are_actually_used() {
        // A variable declared but never referenced is almost always a typo
        for (lang, set) in all_sets() {
            for (name, content) in set {
                let nb = crate::notebook::parse(Path::new(name), content).unwrap();
                let used: std::collections::HashSet<String> = nb
                    .entries
                    .iter()
                    .flat_map(|e| crate::vars::refs(&e.command))
                    .map(|v| v.name)
                    .collect();
                for key in nb.vars.keys() {
                    assert!(
                        used.contains(key),
                        "{lang}/{name} declares {key} but no entry uses it"
                    );
                }
            }
        }
    }

    #[test]
    fn platform_attributes_are_spelled_correctly() {
        for (lang, set) in all_sets() {
            for (name, content) in set {
                let nb = crate::notebook::parse(Path::new(name), content).unwrap();
                for e in &nb.entries {
                    for p in &e.platforms {
                        assert!(
                            matches!(p.as_str(), "windows" | "linux" | "macos" | "any"),
                            "{lang}/{name}: \"{}\" has unknown platform {p}",
                            e.title
                        );
                    }
                }
            }
        }
    }
}
