//! 变量求值：内置变量、Profile 取值、跑命令生成候选列表。
//!
//! 求值顺序（见设计文档 D-03）：
//!   1. `@` 开头 → 内置变量，直接算出来，不打断用户
//!   2. `from: profile` 且当前 Profile 有这个键 → 直接用，不打断用户
//!   3. 有 `cmd` → 跑命令拿候选列表，让用户选
//!   4. 有 `options` → 固定候选，让用户选
//!   5. 其他 → 自由输入
//!
//! 任何一步失败都降级到下一步，绝不让工具卡死或报错退出。

use crate::config::Profiles;
use crate::notebook::VarDecl;
use std::process::Command;
use std::time::Duration;

/// 一个变量该怎么问用户。
#[derive(Debug, Clone)]
pub enum Ask {
    /// 已经有值了，不用问
    Resolved(String),
    /// 从候选列表里选，也允许自由输入
    Choose {
        label: String,
        options: Vec<Choice>,
        default: Option<String>,
    },
    /// 纯自由输入
    Text {
        label: String,
        default: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct Choice {
    /// 真正代入命令的值
    pub value: String,
    /// 列表里显示的完整行（可能带状态等附加信息）
    pub display: String,
}

/// 跑命令拿候选列表。失败返回空，调用方会自动降级为自由输入。
pub fn shell_candidates(cmd: &str) -> Vec<Choice> {
    let out = run_capture(cmd, Duration::from_secs(5));
    let Some(text) = out else { return Vec::new() };
    let mut seen = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r').trim();
        if line.is_empty() {
            continue;
        }
        // 第一个字段是值，整行用来显示（docker ps --format "{{.Names}}\t{{.Status}}"）
        let value = line.split('\t').next().unwrap_or(line).trim().to_string();
        if value.is_empty() {
            continue;
        }
        if seen.iter().any(|c: &Choice| c.value == value) {
            continue;
        }
        seen.push(Choice {
            value,
            display: line.to_string(),
        });
        if seen.len() >= 500 {
            break;
        }
    }
    seen
}

fn run_capture(cmd: &str, _timeout: Duration) -> Option<String> {
    let output = if cfg!(target_os = "windows") {
        Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", cmd])
            .output()
    } else {
        Command::new("sh").args(["-c", cmd]).output()
    };
    match output {
        Ok(o) if o.status.success() => Some(String::from_utf8_lossy(&o.stdout).into_owned()),
        // 命令存在但返回非零（比如不在 git 仓库里）也可能有有用输出
        Ok(o) if !o.stdout.is_empty() => Some(String::from_utf8_lossy(&o.stdout).into_owned()),
        _ => None,
    }
}

/// 内置变量。算不出来就返回 None，退回自由输入。
pub fn builtin(name: &str) -> Option<String> {
    let key = name.trim_start_matches('@');
    match key {
        "cwd" => std::env::current_dir()
            .ok()
            .map(|p| p.display().to_string()),
        "date" => Some(today()),
        "clipboard" => None, // 由前端注入，core 不碰剪贴板
        "host" => run_capture("hostname", Duration::from_secs(2)).map(|s| s.trim().to_string()),
        "git.branch" => run_capture("git rev-parse --abbrev-ref HEAD", Duration::from_secs(2))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        "git.root" => run_capture("git rev-parse --show-toplevel", Duration::from_secs(2))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        other => other
            .strip_prefix("env.")
            .and_then(|v| std::env::var(v).ok()),
    }
}

fn today() -> String {
    // 不引入 chrono：从 UNIX 时间戳自己换算日期
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs / 86_400;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Howard Hinnant 的 civil_from_days 算法。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 决定一个变量该怎么问。
pub fn plan(
    name: &str,
    inline_default: Option<&str>,
    decl: Option<&VarDecl>,
    profiles: &Profiles,
    profile_name: &str,
    allow_shell: bool,
) -> Ask {
    if name.starts_with('@') {
        if let Some(v) = builtin(name) {
            return Ask::Resolved(v);
        }
        return Ask::Text {
            label: format!("{name}（内置变量取值失败，请手填）"),
            default: inline_default.map(String::from),
        };
    }

    let label = match decl.and_then(|d| d.desc.clone()) {
        Some(desc) => format!("{name} — {desc}"),
        None => name.to_string(),
    };

    // 没有声明的变量也要查 Profile。`jot save` 的反向参数化生成的 `{{service}}`
    // 并不会附带 vars: 声明 —— 不查的话，它生成的笔记本反而用不了推导它时
    // 依据的那个 Profile 值。显式写了 from: ask / from: shell 的除外，那是明确意图。
    let consult_profile = decl.map(|d| d.source() == "profile").unwrap_or(true);
    if consult_profile {
        if let Some(v) = profiles.get(profile_name, name) {
            return Ask::Resolved(v.to_string());
        }
        // Profile 里没配，继续往下降级
    }

    if let Some(d) = decl {
        if allow_shell {
            if let Some(cmd) = d.cmd.as_deref() {
                let opts = shell_candidates(cmd);
                if !opts.is_empty() {
                    return Ask::Choose {
                        label,
                        options: opts,
                        default: inline_default.map(String::from),
                    };
                }
            }
        }

        if let Some(opts) = d.options.as_ref() {
            if !opts.is_empty() {
                return Ask::Choose {
                    label,
                    options: opts
                        .iter()
                        .map(|o| Choice {
                            value: o.clone(),
                            display: o.clone(),
                        })
                        .collect(),
                    default: inline_default.map(String::from),
                };
            }
        }
    }

    Ask::Text {
        label,
        default: inline_default.map(String::from),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_format_is_sane() {
        let d = today();
        assert_eq!(d.len(), 10);
        assert_eq!(&d[4..5], "-");
        let year: i64 = d[..4].parse().unwrap();
        assert!((2024..2100).contains(&year), "算出来的年份不对: {d}");
    }

    #[test]
    fn civil_epoch_is_1970_01_01() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn profile_hit_resolves_without_asking() {
        let mut p = Profiles::default();
        p.set("prod", "service", "api.service");
        let decl = VarDecl {
            from: Some("profile".into()),
            ..Default::default()
        };
        match plan("service", None, Some(&decl), &p, "prod", false) {
            Ask::Resolved(v) => assert_eq!(v, "api.service"),
            other => panic!("应该直接解析出来，得到 {other:?}"),
        }
    }

    #[test]
    fn profile_miss_falls_back_to_options() {
        let p = Profiles::default();
        let decl = VarDecl {
            from: Some("profile".into()),
            options: Some(vec!["a".into(), "b".into()]),
            ..Default::default()
        };
        match plan("x", None, Some(&decl), &p, "default", false) {
            Ask::Choose { options, .. } => assert_eq!(options.len(), 2),
            other => panic!("应该降级到候选列表，得到 {other:?}"),
        }
    }

    #[test]
    fn undeclared_var_is_free_text() {
        let p = Profiles::default();
        match plan("whatever", Some("8000"), None, &p, "default", false) {
            Ask::Text { default, .. } => assert_eq!(default.as_deref(), Some("8000")),
            other => panic!("应该是自由输入，得到 {other:?}"),
        }
    }

    /// `jot save` 的反向参数化不会写 vars: 声明，所以未声明的变量必须查 Profile，
    /// 否则它生成的笔记本用不了推导它时依据的那个值。
    #[test]
    fn undeclared_var_still_consults_profile() {
        let mut p = Profiles::default();
        p.set("default", "service", "my-api.service");
        match plan("service", None, None, &p, "default", false) {
            Ask::Resolved(v) => assert_eq!(v, "my-api.service"),
            other => panic!("未声明的变量没有查 Profile，得到 {other:?}"),
        }
    }

    /// 显式写了 from: ask 就是明确要问，Profile 不该抢答。
    #[test]
    fn explicit_ask_beats_profile() {
        let mut p = Profiles::default();
        p.set("default", "service", "my-api.service");
        let decl = VarDecl {
            from: Some("ask".into()),
            ..Default::default()
        };
        assert!(
            matches!(
                plan("service", None, Some(&decl), &p, "default", false),
                Ask::Text { .. }
            ),
            "from: ask 被 Profile 抢答了"
        );
    }

    #[test]
    fn builtin_cwd_resolves() {
        let p = Profiles::default();
        assert!(matches!(
            plan("@cwd", None, None, &p, "default", false),
            Ask::Resolved(_)
        ));
    }
}
