//! Resolving variables: built-ins, profile values, and shell-generated candidates.
//!
//! Order (design doc D-03):
//!   1. leading `@` -> a built-in, computed without interrupting the user
//!   2. `from: profile` with a value in the active profile -> used directly
//!   3. a `cmd` -> run it for a candidate list to choose from
//!   4. `options` -> a fixed candidate list
//!   5. otherwise -> free text
//!
//! Every step degrades into the next; nothing here may hang or abort the tool.

use crate::config::Profiles;
use crate::notebook::VarDecl;
use crate::t;
use std::process::Command;
use std::time::Duration;

/// How a variable should be asked for.
#[derive(Debug, Clone)]
pub enum Ask {
    /// Already has a value, nothing to ask
    Resolved(String),
    /// Pick from a list, with a typed value also allowed
    Choose {
        label: String,
        options: Vec<Choice>,
        default: Option<String>,
    },
    /// Free text only
    Text {
        label: String,
        default: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct Choice {
    /// The value substituted into the command
    pub value: String,
    /// The full line shown in the list, which may carry extra detail
    pub display: String,
}

/// Run a command for candidates. Empty on failure, and the caller degrades to free text.
pub fn shell_candidates(cmd: &str) -> Vec<Choice> {
    let out = run_capture(cmd, Duration::from_secs(5));
    let Some(text) = out else { return Vec::new() };
    let mut seen = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r').trim();
        if line.is_empty() {
            continue;
        }
        // The first field is the value, the whole line is what gets displayed
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
        // A non-zero exit (say, not inside a git repo) can still carry useful output
        Ok(o) if !o.stdout.is_empty() => Some(String::from_utf8_lossy(&o.stdout).into_owned()),
        _ => None,
    }
}

/// Built-in variables. None when it cannot be computed, which falls back to free text.
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
    // No chrono dependency: derive the date from the UNIX timestamp
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs / 86_400;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Howard Hinnant's civil_from_days algorithm.
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

/// Decide how a variable should be asked for.
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
            label: format!(
                "{}",
                t!(
                    "{name}（内置变量取值失败，请手填）",
                    "{name} (built-in lookup failed, enter it manually)"
                )
            ),
            default: inline_default.map(String::from),
        };
    }

    let label = match decl.and_then(|d| d.desc.clone()) {
        Some(desc) => format!("{name} — {desc}"),
        None => name.to_string(),
    };

    // Undeclared variables consult the profile too. The `{{service}}` produced by
    // `jot save`'s reverse parameterization carries no vars: declaration, so
    // without this the entry it generates cannot use the value it came from.
    let consult_profile = decl.map(|d| d.source() == "profile").unwrap_or(true);
    if consult_profile {
        if let Some(v) = profiles.get(profile_name, name) {
            return Ask::Resolved(v.to_string());
        }
        // Not in the profile, so carry on down
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
        assert!((2024..2100).contains(&year), "computed year is wrong: {d}");
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
            other => panic!("should have resolved outright, got {other:?}"),
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
            other => panic!("should have fallen back to a candidate list, got {other:?}"),
        }
    }

    #[test]
    fn undeclared_var_is_free_text() {
        let p = Profiles::default();
        match plan("whatever", Some("8000"), None, &p, "default", false) {
            Ask::Text { default, .. } => assert_eq!(default.as_deref(), Some("8000")),
            other => panic!("should have been free text, got {other:?}"),
        }
    }

    /// `jot save`'s reverse parameterization writes no vars: declaration, so an
    /// undeclared variable must consult the profile or its output is unusable.
    #[test]
    fn undeclared_var_still_consults_profile() {
        let mut p = Profiles::default();
        p.set("default", "service", "my-api.service");
        match plan("service", None, None, &p, "default", false) {
            Ask::Resolved(v) => assert_eq!(v, "my-api.service"),
            other => panic!("an undeclared variable did not consult the profile, got {other:?}"),
        }
    }

    /// An explicit from: ask means ask, and the profile must not pre-empt it.
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
            "the profile pre-empted an explicit from: ask"
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
