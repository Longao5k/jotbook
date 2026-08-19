//! Jotbook 核心：解析笔记本、求值变量、管理 Profile。
//!
//! 这个 crate 不做任何终端交互，两个前端（CLI / GUI）共享它。

pub mod builtin;
pub mod capture;
pub mod config;
pub mod notebook;
pub mod resolve;
pub mod sources;
pub mod usage;
pub mod vars;

pub use config::{Config, Paths, Profiles};
pub use notebook::{Entry, Notebook, VarDecl};
pub use sources::Source;
pub use usage::Usage;
pub use vars::{Seg, VarRef};

use anyhow::Result;
use std::time::Instant;

/// 读一个目录里的 .md。单个文件写坏不应该让整个工具起不来。
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
                // 外部源里的普通 markdown 常常一条命令都没有，不要污染列表
                if !nb.entries.is_empty() {
                    out.push(nb);
                }
            }
            Err(e) => eprintln!("jot: 跳过 {}: {e}", path.display()),
        }
    }
}

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
        let trusted = Config::load(paths).trusted_sources;
        let mut notebooks = Vec::new();

        // 自带的和用户自己的：完全信任
        for dir in paths.notebook_dirs() {
            load_dir(&dir, true, false, &mut notebooks);
        }
        // 外部源：默认不信任，from: shell 会被禁用（D-09）
        for src in sources::list(paths, &trusted) {
            let dir = sources::notebook_dir(&src.path);
            load_dir(&dir, src.trusted, true, &mut notebooks);
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
