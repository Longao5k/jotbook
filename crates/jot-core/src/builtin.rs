//! 随二进制发布的内置笔记本。
//!
//! 首次运行时落地到 `~/.jot/notebooks/builtin/`。落地之后它们就是普通文件，
//! 用户改了就是改了（文件是唯一真相，见设计文档 D-07）。升级二进制时只有
//! BUILTIN_VERSION 变了才会重写这个目录，`local/` 永远不碰。

use crate::config::{Config, Paths};
use anyhow::Result;

/// 内置笔记本内容有变就要改这个版本号，否则老用户不会拿到新内容。
pub const BUILTIN_VERSION: &str = "0.2.0";

/// 加一本笔记本 = 在下面加一行文件名。
macro_rules! notebooks {
    ($($name:literal),* $(,)?) => {
        &[$( ($name, include_str!(concat!("../../../notebooks/", $name))) ),*]
    };
}

pub const BUILTIN: &[(&str, &str)] = notebooks![
    "jot.md",
    // 通用
    "git.md",
    "linux.md",
    "macos.md",
    "powershell.md",
    "ssh.md",
    "tmux.md",
    // 运行时与包管理
    "docker.md",
    "kubectl.md",
    "nginx.md",
    "systemd.md",
    // 语言与框架
    "dotnet.md",
    "flutter.md",
    "npm.md",
    "python.md",
    // 数据库
    "mssql.md",
    "mysql.md",
    "postgres.md",
    "redis.md",
];

/// 需要时把内置笔记本写到磁盘。返回写了几个。
pub fn seed_if_missing(paths: &Paths) -> Result<usize> {
    let cfg = Config::load(paths);
    let dir = paths.builtin_dir();
    let up_to_date = cfg.builtin_version.as_deref() == Some(BUILTIN_VERSION);
    if up_to_date && dir.join("git.md").exists() {
        return Ok(0);
    }

    std::fs::create_dir_all(&dir)?;
    let mut written = 0;
    for (name, content) in BUILTIN {
        let target = dir.join(name);
        // 内容没变就不动文件，避免打乱 mtime 和用户的 git diff
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
    cfg.save(paths)?;
    Ok(written)
}

/// 首次运行时给用户建一个空的个人笔记本，让 `jot save` 有地方落。
pub fn ensure_personal_notebook(paths: &Paths) -> Result<std::path::PathBuf> {
    let path = paths.local_dir().join("my.md");
    if !path.exists() {
        std::fs::create_dir_all(paths.local_dir())?;
        std::fs::write(
            &path,
            "---\nname: my\ndescription: 我自己的命令\ntags: [personal]\n---\n\n",
        )?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn every_builtin_notebook_parses() {
        for (name, content) in BUILTIN {
            let nb = crate::notebook::parse(Path::new(name), content)
                .unwrap_or_else(|e| panic!("{name} 解析失败: {e}"));
            assert!(!nb.entries.is_empty(), "{name} 一条命令都没解析出来");
        }
    }

    #[test]
    fn builtins_have_a_useful_amount_of_content() {
        let total: usize = BUILTIN
            .iter()
            .map(|(n, c)| {
                crate::notebook::parse(Path::new(n), c)
                    .unwrap()
                    .entries
                    .len()
            })
            .sum();
        assert!(total > 600, "内置命令只有 {total} 条，太少了");
    }

    #[test]
    fn every_entry_has_a_title_and_command() {
        for (name, content) in BUILTIN {
            let nb = crate::notebook::parse(Path::new(name), content).unwrap();
            for e in &nb.entries {
                assert!(!e.title.trim().is_empty(), "{name} 有条目没标题");
                assert!(
                    !e.command.trim().is_empty(),
                    "{name} 的「{}」没有命令",
                    e.title
                );
            }
        }
    }

    #[test]
    fn declared_vars_are_actually_used() {
        // 声明了却没人用的变量通常是笔误
        for (name, content) in BUILTIN {
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
                    "{name} 声明了变量 {key} 但没有任何条目用到"
                );
            }
        }
    }

    #[test]
    fn platform_attributes_are_spelled_correctly() {
        for (name, content) in BUILTIN {
            let nb = crate::notebook::parse(Path::new(name), content).unwrap();
            for e in &nb.entries {
                for p in &e.platforms {
                    assert!(
                        matches!(p.as_str(), "windows" | "linux" | "macos" | "any"),
                        "{name} 的「{}」写了未知平台 {p}",
                        e.title
                    );
                }
            }
        }
    }
}
