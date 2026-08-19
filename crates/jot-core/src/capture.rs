//! 捕获：从 shell 历史抓命令、写回笔记本、反向参数化。
//!
//! 这是对抗「空笔记本」的部分（见设计文档 §6）。工具再好，笔记本是空的
//! 就没有价值，所以录入必须比打开编辑器更省事。

use crate::config::{Paths, Profiles};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// 一条历史记录及其出现次数。
#[derive(Debug, Clone)]
pub struct HistItem {
    pub command: String,
    pub count: usize,
}

/// 各 shell 的历史文件位置。
pub fn history_files() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        v.push(
            PathBuf::from(appdata)
                .join("Microsoft")
                .join("Windows")
                .join("PowerShell")
                .join("PSReadLine")
                .join("ConsoleHost_history.txt"),
        );
    }
    if let Some(home) = dirs::home_dir() {
        v.push(home.join(".bash_history"));
        v.push(home.join(".zsh_history"));
        v.push(home.join(".local/share/fish/fish_history"));
    }
    v.into_iter().filter(|p| p.is_file()).collect()
}

fn normalize_line(raw: &str) -> Option<String> {
    let mut s = raw.trim().to_string();
    // zsh:  ": 1700000000:0;command"
    if s.starts_with(": ") {
        if let Some(i) = s.find(';') {
            s = s[i + 1..].to_string();
        }
    }
    // fish: "- cmd: command"
    if let Some(rest) = s.strip_prefix("- cmd: ") {
        s = rest.to_string();
    }
    let s = s.trim().to_string();
    if s.is_empty() {
        return None;
    }
    Some(s)
}

/// 看起来像密钥的命令不导入。宁可漏掉几条，也不能把 token 存进笔记本。
pub fn looks_secret(cmd: &str) -> bool {
    let l = cmd.to_ascii_lowercase();
    const NEEDLES: &[&str] = &[
        "password",
        "passwd",
        "token",
        "secret",
        "api_key",
        "apikey",
        "私钥",
        "private_key",
        "credential",
        "bearer ",
        "authorization:",
        "--pass",
    ];
    if NEEDLES.iter().any(|n| l.contains(n)) {
        return true;
    }
    // 长串无空格的随机字符，多半是 key
    cmd.split_whitespace().any(|w| {
        w.len() >= 40
            && w.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    })
}

fn is_noise(cmd: &str) -> bool {
    let t = cmd.trim();
    if t.chars().count() < 4 {
        return true;
    }
    let head = t.split_whitespace().next().unwrap_or("");
    matches!(
        head,
        "ls" | "ll" | "cd" | "pwd" | "clear" | "cls" | "exit" | "jot" | "dir" | "history" | "q"
    )
}

/// 读取全部历史，按出现次数排序。
pub fn history_ranked(limit: usize) -> Vec<HistItem> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for file in history_files() {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for raw in text.lines() {
            let Some(cmd) = normalize_line(raw) else {
                continue;
            };
            if is_noise(&cmd) || looks_secret(&cmd) {
                continue;
            }
            let e = counts.entry(cmd.clone()).or_insert(0);
            if *e == 0 {
                order.push(cmd);
            }
            *e += 1;
        }
    }

    let mut items: Vec<HistItem> = order
        .into_iter()
        .map(|command| {
            let count = counts[&command];
            HistItem { command, count }
        })
        .collect();
    items.sort_by(|a, b| b.count.cmp(&a.count).then(a.command.cmp(&b.command)));
    items.truncate(limit);
    items
}

/// 历史里最后一条真实命令（`jot save` 不带参数时用）。
pub fn last_command() -> Option<String> {
    let mut newest: Option<(std::time::SystemTime, String)> = None;
    for file in history_files() {
        let Ok(meta) = std::fs::metadata(&file) else {
            continue;
        };
        let Ok(mtime) = meta.modified() else { continue };
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let last = text
            .lines()
            .rev()
            .filter_map(normalize_line)
            .find(|c| !is_noise(c));
        if let Some(cmd) = last {
            if newest.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                newest = Some((mtime, cmd));
            }
        }
    }
    newest.map(|(_, c)| c)
}

/// 反向参数化：命令里出现了当前 Profile 的某个值，就建议换成变量。
///
/// 存 `sudo systemctl restart kestrel-orders-api.service` 时，如果 Profile 里
/// `service` 正好是这个值，就自动变成 `{{service}}`。
pub fn parameterize(command: &str, profiles: &Profiles, profile: &str) -> (String, Vec<String>) {
    let mut out = command.to_string();
    let mut applied = Vec::new();
    let mut pairs: Vec<(&str, &str)> = profiles.entries(profile);
    // 先替换长的值，避免短值先命中把长值切断
    pairs.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
    for (key, value) in pairs {
        if value.len() < 3 || !out.contains(value) {
            continue;
        }
        out = out.replace(value, &format!("{{{{{key}}}}}"));
        applied.push(key.to_string());
    }
    (out, applied)
}

