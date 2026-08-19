//! 社区笔记本源。
//!
//! 每个源就是 `~/.jot/notebooks/sources/<名字>/` 下的一个 git 克隆。
//! 刻意不引入 HTTP 和 tar 依赖 —— 直接调 `git`，于是私有仓库、SSH 认证、
//! 增量更新、本地改动检测全都免费拿到，而且用户可以自己 git clone 进去。
//!
//! 信任（设计文档 D-09）：外部源的 `from: shell` 变量默认禁用。那是任意
//! 代码执行，光是预览一下就会中招。要开需要显式 `jot trust <名字>`。

use crate::config::Paths;
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
    /// 远端地址。**按需**调用 —— 每次要 spawn 一个 git 子进程（Windows 上约 45ms），
    /// 绝不能出现在加载路径上，那会直接炸穿冷启动预算（D-10）。
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

/// 从 git URL 猜一个目录名：最后一段去掉 .git。
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

/// `gh:user/repo` 这种简写展开成完整 URL。
pub fn expand_url(spec: &str) -> String {
    match spec.split_once(':') {
        Some(("gh" | "github", rest)) => format!("https://github.com/{rest}.git"),
        Some(("gl" | "gitlab", rest)) => format!("https://gitlab.com/{rest}.git"),
        _ => spec.to_string(),
    }
}

fn git(args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(args)
        .output()
        .context("找不到 git —— 社区源功能依赖它")?;
    if !out.status.success() {
        bail!(
            "git {} 失败：{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// 已装的全部源。**只读目录，不碰 git** —— 这个函数在每次加载时都会被调用。
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

/// 克隆一个新源。
pub fn add(paths: &Paths, spec: &str, name_override: Option<&str>) -> Result<Source> {
    let url = expand_url(spec);
    let name = name_override
        .map(String::from)
        .unwrap_or_else(|| name_from_url(&url));
    let dest = paths.sources_dir().join(&name);
    if dest.exists() {
        bail!("«{name}» 已经装过了。要更新用 `jot sync {name}`");
    }
    std::fs::create_dir_all(paths.sources_dir())?;

    git(&["clone", "--depth", "1", &url, &dest.to_string_lossy()])?;

    Ok(Source {
        name,
        path: dest,
        trusted: false,
    })
}

/// 更新一个源。有本地改动就跳过，不覆盖用户的东西。
pub fn sync(source: &Source) -> Result<bool> {
    let dir = source.path.to_string_lossy().to_string();
    let dirty = git(&["-C", &dir, "status", "--porcelain"])?;
    if !dirty.trim().is_empty() {
        bail!("«{}» 有未提交的本地改动，跳过更新", source.name);
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
        bail!("没有叫 «{name}» 的源");
    }
    // 只删 sources/ 底下的东西，防止名字里带 .. 之类
    let canon_root = paths.sources_dir().canonicalize().unwrap_or_default();
    let canon_dest = dest.canonicalize().unwrap_or_default();
    if !canon_dest.starts_with(&canon_root) || canon_dest == canon_root {
        bail!("拒绝删除 {} —— 不在 sources 目录内", dest.display());
    }
    std::fs::remove_dir_all(&dest)?;
    Ok(())
}

/// 一个源里到底去哪找 .md：优先 `notebooks/` 子目录，没有就用仓库根。
pub fn notebook_dir(source_root: &Path) -> PathBuf {
    let nested = source_root.join("notebooks");
    if nested.is_dir() {
        nested
    } else {
        source_root.to_path_buf()
    }
}

/// 仓库根目录下这些文件不是笔记本。
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
        // 不能让名字里出现路径分隔符
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
