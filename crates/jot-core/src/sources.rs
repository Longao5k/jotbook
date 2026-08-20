//! Community notebook sources.
//!
//! A source is a git clone under `~/.jot/notebooks/sources/<name>/`. No HTTP
//! or tar dependency on purpose: shelling out to `git` gets private repos, SSH
//! auth, incremental updates and dirty-tree detection for free, and users can
//!
//! clone into the directory themselves. Trust (D-09): `from: shell` is disabled
//! for external sources - it is arbitrary code execution - until `jot trust`.

use crate::config::Paths;
use crate::t;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Source {
    pub name: String,
    pub path: PathBuf,
    pub trusted: bool,
}

impl Source {
    /// The remote URL. Call it **on demand**: it spawns a git subprocess (~45ms on
    /// Windows) and must never appear on the load path, which would blow D-10.
    pub fn remote_url(&self) -> Option<String> {
        git(&[
            "-C",
            &self.path.to_string_lossy(),
            "remote",
            "get-url",
            "origin",
        ])
        .ok()
        .map(|s| s.trim().to_string())
    }
}

/// Derive a directory name from a git URL: the last segment, minus .git.
pub fn name_from_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/').trim_end_matches(".git");
    let last = trimmed
        .rsplit(['/', ':', '\\'])
        .next()
        .unwrap_or("source")
        .trim();
    let cleaned: String = last
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches(['-', '.']).to_string();
    if cleaned.is_empty() {
        "source".to_string()
    } else {
        cleaned
    }
}

/// Expand the `gh:user/repo` shorthand into a full URL.
pub fn expand_url(spec: &str) -> String {
    match spec.split_once(':') {
        Some(("gh" | "github", rest)) => format!("https://github.com/{rest}.git"),
        Some(("gl" | "gitlab", rest)) => format!("https://gitlab.com/{rest}.git"),
        _ => spec.to_string(),
    }
}

fn git(args: &[&str]) -> Result<String> {
    let out = Command::new("git").args(args).output().context(t!(
        "找不到 git —— 社区源功能依赖它",
        "git not found - community sources depend on it"
    ))?;
    if !out.status.success() {
        bail!(
            "{}",
            t!(
                "git {} 失败：{}",
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            )
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Every installed source. **Reads directories only, never touches git** - this runs on every load.
pub fn list(paths: &Paths, trusted: &[String]) -> Vec<Source> {
    let dir = paths.sources_dir();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<Source> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .map(|path| {
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let trusted = trusted.contains(&name);
            Source {
                name,
                path,
                trusted,
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Clone a new source.
pub fn add(paths: &Paths, spec: &str, name_override: Option<&str>) -> Result<Source> {
    let url = expand_url(spec);
    let name = name_override
        .map(String::from)
        .unwrap_or_else(|| name_from_url(&url));
    let dest = paths.sources_dir().join(&name);
    if dest.exists() {
        bail!(
            "{}",
            t!(
                "«{name}» 已经装过了。要更新用 `jot sync {name}`",
                "«{name}» is already installed. Update it with `jot sync {name}`"
            )
        );
    }
    std::fs::create_dir_all(paths.sources_dir())?;

    git(&["clone", "--depth", "1", &url, &dest.to_string_lossy()])?;

    Ok(Source {
        name,
        path: dest,
        trusted: false,
    })
}

/// Update a source. Skipped when the clone is dirty, so local edits survive.
pub fn sync(source: &Source) -> Result<bool> {
    let dir = source.path.to_string_lossy().to_string();
    let dirty = git(&["-C", &dir, "status", "--porcelain"])?;
    if !dirty.trim().is_empty() {
        bail!(
            "{}",
            t!(
                "«{}» 有未提交的本地改动，跳过更新",
                "«{}» has uncommitted local changes, so it was skipped",
                source.name
            )
        );
    }
    let before = git(&["-C", &dir, "rev-parse", "HEAD"])?.trim().to_string();
    git(&["-C", &dir, "fetch", "--depth", "1", "origin"])?;
    git(&["-C", &dir, "reset", "--hard", "FETCH_HEAD"])?;
    let after = git(&["-C", &dir, "rev-parse", "HEAD"])?.trim().to_string();
    Ok(before != after)
}

pub fn remove(paths: &Paths, name: &str) -> Result<()> {
    let dest = paths.sources_dir().join(name);
    if !dest.is_dir() {
        bail!(
            "{}",
            t!("没有叫 «{name}» 的源", "no source called «{name}»")
        );
    }
    // Only ever delete inside sources/, in case a name contains ..
    let canon_root = paths.sources_dir().canonicalize().unwrap_or_default();
    let canon_dest = dest.canonicalize().unwrap_or_default();
    if !canon_dest.starts_with(&canon_root) || canon_dest == canon_root {
        bail!(
            "{}",
            t!(
                "拒绝删除 {} —— 不在 sources 目录内",
                "refusing to delete {} - it is not inside the sources directory",
                dest.display()
            )
        );
    }
    std::fs::remove_dir_all(&dest)?;
    Ok(())
}

/// Where to look for .md inside a source: `notebooks/` if present, else the repo root.
pub fn notebook_dir(source_root: &Path) -> PathBuf {
    let nested = source_root.join("notebooks");
    if nested.is_dir() {
        nested
    } else {
        source_root.to_path_buf()
    }
}

/// Files at a repository root that are not notebooks.
pub fn is_boilerplate(path: &Path) -> bool {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_ascii_uppercase())
        .unwrap_or_default();
    stem.starts_with("README")
        || stem.starts_with("CONTRIBUTING")
        || stem.starts_with("CHANGELOG")
        || stem.starts_with("LICENSE")
        || stem.starts_with("CODE_OF_CONDUCT")
        || stem.starts_with("SECURITY")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorthand_expands() {
        assert_eq!(
            expand_url("gh:someone/notebooks"),
            "https://github.com/someone/notebooks.git"
        );
        assert_eq!(
            expand_url("gl:someone/notebooks"),
            "https://gitlab.com/someone/notebooks.git"
        );
    }

    #[test]
    fn full_urls_pass_through() {
        for u in [
            "https://github.com/a/b.git",
            "git@github.com:a/b.git",
            "/local/path/repo",
        ] {
            assert_eq!(expand_url(u), u);
        }
    }

    #[test]
    fn names_are_derived_and_sanitised() {
        assert_eq!(
            name_from_url("https://github.com/a/my-notebooks.git"),
            "my-notebooks"
        );
        assert_eq!(name_from_url("git@github.com:a/b.git"), "b");
        assert_eq!(name_from_url("https://example.com/x/y/"), "y");
        // A name must never contain a path separator
        assert!(!name_from_url("https://e.com/a/../../etc").contains('/'));
    }

    #[test]
    fn boilerplate_is_recognised() {
        assert!(is_boilerplate(Path::new("README.md")));
        assert!(is_boilerplate(Path::new("README.zh-CN.md")));
        assert!(is_boilerplate(Path::new("LICENSE.md")));
        assert!(!is_boilerplate(Path::new("docker.md")));
    }
}