/// 猜一个标题：取命令的前两个有意义的词。
pub fn guess_title(command: &str) -> String {
    let first_line = command.lines().next().unwrap_or(command).trim();
    let words: Vec<&str> = first_line
        .split_whitespace()
        .filter(|w| !w.starts_with('-'))
        .take(3)
        .collect();
    if words.is_empty() {
        "新命令".to_string()
    } else {
        words.join(" ")
    }
}

/// 根据命令内容猜围栏语言。
pub fn guess_lang(command: &str) -> &'static str {
    let l = command.trim_start().to_ascii_lowercase();
    const SQL: &[&str] = &["select ", "insert ", "update ", "delete ", "with "];
    const PS: &[&str] = &[
        "get-", "set-", "new-", "remove-", "start-", "stop-", "$env:",
    ];

    if SQL.iter().any(|p| l.starts_with(p)) {
        "sql"
    } else if PS.iter().any(|p| l.starts_with(p)) || cfg!(target_os = "windows") {
        "ps1"
    } else {
        "sh"
    }
}

/// 往笔记本末尾追加一条。
pub fn append(
    paths: &Paths,
    notebook: Option<&str>,
    title: &str,
    description: &str,
    lang: &str,
    command: &str,
) -> Result<PathBuf> {
    let path = match notebook {
        Some(n) => paths.local_dir().join(format!("{n}.md")),
        None => crate::builtin::ensure_personal_notebook(paths)?,
    };
    if !path.exists() {
        std::fs::create_dir_all(paths.local_dir())?;
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        std::fs::write(
            &path,
            format!("---\nname: {stem}\ndescription: {stem}\n---\n\n"),
        )?;
    }

    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let mut block = String::new();
    if !existing.ends_with('\n') && !existing.is_empty() {
        block.push('\n');
    }
    if !existing.ends_with("\n\n") && !existing.is_empty() {
        block.push('\n');
    }
    block.push_str(&format!("## {title}\n\n"));
    if !description.trim().is_empty() {
        block.push_str(&format!("{}\n\n", description.trim()));
    }
    block.push_str(&format!("```{lang}\n{command}\n```\n"));

    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .with_context(|| format!("打不开 {}", path.display()))?;
    f.write_all(block.as_bytes())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zsh_timestamps_are_stripped() {
        assert_eq!(
            normalize_line(": 1700000000:0;git status").as_deref(),
            Some("git status")
        );
    }

    #[test]
    fn fish_prefix_is_stripped() {
        assert_eq!(
            normalize_line("- cmd: git status").as_deref(),
            Some("git status")
        );
    }

    #[test]
    fn secrets_are_rejected() {
        assert!(looks_secret("export GITHUB_TOKEN=abc"));
        assert!(looks_secret("mysql -u root --password=hunter2"));
        assert!(looks_secret(
            "curl -H 'Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9'"
        ));
        assert!(!looks_secret("git status"));
        assert!(!looks_secret("docker compose up -d"));
    }

    #[test]
    fn noise_is_rejected() {
        assert!(is_noise("ls"));
        assert!(is_noise("cd ..")); // head 是 cd
        assert!(is_noise("jot save"));
        assert!(!is_noise("git rebase -i HEAD~3"));
    }

    #[test]
    fn reverse_parameterize_finds_profile_values() {
        let mut p = Profiles::default();
        p.set("prod", "service", "kestrel-orders-api.service");
        let (out, applied) = parameterize(
            "sudo systemctl restart kestrel-orders-api.service",
            &p,
            "prod",
        );
        assert_eq!(out, "sudo systemctl restart {{service}}");
        assert_eq!(applied, vec!["service"]);
    }

    #[test]
    fn parameterize_prefers_longer_values() {
        let mut p = Profiles::default();
        p.set("d", "short", "api");
        p.set("d", "long", "api.example.com");
        let (out, _) = parameterize("curl https://api.example.com/health", &p, "d");
        assert!(out.contains("{{long}}"), "得到 {out}");
    }

    #[test]
    fn title_guess_skips_flags() {
        assert_eq!(guess_title("git rebase -i HEAD~3"), "git rebase HEAD~3");
    }

    #[test]
    fn lang_guess() {
        assert_eq!(guess_lang("SELECT * FROM t"), "sql");
        assert_eq!(guess_lang("Get-Process"), "ps1");
    }
}
