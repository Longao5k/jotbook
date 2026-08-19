//! 冷启动预算的回归守卫（设计文档 D-10：加载路径 50ms 硬指标）。
//!
//! 这条不是假想的风险：社区源功能刚上线时，`sources::list()` 会对每个源
//! spawn 一次 `git remote get-url`。那个函数每次加载都会跑，Windows 上一次
//! spawn 约 45ms —— 装 3 个源就把加载从 3ms 拖到 139ms。widget 是每次按键
//! 重启进程的，这会直接毁掉体验。
//!
//! 阈值取得很宽松，只为抓住数量级的回归，不会因为 CI 机器慢而误报。

use jot_core::{Library, Paths};
use std::path::PathBuf;
use std::time::Instant;

const SOURCES: usize = 8;
/// 宽松到不会误报，又足以抓住「每个源一次子进程」这类回归。
/// 参考：正常约 5–10ms；每源一次 git spawn 约 45ms × 8 ≈ 360ms。
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
            format!("---\nname: n{i}\n---\n\n## 条目 {i}\n\n```sh\necho {i}\n```\n"),
        )
        .unwrap();
    }
    root
}

#[test]
fn loading_stays_within_the_cold_start_budget() {
    let root = setup();
    let paths = Paths { root };

    // 第一次会落地内置笔记本，不计入
    let warm = Library::load(&paths).unwrap();
    assert!(
        warm.notebooks.len() > SOURCES,
        "内置笔记本没被加载，这个测试没意义"
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
        "{SOURCES} 个外部源没被全部加载"
    );
    assert!(
        elapsed < BUDGET_MS,
        "装了 {SOURCES} 个源时加载耗时 {elapsed:.0}ms，超过 {BUDGET_MS:.0}ms 预算。\n\
         最常见的原因是加载路径上出现了子进程调用（见 D-10）。",
    );
}
