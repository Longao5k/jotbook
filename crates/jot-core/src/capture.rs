//! Capture: pull commands out of shell history, write them back to a
//!
//! notebook, and reverse-parameterize them. This is the part that fights the
//! empty notebook (design doc 6). A tool nobody fills in has no value, so
//! capturing has to be less work than opening an editor.

use crate::config::{Paths, Profiles};
use crate::t;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// One history entry and how often it appears.
#[derive(Debug, Clone)]
pub struct HistItem {
    pub command: String,
    pub count: usize,
}

/// Where each shell keeps its history.
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

/// Commands that look like they carry a secret are never imported. Better to
/// miss a few than to write a token into a notebook.
pub fn looks_secret(cmd: &str) -> bool {
    let l = cmd.to_ascii_lowercase();
    const NEEDLES: &[&str] = &[
        "password",
        "passwd",
        "token",
        "secret",
        "api_key",
        "apikey",
        // Kept in Chinese on purpose: this is a detection needle, not prose
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
    // A long run of characters with no spaces is usually a key
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

/// Read all history, ranked by how often each command appears.
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

/// The last real command in history, used by a bare `jot save`.
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

/// Reverse parameterization: when a command contains one of the active
///
/// profile's values, offer to swap it for the variable. Saving
/// `sudo systemctl restart api.service` becomes `{{service}}` automatically.
pub fn parameterize(command: &str, profiles: &Profiles, profile: &str) -> (String, Vec<String>) {
    let mut out = command.to_string();
    let mut applied = Vec::new();
    let mut pairs: Vec<(&str, &str)> = profiles.entries(profile);
    // Longest values first, so a short one cannot cut a longer one in half
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

/// Guess a title from the first couple of meaningful words.
pub fn guess_title(command: &str) -> String {
    let first_line = command.lines().next().unwrap_or(command).trim();
    let words: Vec<&str> = first_line
        .split_whitespace()
        .filter(|w| !w.starts_with('-'))
        .take(3)
        .collect();
    if words.is_empty() {
        t!("新命令", "New command").into_owned().to_string()
    } else {
        words.join(" ")
    }
}

/// Guess the fence language from the command itself.
pub fn guess_lang(command: &str) -> &'static str {
    let l = command.trim_start().to_ascii_lowercase();
    const SQL: &[&str] = &["select ", "insert ", "update ", "delete ", "with "];
    const PS: &[&str] = &[
        "get-", "set-", "new-", "remove-", "start-", "stop-", "$env:",
    ];
    // Commands that only exist on a POSIX system. Without these, saving
    // `sudo systemctl restart ...` from Windows would label it ps1.
    const POSIX: &[&str] = &[
        "sudo ",
        "systemctl",
        "journalctl",
        "apt ",
        "apt-get",
        "yum ",
        "dnf ",
        "chmod ",
        "chown ",
        "./",
        "grep ",
        "awk ",
        "sed ",
        "ls -",
        "rm -rf",
        "ssh ",
        "scp ",
        "rsync ",
        "tar ",
        "df -",
        "du -",
        "ps aux",
    ];

    if SQL.iter().any(|p| l.starts_with(p)) {
        "sql"
    } else if POSIX.iter().any(|p| l.starts_with(p)) {
        "sh"
    } else if PS.iter().any(|p| l.starts_with(p)) || cfg!(target_os = "windows") {
        "ps1"
    } else {
        "sh"
    }
}

/// Append an entry to a notebook.
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
        assert!(out.contains("{{long}}"), "got {out}");
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

/// One command pulled out of pasted text, before it becomes an entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    pub command: String,
    pub description: String,
}

/// Strip a list marker: `1.` `2)` `3、` `-` `*` `•` `+`.
fn strip_list_marker(s: &str) -> &str {
    let t = s.trim_start();
    let mut digits = 0usize;
    for (i, c) in t.char_indices() {
        if c.is_ascii_digit() {
            digits += 1;
            continue;
        }
        if digits > 0 && matches!(c, '.' | ')' | '、' | ':' | '：') {
            return t[i + c.len_utf8()..].trim_start();
        }
        break;
    }
    for p in ["- ", "* ", "• ", "+ "] {
        if let Some(r) = t.strip_prefix(p) {
            return r.trim_start();
        }
    }
    t
}

/// Strip leading comment markers, repeatedly - pasted code is often
/// double-commented, as in `// // note`.
fn strip_comment_markers(s: &str) -> &str {
    let mut t = s.trim_start();
    loop {
        let next = if let Some(r) = t.strip_prefix("//") {
            r
        } else if let Some(r) = t.strip_prefix("# ") {
            r
        } else if let Some(r) = t.strip_prefix("-- ") {
            r
        } else {
            break;
        };
        t = next.trim_start();
    }
    t
}

fn is_cjk(c: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&c) || ('\u{3000}'..='\u{303f}').contains(&c)
}

