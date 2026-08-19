//! Jotbook 核心：解析笔记本、求值变量、管理 Profile。
//!
//! 这个 crate 不做任何终端交互，两个前端（CLI / GUI）共享它。

pub mod builtin;
pub mod capture;
pub mod config;
pub mod notebook;
pub mod resolve;
pub mod vars;

pub use config::{Config, Paths, Profiles};
pub use notebook::{Entry, Notebook, VarDecl};
pub use vars::{Seg, VarRef};

use anyhow::Result;
use std::time::Instant;

/// 全部笔记本 + 一次加载的耗时统计。
pub struct Library {
    pub notebooks: Vec<Notebook>,
    pub load_ms: f64,
}

impl Library {
    /// 从数据目录加载全部笔记本。首次运行会自动落地内置笔记本。
    pub fn load(paths: &Paths) -> Result<Library> {
        // 落地内置笔记本只在首次运行发生，不该算进稳态加载耗时
        paths.ensure()?;
        builtin::seed_if_missing(paths)?;

        let t0 = Instant::now();
        let mut notebooks = Vec::new();
        for dir in paths.notebook_dirs() {
            if !dir.is_dir() {
                continue;
            }
            let mut files: Vec<_> = std::fs::read_dir(&dir)?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map(|e| e == "md").unwrap_or(false))
                .collect();
            files.sort();
            for path in files {
                let text = match std::fs::read_to_string(&path) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                match notebook::parse(&path, &text) {
                    Ok(nb) => notebooks.push(nb),
                    // 单个笔记本写坏了不应该让整个工具起不来
                    Err(e) => eprintln!("jot: 跳过 {}: {e}", path.display()),
                }
            }
        }

        Ok(Library {
            notebooks,
            load_ms: t0.elapsed().as_secs_f64() * 1000.0,
        })
    }

    /// 当前平台可见的全部条目。
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

    /// 找到条目所属笔记本的变量声明表。
    ///
    /// 按文件路径匹配而不是按 name —— `builtin/git.md` 和 `local/git.md` 可以同名，
    /// 按 name 找会拿到错误那本的变量声明。
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

    /// 两本同名笔记本（builtin 和 local 各一份）必须各用各的变量声明。
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
            "拿到了同名另一本笔记本的变量声明"
        );
    }
}
