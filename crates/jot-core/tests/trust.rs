//! D-09 的安全属性：外部源的 `from: shell` 是任意代码执行，默认必须禁用。
//!
//! 这条比任何功能都重要 —— 产品叙事是「jot 从不执行任何东西」，
//! 如果装一个社区笔记本就能让它跑命令，那句话就是假的。

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
## 一条带动态变量的命令\n\
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
        .expect("外部源的笔记本没被加载")
        .clone()
}

#[test]
fn external_sources_are_untrusted_by_default() {
    let root = setup("default");
    let lib = load(&root);
    assert!(
        !risky_entry(&lib).trusted,
        "外部源的条目默认就是可信的 —— from: shell 会被执行"
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
        .expect("本地笔记本没被加载")
        .clone();
    assert!(mine.trusted, "自己写的笔记本不该被降级为不可信");
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
        "显式授信之后条目仍然不可信"
    );
}

#[test]
fn trusting_a_different_source_does_not_leak() {
    let root = setup("leak");
    let paths = Paths { root: root.clone() };

    let mut cfg = Config::load(&paths);
    cfg.trusted_sources = vec!["某个别的源".to_string()];
    cfg.save(&paths).unwrap();

    assert!(
        !risky_entry(&load(&root)).trusted,
        "授信了 A 源却把 B 源也放行了"
    );
}

#[test]
fn boilerplate_files_are_not_loaded_as_notebooks() {
    let root = setup("boilerplate");
    let src = root.join("notebooks").join("sources").join("community");
    // 一份带 ## 标题和代码块的 README —— 结构上完全能解析成笔记本
    std::fs::write(
        src.join("README.md"),
        "---\nname: readme\n---\n\n## 安装\n\n```sh\ncargo install x\n```\n",
    )
    .unwrap();

    let lib = load(&root);
    assert!(
        !lib.notebooks.iter().any(|n| n.name == "readme"),
        "仓库的 README 被当成笔记本收进来了"
    );
}