/// Split a pasted line into the command and the note someone wrote beside it.
///
/// Three shapes, in order of how unambiguous they are: an explicit run of
/// separator characters, a gap of two or more spaces, and the point where CJK
/// prose begins. The last one matters because notes get written flush against
/// the command - `user_rotation 0竖屏` has no separator at all.
pub fn split_pasted_line(line: &str) -> (String, String) {
    let s = line.trim();
    let b = s.as_bytes();

    // A run of two or more dashes or equals, preceded by a space
    for i in 1..b.len() {
        if (b[i] == b'-' || b[i] == b'=') && b[i - 1] == b' ' {
            let run = b[i..].iter().take_while(|c| **c == b[i]).count();
            if run >= 2 {
                let cmd = s[..i].trim_end();
                if !cmd.is_empty() {
                    return (cmd.to_string(), s[i + run..].trim().to_string());
                }
            }
        }
    }

    // A gap of two or more spaces
    if let Some(i) = s.find("  ") {
        let cmd = s[..i].trim_end();
        let rest = strip_comment_markers(s[i..].trim());
        if !cmd.is_empty() && !rest.is_empty() {
            return (cmd.to_string(), rest.trim().to_string());
        }
    }

    // Where CJK prose begins, as long as there is a command in front of it
    if let Some((i, _)) = s.char_indices().find(|(_, c)| is_cjk(*c)) {
        let cmd = s[..i].trim_end();
        if !cmd.is_empty() && cmd.chars().any(|c| c.is_ascii_alphanumeric()) {
            return (cmd.to_string(), s[i..].trim().to_string());
        }
    }

    (s.to_string(), String::new())
}

/// Does this look like a command rather than a sentence someone wrote?
fn looks_like_command(s: &str) -> bool {
    let s = s.trim();
    if s.chars().count() < 3 {
        return false;
    }
    let Some(head) = s.split_whitespace().next() else {
        return false;
    };
    if head.chars().any(is_cjk) {
        return false;
    }
    // Sentences end in a full stop; commands do not
    if s.ends_with('.') && s.split_whitespace().count() > 4 {
        return false;
    }
    // Source code, not a shell command. Pasted notes routinely include a
    // commented-out line of the program that used to run the command.
    const CODE_KEYWORDS: &[&str] = &[
        "await ",
        "async ",
        "const ",
        "let ",
        "var ",
        "function ",
        "return ",
        "import ",
        "export ",
        "public ",
        "private ",
        "def ",
        "class ",
    ];
    if CODE_KEYWORDS.iter().any(|k| s.starts_with(k)) {
        return false;
    }
    // A call that ends in a semicolon is a statement in some language. A shell
    // command can end in `;` (find -exec ... \;) but not with parens as well.
    if s.ends_with(';') && s.contains('(') && s.contains(')') {
        return false;
    }
    // A capitalised first word with no punctuation in it, followed by more
    // words, is a sentence: "Remove saved WiFi configurations". PowerShell
    // cmdlets are spared by their hyphen (Get-Process, Script-Migration) and
    // SQL by being upper case throughout (SELECT, INSERT).
    let starts_upper = head.starts_with(|c: char| c.is_ascii_uppercase());
    let has_lower = head.chars().any(|c| c.is_ascii_lowercase());
    let plain_word = head.chars().all(|c| c.is_ascii_alphabetic());
    if starts_upper && has_lower && plain_word && s.split_whitespace().count() >= 3 {
        return false;
    }
    head.chars()
        .all(|c| c.is_ascii_alphanumeric() || "._-/\\$~:".contains(c))
}

/// Turn pasted text into drafts, one per line.
///
/// Notably it tags nothing: whether a command is Linux-only is the author's
/// call, and guessing would put entries out of reach for anyone working over
/// ssh or in WSL.
pub fn parse_pasted(text: &str) -> Vec<Draft> {
    let mut out: Vec<Draft> = Vec::new();
    for raw in text.lines() {
        let line = strip_comment_markers(strip_list_marker(raw));
        if line.trim().is_empty() {
            continue;
        }
        let (command, description) = split_pasted_line(line);
        if !looks_like_command(&command) || looks_secret(&command) {
            continue;
        }
        if out.iter().any(|d| d.command == command) {
            continue;
        }
        out.push(Draft {
            command,
            description,
        });
    }
    out
}

#[cfg(test)]
mod paste_tests {
    use super::*;

