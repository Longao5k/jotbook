//! Notebook parsing: YAML frontmatter, Markdown headings, fenced code blocks.
//!
//! A hand-written line scanner rather than a markdown library: only headings,
//! prose and attributed fences matter, and scanning lines is both enough and fast.

use crate::t;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct VarDecl {
    #[serde(default)]
    pub desc: Option<String>,
    /// ask | profile | shell
    #[serde(default)]
    pub from: Option<String>,
    /// Command that produces the candidate list
    #[serde(default)]
    pub cmd: Option<String>,
    /// Fixed candidates
    #[serde(default)]
    pub options: Option<Vec<String>>,
}

impl VarDecl {
    pub fn source(&self) -> &str {
        self.from.as_deref().unwrap_or("ask")
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct FrontMatter {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    platform: Option<Vec<String>>,
    #[serde(default)]
    vars: BTreeMap<String, VarDecl>,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub title: String,
    pub description: String,
    pub lang: String,
    pub command: String,
    pub notebook: String,
    pub tags: Vec<String>,
    pub platforms: Vec<String>,
    pub confirm: bool,
    pub remote: bool,
    pub source: PathBuf,
    pub line: usize,
    /// From a trusted location (built-in, local, or an explicitly trusted source).
    pub trusted: bool,
}

impl Entry {
    pub fn visible_on(&self, plat: &str) -> bool {
        self.platforms.is_empty() || self.platforms.iter().any(|p| p == plat)
    }

    /// A stable identifier across sessions, used to record usage.
    ///
    /// Keyed on the *command*, not the title. Titles are translated, so keying
    /// on them would throw away everyone's frecency data the moment they ran
    /// `jot lang` - and would do the same whenever a contributor reworded one.
    /// The command is identical across languages, which the notebook sync test
    /// enforces.
    pub fn id(&self) -> String {
        format!("{}/{:016x}", self.notebook, fnv1a(&self.command))
    }

    /// The text fuzzy matching runs against.
    pub fn haystack(&self) -> String {
        format!(
            "{} {} {} {} {}",
            self.title,
            self.notebook,
            self.command,
            self.tags.join(" "),
            self.description
        )
    }

    /// Multi-line? That changes injection: multi-line needs bracketed paste.
    pub fn is_multiline(&self) -> bool {
        self.command.contains('\n')
    }

    /// Reference material rather than a runnable command (cheatsheets, config templates).
    pub fn is_reference(&self) -> bool {
        matches!(self.lang.as_str(), "txt" | "ini" | "yaml" | "yml")
            || self.tags.iter().any(|t| t == "reference")
    }
}

#[derive(Debug, Clone)]
pub struct Notebook {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub tags: Vec<String>,
    pub vars: BTreeMap<String, VarDecl>,
    pub entries: Vec<Entry>,
}

/// FNV-1a. Only needs to be stable and well spread, not cryptographic.
fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

pub fn current_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// Split the frontmatter from the body.
fn split_frontmatter(text: &str) -> (Option<&str>, &str, usize) {
    let t = text.strip_prefix('\u{feff}').unwrap_or(text);
    if !t.starts_with("---") {
        return (None, t, 0);
    }
    let after = match t.find('\n') {
        Some(i) => i + 1,
        None => return (None, t, 0),
    };
    let rest = &t[after..];
    // Find the --- that sits alone on its line
    let mut offset = 0usize;
    for (lines, line) in (1usize..).zip(rest.split_inclusive('\n')) {
        if line.trim_end() == "---" {
            let fm = &rest[..offset];
            let body_start = after + offset + line.len();
            return (Some(fm), &t[body_start..], lines + 1);
        }
        offset += line.len();
    }
    (None, t, 0)
}

/// Parse a fence info string: ```sh @platform=linux @confirm @tags=deploy
struct FenceInfo {
    lang: String,
    platforms: Vec<String>,
    tags: Vec<String>,
    confirm: bool,
    remote: bool,
}

fn parse_fence_info(info: &str) -> FenceInfo {
    let mut out = FenceInfo {
        lang: String::new(),
        platforms: Vec::new(),
        tags: Vec::new(),
        confirm: false,
        remote: false,
    };
    for (i, tok) in info.split_whitespace().enumerate() {
        if i == 0 && !tok.starts_with('@') {
            out.lang = tok.to_ascii_lowercase();
            continue;
        }
        let tok = tok.trim_start_matches('@');
        match tok.split_once('=') {
            Some(("platform", v)) => out
                .platforms
                .extend(v.split(',').map(|s| s.trim().to_ascii_lowercase())),
            // @tags= may appear more than once and accumulates
            Some(("tags", v)) => out.tags.extend(v.split(',').map(|s| s.trim().to_string())),
            Some(("id", _)) => {}
            _ => match tok {
                "confirm" => out.confirm = true,
                "remote" => out.remote = true,
                _ => {}
            },
        }
    }
    if out.lang.is_empty() {
        out.lang = "sh".into();
    }
    out
}

fn fence_marker(line: &str) -> Option<(usize, &str)> {
    let t = line.trim_start();
    if !t.starts_with("```") {
        return None;
    }
    let indent = line.len() - t.len();
    let ticks = t.chars().take_while(|c| *c == '`').count();
    if ticks < 3 {
        return None;
    }
    Some((indent, t[ticks..].trim()))
}

pub fn parse(path: &Path, text: &str) -> Result<Notebook> {
    let (fm_raw, body, fm_lines) = split_frontmatter(text);
    let fm: FrontMatter = match fm_raw {
        Some(raw) => serde_yaml::from_str(raw).with_context(|| {
            format!(
                "{}",
                t!(
                    "{} 的 frontmatter 不是合法 YAML",
                    "the frontmatter in {} is not valid YAML",
                    path.display()
                )
            )
        })?,
        None => FrontMatter::default(),
    };

    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".into());
    let name = fm.name.clone().unwrap_or(stem);

    let mut entries = Vec::new();
    let mut title = String::new();
    let mut desc: Vec<String> = Vec::new();

    let mut in_fence = false;
    let mut fence_ticks = 0usize;
    let mut fence_indent = 0usize;
    let mut fence_info = String::new();
    let mut fence_start = 0usize;
    let mut buf: Vec<String> = Vec::new();

    for (idx, raw_line) in body.lines().enumerate() {
        let lineno = fm_lines + idx + 1;

        if in_fence {
            let t = raw_line.trim_start();
            let closes = t.starts_with("```")
                && t.chars().take_while(|c| *c == '`').count() >= fence_ticks
                && t[fence_ticks.min(t.len())..].trim().is_empty();
            if closes {
                in_fence = false;
                let command = buf.join("\n").trim_end().to_string();
                buf.clear();
                if !title.is_empty() && !command.is_empty() {
                    let info = parse_fence_info(&fence_info);
                    let platforms = if info.platforms.is_empty() {
                        fm.platform.clone().unwrap_or_default()
                    } else {
                        info.platforms
                    };
                    let mut tags = fm.tags.clone();
                    tags.extend(info.tags);
                    entries.push(Entry {
                        title: title.clone(),
                        description: desc.join(" ").trim().to_string(),
                        lang: info.lang,
                        command,
                        notebook: name.clone(),
                        tags,
                        platforms,
                        confirm: info.confirm,
                        remote: info.remote,
                        source: path.to_path_buf(),
                        line: fence_start,
                        trusted: true,
                    });
                }
            } else {
                // Strip the fence's own indentation
                let s = if fence_indent > 0 && raw_line.len() >= fence_indent {
                    let (lead, rest) = raw_line.split_at(fence_indent);
                    if lead.trim().is_empty() {
                        rest
                    } else {
                        raw_line
                    }
                } else {
                    raw_line
                };
                buf.push(s.to_string());
            }
            continue;
        }

        if let Some((indent, info)) = fence_marker(raw_line) {
            in_fence = true;
            fence_ticks = raw_line
                .trim_start()
                .chars()
                .take_while(|c| *c == '`')
                .count();
            fence_indent = indent;
            fence_info = info.to_string();
            fence_start = lineno;
            continue;
        }

        let t = raw_line.trim();
        if let Some(h) = t.strip_prefix("## ") {
            title = h.trim().to_string();
            desc.clear();
        } else if t.starts_with("# ") {
            // A top-level heading is the document title; ignore it
        } else if !t.is_empty() && !title.is_empty() {
            desc.push(t.to_string());
        }
    }

    Ok(Notebook {
        name,
        description: fm.description.unwrap_or_default(),
        path: path.to_path_buf(),
        tags: fm.tags,
        vars: fm.vars,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"---
name: demo
description: example
tags: [t1]
vars:
  service:
    desc: service name
    from: profile
---

## First

One line of description.

```sh @platform=linux @confirm @tags=deploy
sudo systemctl restart {{service}}
```

## Second

```ps1 @platform=windows
Get-Service
```
"#;

    fn nb() -> Notebook {
        parse(Path::new("demo.md"), SAMPLE).unwrap()
    }

    #[test]
    fn parses_frontmatter() {
        let n = nb();
        assert_eq!(n.name, "demo");
        assert_eq!(n.description, "example");
        assert_eq!(n.vars["service"].source(), "profile");
    }

    #[test]
    fn parses_entries() {
        let n = nb();
        assert_eq!(n.entries.len(), 2);
        let e = &n.entries[0];
        assert_eq!(e.title, "First");
        assert_eq!(e.description, "One line of description.");
        assert_eq!(e.command, "sudo systemctl restart {{service}}");
        assert_eq!(e.lang, "sh");
        assert!(e.confirm);
        assert_eq!(e.platforms, vec!["linux"]);
        assert!(e.tags.contains(&"deploy".to_string()));
        assert!(e.tags.contains(&"t1".to_string()));
    }

    #[test]
    fn platform_filtering() {
        let n = nb();
        assert!(n.entries[0].visible_on("linux"));
        assert!(!n.entries[0].visible_on("windows"));
        assert!(n.entries[1].visible_on("windows"));
    }

    #[test]
    fn notebook_level_platform_is_inherited() {
        let src = "---\nname: x\nplatform: [windows]\n---\n\n## a\n\n```ps1\nls\n```\n";
        let n = parse(Path::new("x.md"), src).unwrap();
        assert_eq!(n.entries[0].platforms, vec!["windows"]);
    }

    #[test]
    fn multiline_commands_survive() {
        let src = "---\nname: x\n---\n\n## a\n\n```sh\nline1\nline2\n```\n";
        let n = parse(Path::new("x.md"), src).unwrap();
        assert_eq!(n.entries[0].command, "line1\nline2");
        assert!(n.entries[0].is_multiline());
    }

    #[test]
    fn no_frontmatter_is_ok() {
        let src = "## a\n\n```sh\nls\n```\n";
        let n = parse(Path::new("plain.md"), src).unwrap();
        assert_eq!(n.name, "plain");
        assert_eq!(n.entries.len(), 1);
    }

    #[test]
    fn code_block_without_heading_is_skipped() {
        let src = "---\nname: x\n---\n\n```sh\nls\n```\n";
        let n = parse(Path::new("x.md"), src).unwrap();
        assert!(n.entries.is_empty());
    }
}