    /// The real thing someone pasted: numbered, mixed languages, notes
    /// attached four different ways, and two commented-out code lines.
    const PASTED: &str = concat!(
        "1. adb shell cat /proc/meminfo | head -n 5           ---------- Check RAM size\n",
        "2. adb shell su 0 rm /data/misc/wifi/WifiConfigStore.xml 清除wifi\n",
        "\n",
        "3.  // // Remove saved WiFi configurations\n",
        "      // await shell.run(x);\n",
        "4.adb shell screencap -p /sdcard/Resetdialog.png\n",
        "5. adb shell settings put system user_rotation 0竖屏\n",
        "\n",
        "6.adb shell settings put system font_scale 1.2 更改默认字体大小\n",
        "\n",
        "7. apksigner verify -verbose -print-certs .\\MyApp_release.apk 查看签名\n",
    );

    #[test]
    fn a_dash_run_separates_command_from_note() {
        let (c, d) = split_pasted_line(
            "adb shell cat /proc/meminfo | head -n 5     ---------- Check RAM size",
        );
        assert_eq!(c, "adb shell cat /proc/meminfo | head -n 5");
        assert_eq!(d, "Check RAM size");
    }

    /// A flag is not a separator, however it is spelled.
    #[test]
    fn flags_are_not_separators() {
        let (c, d) = split_pasted_line("apksigner verify -verbose -print-certs a.apk");
        assert_eq!(c, "apksigner verify -verbose -print-certs a.apk");
        assert_eq!(d, "");
    }

    #[test]
    fn cjk_prose_is_split_off_even_with_no_separator() {
        let (c, d) = split_pasted_line("adb shell settings put system user_rotation 0竖屏");
        assert_eq!(c, "adb shell settings put system user_rotation 0");
        assert_eq!(d, "竖屏");

        let (c, d) = split_pasted_line("apksigner verify -print-certs a.apk 查看签名");
        assert_eq!(c, "apksigner verify -print-certs a.apk");
        assert_eq!(d, "查看签名");
    }

    #[test]
    fn list_markers_and_comments_are_stripped() {
        assert_eq!(strip_list_marker("1. adb devices"), "adb devices");
        assert_eq!(strip_list_marker("4.adb devices"), "adb devices");
        assert_eq!(strip_list_marker("- adb devices"), "adb devices");
        assert_eq!(strip_comment_markers("// // adb devices"), "adb devices");
        assert_eq!(strip_comment_markers("# adb devices"), "adb devices");
    }

    #[test]
    fn the_pasted_block_normalises() {
        let drafts = parse_pasted(PASTED);
        let cmds: Vec<&str> = drafts.iter().map(|d| d.command.as_str()).collect();

        for expected in [
            "adb shell cat /proc/meminfo | head -n 5",
            "adb shell su 0 rm /data/misc/wifi/WifiConfigStore.xml",
            "adb shell screencap -p /sdcard/Resetdialog.png",
            "adb shell settings put system user_rotation 0",
            "adb shell settings put system font_scale 1.2",
        ] {
            assert!(
                cmds.contains(&expected),
                "missing {expected:?} in {cmds:#?}"
            );
        }

        // A sentence is not a command
        assert!(
            !cmds.iter().any(|c| c.starts_with("Remove saved")),
            "a sentence was imported as a command: {cmds:#?}"
        );

        // The notes came across with them
        let ram = drafts
            .iter()
            .find(|d| d.command.contains("meminfo"))
            .unwrap();
        assert_eq!(ram.description, "Check RAM size");
        let wifi = drafts
            .iter()
            .find(|d| d.command.contains("WifiConfigStore"))
            .unwrap();
        assert_eq!(wifi.description, "清除wifi");
    }

    /// The prose rule must not eat PowerShell cmdlets or SQL.
    #[test]
    fn commands_that_start_with_a_capital_survive() {
        for cmd in [
            "Script-Migration 20250801_Init latest -Idempotent",
            "Get-Process jot | Stop-Process",
            "SELECT id FROM users WHERE active",
            "Remove-Item -Recurse -Force node_modules",
        ] {
            assert!(looks_like_command(cmd), "rejected a real command: {cmd}");
        }
        for prose in [
            "Remove saved WiFi configurations",
            "Check the RAM size first",
            // A commented-out line of the program that used to run it
            "await shell.run('su 0 rm /data/misc/wifi/WifiConfigStore.xml');",
            "const out = execSync(cmd);",
        ] {
            assert!(!looks_like_command(prose), "accepted non-command: {prose}");
        }
        // ...but a shell command that legitimately ends in a semicolon stays
        assert!(looks_like_command(r"find . -name x -exec rm {} \;"));
    }

    #[test]
    fn duplicates_and_secrets_are_dropped() {
        let drafts = parse_pasted("adb devices\nadb devices\nexport TOKEN=abc123def\n");
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].command, "adb devices");
    }

    /// Nothing acquires a platform: whether a command is Linux-only is the
    /// author's call, never a guess.
    #[test]
    fn nothing_is_tagged_automatically() {
        let drafts = parse_pasted("sudo systemctl restart api\n");
        assert_eq!(drafts.len(), 1);
        assert!(drafts[0].description.is_empty());
    }
}
