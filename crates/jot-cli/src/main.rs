//! jot - a notebook for your own commands.
//!
//! The core promise: jot only puts a command on your prompt. It never runs
//! anything - pressing Enter is always the human's job.

mod console;
mod locale;
mod shellinit;
mod tui;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use jot_core::builtin;
use jot_core::capture;
use jot_core::notebook::Entry;
use jot_core::resolve::{self, Ask};
use jot_core::t;
use jot_core::vars;
use jot_core::{Config, Library, Paths, Profiles, Usage};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use tui::{Picked, Ui};

/// Exit code for a user cancel; the shell widget reads it as "leave the line alone".
const EXIT_CANCEL: i32 = 130;

const SUBCOMMANDS: &[&str] = &[
    "pick",
    "save",
    "ls",
    "list",
    "init",
    "edit",
    "new",
    "use",
    "profile",
    "import",
    "doctor",
    "path",
    "help",
    "add",
    "sync",
    "sources",
    "remove",
    "trust",
    "untrust",
    "lang",
    "notebooks",
    "notebook",
    "rename",
];

#[derive(Parser)]
#[command(
    name = "jot",
    version,
    about = t!("命令笔记本 —— 存你自己的命令，随手调出来", "A notebook for your own commands, one keypress away"),
    long_about = t!("命令笔记本。\n\n直接运行 `jot` 打开选择器，或 `jot docker log` 带词搜索。\n选中之后 jot 把命令填到你的命令行上，回车由你自己按。", "A notebook for your own commands.\n\nRun `jot` for the picker, or `jot docker log` to search straight away.\nOnce you pick one, jot types it onto your prompt - you press Enter.")
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Open the picker (the default)
    Pick {
        /// Initial search term
        #[arg(long, short, default_value = "")]
        query: String,
        /// Invoked by the shell widget: only the result goes to stdout
        #[arg(long)]
        widget: bool,
        /// The widget's current line, used as the initial search term
        #[arg(long, default_value = "")]
        line: String,
        /// No UI, take the best match (for scripts). Errors if a variable cannot be filled.
        #[arg(long)]
        first: bool,
    },
    /// Save a command; with no argument, the last one from shell history
    Save {
        command: Vec<String>,
        /// Which personal notebook to save into
        #[arg(long, short)]
        notebook: Option<String>,
        /// Title. Given here, jot saves without opening the picker
        #[arg(long, short)]
        title: Option<String>,
        /// Description, only used alongside --title
        #[arg(long, short)]
        desc: Option<String>,
        /// Mark it dangerous, so using it asks for confirmation
        #[arg(long)]
        confirm: bool,
        /// Restrict to a platform: windows, linux or macos
        #[arg(long)]
        platform: Option<String>,
        /// Extra search keywords, comma separated
        #[arg(long)]
        tags: Option<String>,
    },
    /// List every entry
    #[command(alias = "list")]
    Ls {
        #[arg(long, short)]
        notebook: Option<String>,
    },
    /// Print the shell integration script
    Init {
        /// powershell | bash | zsh | fish
        shell: String,
        /// Custom key binding
        #[arg(long)]
        key: Option<String>,
    },
    /// Open a notebook in your editor
    Edit { notebook: Option<String> },
    /// Create a personal notebook
    New { name: String },
    /// Switch profile
    Use { profile: String },
    /// Show or set profile variables
    Profile {
        #[command(subcommand)]
        action: Option<ProfileCmd>,
    },
    /// Import from shell history
    Import {
        #[command(subcommand)]
        what: ImportCmd,
    },
    /// Install a community notebook source (a git repository)
    Add {
        /// A git URL, or the gh:user/repo / gl:user/repo shorthand
        url: String,
        /// Override the local name
        #[arg(long)]
        name: Option<String>,
    },
    /// Update sources; all of them if no name is given
    Sync { name: Option<String> },
    /// List installed community sources
    Sources,
    /// Uninstall a community source
    Remove { name: String },
    /// Trust a source, allowing its from: shell variables to run
    Trust { name: String },
    /// Withdraw trust
    Untrust { name: String },
    /// List the notebooks, with the @name to filter by in the picker
    #[command(alias = "notebook")]
    Notebooks {
        /// Just the names, one per line, for scripts and variable sources
        #[arg(long)]
        names: bool,
    },
    /// Rename one of your own notebooks
    Rename { from: String, to: String },
    /// Show or set the interface language
    Lang {
        /// en | zh | auto
        value: Option<String>,
    },
    /// Self-check
    Doctor,
    /// Print the data directory
    Path,
}

#[derive(Subcommand)]
enum ProfileCmd {
    /// Set a variable
    Set { key: String, value: String },
    /// Remove a variable
    Unset { key: String },
    /// List every profile
    List,
}

#[derive(Subcommand)]
enum ImportCmd {
    /// Import from shell history, ranked by how often you use each command
    History {
        #[arg(long, default_value_t = 60)]
        top: usize,
    },
}

/// `jot docker logs` -> `jot pick --query "docker logs"`, so you need not
///
/// remember the subcommand. It stops at the first flag: the `--first` in
/// `jot docker logs --first` is an option, not part of the search term.
fn rewrite_bare_query(args: Vec<String>) -> Vec<String> {
    let Some(first) = args.get(1) else {
        return args;
    };
    if first.starts_with('-') || SUBCOMMANDS.contains(&first.as_str()) {
        return args;
    }
    let split = args[1..]
        .iter()
        .position(|a| a.starts_with('-'))
        .map(|i| i + 1)
        .unwrap_or(args.len());

    let mut out = vec![args[0].clone(), "pick".into(), "--query".into()];
    out.push(args[1..split].join(" "));
    out.extend_from_slice(&args[split..]);
    out
}

fn main() {
    // Scope kept tight on purpose: process::exit skips destructors, and the
    // console code page has to be restored before we leave.
    let code = {
        // Legacy conhost may be on a regional code page, which mangles UTF-8
        let _console = console::Utf8Console::enter();
        match run() {
            Ok(code) => code,
            Err(e) => {
                report_error(&e);
                1
            }
        }
    };
    std::process::exit(code);
}

/// Put an error where the user will actually see it.
///
/// stderr is a pipe whenever a shell widget captures us, and a message
/// swallowed by a pipe is exactly how a key binding ends up failing in
/// silence. When that happens, write to the terminal device as well.
fn report_error(e: &anyhow::Error) {
    let msg = format!("jot: {e:#}");
    eprintln!("{msg}");

    // Only for a shell widget: it may be capturing stderr, and silencing it
    // any other way (`jot 2>/dev/null`) is the caller's deliberate choice.
    let from_widget = std::env::var("JOT_WIDGET").is_ok();
    if from_widget && !std::io::IsTerminal::is_terminal(&std::io::stderr()) {
        if let Some(mut console) = tui::open_console() {
            use std::io::Write;
            let _ = writeln!(console, "{msg}");
        }
    }
}

fn run() -> Result<i32> {
    // Pin the language before anything prints, including clap's own help text
    if let Ok(paths) = Paths::discover() {
        locale::resolve(&Config::load(&paths));
    }
    let cli = Cli::parse_from(rewrite_bare_query(std::env::args().collect()));
    match cli.cmd {
        None => cmd_pick("", false, "", false),
        Some(Cmd::Pick {
            query,
            widget,
            line,
            first,
        }) => cmd_pick(&query, widget, &line, first),
        Some(Cmd::Save {
            command,
            notebook,
            title,
            desc,
            confirm,
            platform,
            tags,
        }) => cmd_save(SaveArgs {
            command,
            notebook,
            title,
            desc,
            confirm,
            platform,
            tags,
        }),
        Some(Cmd::Ls { notebook }) => cmd_ls(notebook.as_deref()),
        Some(Cmd::Init { shell, key }) => cmd_init(&shell, key.as_deref()),
        Some(Cmd::Edit { notebook }) => cmd_edit(notebook.as_deref()),
        Some(Cmd::New { name }) => cmd_new(&name),
        Some(Cmd::Use { profile }) => cmd_use(&profile),
        Some(Cmd::Profile { action }) => cmd_profile(action),
        Some(Cmd::Import { what }) => match what {
            ImportCmd::History { top } => cmd_import_history(top),
        },
        Some(Cmd::Add { url, name }) => cmd_add(&url, name.as_deref()),
        Some(Cmd::Sync { name }) => cmd_sync(name.as_deref()),
        Some(Cmd::Sources) => cmd_sources(),
        Some(Cmd::Remove { name }) => cmd_remove(&name),
        Some(Cmd::Trust { name }) => cmd_trust(&name, true),
        Some(Cmd::Untrust { name }) => cmd_trust(&name, false),
        Some(Cmd::Notebooks { names }) => cmd_notebooks(names),
        Some(Cmd::Rename { from, to }) => cmd_rename(&from, &to),
        Some(Cmd::Lang { value }) => cmd_lang(value.as_deref()),
        Some(Cmd::Doctor) => cmd_doctor(),
        Some(Cmd::Path) => cmd_path(),
    }
}

// ─────────────────────────── pick ───────────────────────────

fn cmd_pick(query: &str, widget: bool, line: &str, first: bool) -> Result<i32> {
    let paths = Paths::discover()?;
    let seeded = builtin::seed_if_missing(&paths)?;
    if seeded > 0 && !widget {
        eprintln!(
            "{}",
            t!(
                "jot: 已装好 {seeded} 个内置笔记本 → {}",
                "jot: installed {seeded} built-in notebooks -> {}",
                paths.builtin_dir().display()
            )
        );
    }

    let lib = Library::load(&paths)?;
    let cfg = Config::load(&paths);
    let profiles = Profiles::load(&paths);
    let mut usage = Usage::load(&paths);
    let entries = lib.entries();
    if entries.is_empty() {
        bail!(
            "{}",
            t!(
                "一条命令都没有。检查 {}",
                "no commands at all. Check {}",
                paths.notebooks().display()
            )
        );
    }

    let initial = if !query.is_empty() { query } else { line };

    if first {
        return cmd_pick_first(
            &lib,
            &entries,
            initial,
            &profiles,
            cfg.profile_name(),
            &mut usage,
            &paths,
        );
    }

    let mut ui = Ui::new()?;
    let idx = match ui.pick(&entries, initial, &usage)? {
        Picked::Cancel => {
            drop(ui);
            return Ok(EXIT_CANCEL);
        }
        Picked::Edit(i) => {
            let path = entries[i].source.clone();
            drop(ui);
            open_editor(&path)?;
            return Ok(0);
        }
        Picked::Entry(i) => i,
    };
    let entry = entries[idx];
    let decls = lib.decls_for(entry);

    let mut values: HashMap<String, String> = HashMap::new();
    for r in vars::refs(&entry.command) {
        // @remote entries are used after ssh, where dynamic candidates would
        // still be evaluated locally, so they must be disabled
        let plan = resolve::plan(
            &r.name,
            r.default.as_deref(),
            decls.get(&r.name),
            &profiles,
            cfg.profile_name(),
            // Untrusted external sources have from: shell disabled (D-09), and so do
            // @remote entries, whose candidates would be evaluated locally
            entry.trusted && !entry.remote,
        );
        let context = vars::render(&entry.command, &values);
        let got = match plan {
            Ask::Resolved(v) => Some(v),
            Ask::Choose {
                label,
                options,
                default,
            } => ui.ask_choice(&context, &label, &options, default.as_deref())?,
            Ask::Text { label, default } => ui.ask_text(&context, &label, default.as_deref())?,
        };
        match got {
            Some(v) => {
                values.insert(r.name, v);
            }
            None => {
                drop(ui);
                return Ok(EXIT_CANCEL);
            }
        }
    }

    let final_cmd = vars::render(&entry.command, &values);
    if entry.confirm && !ui.confirm(&final_cmd)? {
        drop(ui);
        return Ok(EXIT_CANCEL);
    }
    drop(ui);

    // Record the use, so it floats up next time
    usage.record(&entry.id());
    let _ = usage.save(&paths);

    emit(&final_cmd, widget, entry, &paths, cfg);
    Ok(0)
}

/// Take the best match without a UI, for scripts.
/// Only variables that resolve on their own are accepted: profile, built-ins, inline defaults.
fn cmd_pick_first(
    lib: &Library,
    entries: &[&Entry],
    query: &str,
    profiles: &Profiles,
    profile: &str,
    usage: &mut Usage,
    paths: &Paths,
) -> Result<i32> {
    use fuzzy_matcher::skim::SkimMatcherV2;
    use fuzzy_matcher::FuzzyMatcher;

    if query.trim().is_empty() {
        bail!(
            "{}",
            t!("--first 需要一个搜索词", "--first needs a search term")
        );
    }
    let matcher = SkimMatcherV2::default().ignore_case();
    let best = entries
        .iter()
        .filter_map(|e| {
            let hay = e.haystack();
            let mut total = 0i64;
            for part in query.split_whitespace() {
                total += matcher.fuzzy_match(&hay, part)?;
            }
            Some((*e, total))
        })
        .max_by_key(|(_, s)| *s)
        .map(|(e, _)| e);
    let Some(entry) = best else {
        bail!(
            "{}",
            t!("没有匹配 «{query}» 的条目", "nothing matches «{query}»")
        );
    };

    let decls = lib.decls_for(entry);
    let mut values: HashMap<String, String> = HashMap::new();
    let mut missing: Vec<String> = Vec::new();
    for r in vars::refs(&entry.command) {
        let plan = resolve::plan(
            &r.name,
            r.default.as_deref(),
            decls.get(&r.name),
            profiles,
            profile,
            false,
        );
        match plan {
            Ask::Resolved(v) => {
                values.insert(r.name, v);
            }
            _ => match r.default.clone() {
                Some(d) => {
                    values.insert(r.name, d);
                }
                None => missing.push(r.name),
            },
        }
    }
    if !missing.is_empty() {
        bail!("{}", t!("「{}」还需要这些变量：{} —— --first 模式不能交互，先用 `jot profile set` 配好", "\"{}\" still needs: {} - --first cannot prompt, so set them with `jot profile set` first",
            entry.title,
            missing.join(", ")
        ));
    }

    if entry.confirm {
        eprintln!(
            "{}",
            t!(
                "jot: ⚠ 「{}」被标记为危险命令，确认后再执行",
                "jot: warning - \"{}\" is marked dangerous; check it before running",
                entry.title
            )
        );
    }
    usage.record(&entry.id());
    let _ = usage.save(paths);

    println!("{}", vars::render(&entry.command, &values));
    eprintln!("jot: {} / {}", entry.notebook, entry.title);
    Ok(0)
}

/// Deliver the final command. jot stops here: no execution, no Enter.
fn emit(cmd: &str, widget: bool, entry: &Entry, paths: &Paths, mut cfg: Config) {
    println!("{cmd}");
    if widget {
        return;
    }
    match copy_to_clipboard(cmd) {
        Ok(()) => eprintln!(
            "{}",
            t!("jot: 已复制到剪贴板", "jot: copied to the clipboard")
        ),
        Err(_) => eprintln!(
            "{}",
            t!(
                "jot: 剪贴板不可用，命令已打印在上面",
                "jot: no clipboard available; the command is printed above"
            )
        ),
    }
    if entry.is_reference() {
        eprintln!(
            "{}",
            t!(
                "jot: 这是一条参考内容，不是可直接执行的命令",
                "jot: this is reference material, not a runnable command"
            )
        );
    }
    // Suggest the shell integration only a few times, then stop. The config
    // write is therefore confined to those first few runs.
    if cfg.hints_shown < jot_core::config::HINT_LIMIT {
        let shell = if cfg!(target_os = "windows") {
            "powershell"
        } else {
            "bash"
        };
        eprintln!("{}", t!("jot: 装上 shell 集成就能直接填进命令行 → jot init {shell}", "jot: install the shell integration to type straight onto your prompt -> jot init {shell}"));
        cfg.hints_shown += 1;
        let _ = cfg.save(paths);
    }
}

fn copy_to_clipboard(s: &str) -> Result<()> {
    let mut cb = arboard::Clipboard::new()?;
    cb.set_text(s.to_string())?;
    Ok(())
}

// ─────────────────────────── save ───────────────────────────

struct SaveArgs {
    command: Vec<String>,
    notebook: Option<String>,
    title: Option<String>,
    desc: Option<String>,
    confirm: bool,
    platform: Option<String>,
    tags: Option<String>,
}

fn cmd_save(args: SaveArgs) -> Result<i32> {
    let SaveArgs {
        command,
        notebook,
        title,
        desc,
        confirm,
        platform,
        tags,
    } = args;
    let paths = Paths::discover()?;
    let cfg = Config::load(&paths);
    let profiles = Profiles::load(&paths);

    let raw = if command.is_empty() {
        capture::last_command()
            .context(t!("shell 历史里没找到可用的命令。直接写：jot save \"你的命令\"", "no usable command found in your shell history. Pass one instead: jot save \"your command\""))?
    } else {
        command.join(" ")
    };

    if capture::looks_secret(&raw) {
        bail!("{}", t!("这条命令看起来含有密钥或口令，jot 拒绝存它：\n  {raw}", "this looks like it contains a secret or password, so jot will not store it:\n  {raw}"));
    }

    // Reverse parameterization: swap in variables for values from the profile
    let (parameterized, applied) = capture::parameterize(&raw, &profiles, cfg.profile_name());

    // With a title given there is nothing to ask, so this stays scriptable
    // and usable from a shell that cannot host a picker.
    let (title, desc) = match title {
        Some(t) if !t.trim().is_empty() => (t, desc.unwrap_or_default()),
        _ => {
            let mut ui = Ui::new()?;
            let asked = ui.ask_text(
                &parameterized,
                t!("标题", "Title").as_ref(),
                Some(&capture::guess_title(&raw)),
            )?;
            let Some(t) = asked.filter(|t| !t.trim().is_empty()) else {
                drop(ui);
                return Ok(EXIT_CANCEL);
            };
            let d = ui
                .ask_text(
                    &parameterized,
                    t!("说明（可留空）", "Description (optional)").as_ref(),
                    Some(""),
                )?
                .unwrap_or_default();
            drop(ui);
            (t, d)
        }
    };

    // An explicit platform settles the fence language: a linux entry is not
    // PowerShell just because it was saved from a Windows box.
    let guessed = capture::guess_lang(&raw);
    let mut fence = match platform.as_deref() {
        Some("linux") | Some("macos") if guessed != "sql" => "sh".to_string(),
        Some("windows") if guessed != "sql" => "ps1".to_string(),
        _ => guessed.to_string(),
    };
    if let Some(p) = platform.as_deref() {
        fence.push_str(&format!(" @platform={p}"));
    }
    if confirm {
        fence.push_str(" @confirm");
    }
    if let Some(tg) = tags.as_deref() {
        fence.push_str(&format!(" @tags={tg}"));
    }

    let path = capture::append(
        &paths,
        notebook.as_deref(),
        &title,
        &desc,
        &fence,
        &parameterized,
    )?;

    eprintln!(
        "{}",
        t!("jot: 已存入 {}", "jot: saved to {}", path.display())
    );
    if !applied.is_empty() {
        eprintln!(
            "{}",
            t!(
                "jot: 自动参数化了 {} → {}",
                "jot: parameterized {} -> {}",
                applied.join(", "),
                parameterized
            )
        );
    }
    Ok(0)
}

// ─────────────────────────── import ───────────────────────────

fn cmd_import_history(top: usize) -> Result<i32> {
    let paths = Paths::discover()?;
    let cfg = Config::load(&paths);
    let profiles = Profiles::load(&paths);

    let files = capture::history_files();
    if files.is_empty() {
        bail!(
            "{}",
            t!("没找到任何 shell 历史文件", "no shell history file found")
        );
    }
    let items = capture::history_ranked(top);
    if items.is_empty() {
        bail!(
            "{}",
            t!(
                "历史里没有值得导入的命令",
                "nothing in the history worth importing"
            )
        );
    }

    let rows: Vec<String> = items
        .iter()
        .map(|i| format!("{:>3}×  {}", i.count, i.command))
        .collect();

    let mut ui = Ui::new()?;
    let picked = ui.multi_select(
        t!(
            "从 shell 历史导入（按使用频次排序）",
            "Import from shell history (most used first)"
        )
        .as_ref(),
        &rows,
    )?;
    drop(ui);

    let Some(picked) = picked else {
        return Ok(EXIT_CANCEL);
    };
    if picked.is_empty() {
        eprintln!("{}", t!("jot: 没有选中任何命令", "jot: nothing selected"));
        return Ok(0);
    }

    let mut n = 0;
    for i in picked {
        let raw = &items[i].command;
        let (parameterized, _) = capture::parameterize(raw, &profiles, cfg.profile_name());
        capture::append(
            &paths,
            None,
            &capture::guess_title(raw),
            "",
            capture::guess_lang(raw),
            &parameterized,
        )?;
        n += 1;
    }
    eprintln!(
        "{}",
        t!(
            "jot: 导入了 {n} 条 → {}",
            "jot: imported {n} -> {}",
            builtin::ensure_personal_notebook(&paths)?.display()
        )
    );
    eprintln!(
        "{}",
        t!(
            "jot: 建议现在跑一次 `jot edit my` 把标题改成你看得懂的话",
            "jot: run `jot edit my` now and rewrite the titles into something you will recognise"
        )
    );
    Ok(0)
}

// ─────────────────────────── other subcommands ───────────────────────────

fn cmd_ls(notebook: Option<&str>) -> Result<i32> {
    let paths = Paths::discover()?;
    let lib = Library::load(&paths)?;
    let plat = jot_core::notebook::current_platform();
    for nb in &lib.notebooks {
        if let Some(f) = notebook {
            if nb.name != f {
                continue;
            }
        }
        if nb.entries.is_empty() && notebook.is_none() {
            continue;
        }
        println!(
            "{}",
            t!(
                "\n# {}  ({} 条)  {}",
                "\n# {}  ({} entries)  {}",
                nb.name,
                nb.entries.len(),
                nb.description
            )
        );
        for e in &nb.entries {
            // A platform is a label, not a filter: you may well be running a
            // linux command from Windows over ssh, or inside WSL. Square
            // brackets mark the ones that will not run here as-is.
            let tag = match e.platform_label() {
                Some(p) if !e.runs_on(plat) => format!("  [{p}]"),
                Some(p) => format!("  ({p})"),
                None => String::new(),
            };
            println!("  {:<40} {}{}", e.title, vars::preview(&e.command), tag);
        }
    }
    Ok(0)
}

fn cmd_init(shell: &str, key: Option<&str>) -> Result<i32> {
    match shellinit::script(&shell.to_ascii_lowercase(), key) {
        Some(s) => {
            print!("{s}");
            Ok(0)
        }
        None => bail!(
            "{}",
            t!(
                "不认识的 shell «{shell}»，支持：{}",
                "unknown shell «{shell}». Supported: {}",
                shellinit::SHELLS.join(" / ")
            )
        ),
    }
}

fn cmd_edit(notebook: Option<&str>) -> Result<i32> {
    let paths = Paths::discover()?;
    builtin::seed_if_missing(&paths)?;
    let path = match notebook {
        Some(n) => {
            let local = paths.local_dir().join(format!("{n}.md"));
            let builtin_p = paths.builtin_dir().join(format!("{n}.md"));
            if local.exists() {
                local
            } else if builtin_p.exists() {
                builtin_p
            } else {
                bail!(
                    "{}",
                    t!(
                        "没有叫 «{n}» 的笔记本。已有的看 `jot ls`",
                        "no notebook called «{n}». See `jot ls` for what you have"
                    )
                );
            }
        }
        None => builtin::ensure_personal_notebook(&paths)?,
    };
    open_editor(&path)?;
    Ok(0)
}

fn cmd_new(name: &str) -> Result<i32> {
    let paths = Paths::discover()?;
    paths.ensure()?;
    let path = paths.local_dir().join(format!("{name}.md"));
    if path.exists() {
        bail!(
            "{}",
            t!("{} 已经存在了", "{} already exists", path.display())
        );
    }
    // The format reference lives in a comment: it should teach the syntax
    // without adding a dummy entry to everyone's search results.
    let template = t!(
        "---\nname: {name}\ndescription: \ntags: []\n---\n\n\
<!--\n\
格式参考 —— 写完自己的条目后可以把这段删掉。\n\n\
## 条目标题\n\n\
说明写在这里：为什么用、有什么坑。\n\n\
```sh @platform=linux @confirm @tags=deploy\n\
sudo systemctl restart {{{{service}}}}\n\
```\n\n\
属性：@platform=windows|linux|macos   @confirm   @remote   @tags=a,b\n\
变量：{{{{service}}}} 每次问你；名字和 Profile 里的键一致就自动代入\n\
-->\n",
        "---\nname: {name}\ndescription: \ntags: []\n---\n\n\
<!--\n\
Format reference - delete this once you have entries of your own.\n\n\
## Entry title\n\n\
The description goes here: why you use it, what to watch out for.\n\n\
```sh @platform=linux @confirm @tags=deploy\n\
sudo systemctl restart {{{{service}}}}\n\
```\n\n\
Attributes: @platform=windows|linux|macos   @confirm   @remote   @tags=a,b\n\
Variables:  {{{{service}}}} asks you each time, or resolves from your profile\n\
-->\n",
        name = name
    );
    std::fs::write(&path, template.as_ref())?;
    eprintln!(
        "{}",
        t!("jot: 建好了 {}", "jot: created {}", path.display())
    );
    open_editor(&path)?;
    Ok(0)
}

fn cmd_use(profile: &str) -> Result<i32> {
    let paths = Paths::discover()?;
    let mut cfg = Config::load(&paths);
    cfg.profile = Some(profile.to_string());
    cfg.save(&paths)?;
    let profiles = Profiles::load(&paths);
    eprintln!(
        "{}",
        t!(
            "jot: 当前 Profile → {profile}",
            "jot: active profile -> {profile}"
        )
    );
    let entries = profiles.entries(profile);
    if entries.is_empty() {
        eprintln!("{}", t!("jot: 这个 Profile 还没有变量，用 `jot profile set 键 值` 加一个", "jot: this profile has no variables yet; add one with `jot profile set <key> <value>`"));
    } else {
        for (k, v) in entries {
            eprintln!("  {k} = {v}");
        }
    }
    Ok(0)
}

fn cmd_profile(action: Option<ProfileCmd>) -> Result<i32> {
    let paths = Paths::discover()?;
    let cfg = Config::load(&paths);
    let mut profiles = Profiles::load(&paths);
    let current = cfg.profile_name().to_string();

    match action {
        None => {
            println!(
                "{}",
                t!("当前 Profile: {current}", "Active profile: {current}")
            );
            let entries = profiles.entries(&current);
            if entries.is_empty() {
                println!("{}", t!("（还没有变量）", "(no variables yet)"));
            }
            for (k, v) in entries {
                println!("  {k} = {v}");
            }
        }
        Some(ProfileCmd::List) => {
            for name in profiles.names() {
                let mark = if name == current { "*" } else { " " };
                println!("{mark} {name}");
            }
        }
        Some(ProfileCmd::Set { key, value }) => {
            profiles.set(&current, &key, &value);
            profiles.save(&paths)?;
            eprintln!("jot: [{current}] {key} = {value}");
        }
        Some(ProfileCmd::Unset { key }) => {
            if let Some(m) = profiles.0.get_mut(&current) {
                m.remove(&key);
            }
            profiles.save(&paths)?;
            eprintln!(
                "{}",
                t!(
                    "jot: [{current}] 已删除 {key}",
                    "jot: [{current}] removed {key}"
                )
            );
        }
    }
    Ok(0)
}

// ─────────────────────────── community sources ───────────────────────────

fn cmd_add(url: &str, name: Option<&str>) -> Result<i32> {
    let paths = Paths::discover()?;
    let src = jot_core::sources::add(&paths, url, name)?;

    let lib = Library::load(&paths)?;
    let dir = jot_core::sources::notebook_dir(&src.path);
    let n: usize = lib
        .notebooks
        .iter()
        .filter(|nb| nb.path.starts_with(&dir))
        .map(|nb| nb.entries.len())
        .sum();

    eprintln!(
        "{}",
        t!(
            "jot: 装好 «{}» → {} 条命令",
            "jot: installed «{}» -> {} commands",
            src.name,
            n
        )
    );
    if n == 0 {
        eprintln!("{}", t!("jot: 这个仓库里没找到可用的笔记本。jot 会看 notebooks/ 子目录，没有就看仓库根的 *.md", "jot: no usable notebooks in that repository. jot looks in notebooks/, falling back to *.md at the repo root"
        ));
    }
    eprintln!(
        "{}",
        t!(
            "jot: 外部源的动态变量（from: shell）默认禁用 —— 那是任意代码执行。\n     看过内容确认没问题后用 `jot trust {}` 打开。",
            "jot: dynamic variables (from: shell) are disabled for external sources -\n     that is arbitrary code execution. Read the notebook first, then allow\n     it with `jot trust {}`.",
            src.name
        )
    );
    Ok(0)
}

fn cmd_sync(name: Option<&str>) -> Result<i32> {
    let paths = Paths::discover()?;
    let cfg = Config::load(&paths);
    let all = jot_core::sources::list(&paths, &cfg.trusted_sources);
    if all.is_empty() {
        bail!(
            "{}",
            t!(
                "还没装任何社区源。用 `jot add gh:user/repo` 装一个",
                "no community sources installed. Add one with `jot add gh:user/repo`"
            )
        );
    }
    let targets: Vec<_> = match name {
        Some(n) => all.into_iter().filter(|s| s.name == n).collect(),
        None => all,
    };
    if targets.is_empty() {
        bail!(
            "{}",
            t!(
                "没有叫 «{}» 的源",
                "no source called «{}»",
                name.unwrap_or("")
            )
        );
    }
    for src in &targets {
        match jot_core::sources::sync(src) {
            Ok(true) => eprintln!("{}", t!("jot: {} 已更新", "jot: {} updated", src.name)),
            Ok(false) => eprintln!(
                "{}",
                t!("jot: {} 已是最新", "jot: {} already up to date", src.name)
            ),
            Err(e) => eprintln!(
                "{}",
                t!(
                    "jot: {} 更新失败：{e}",
                    "jot: {} could not be updated: {e}",
                    src.name
                )
            ),
        }
    }
    Ok(0)
}

fn cmd_sources() -> Result<i32> {
    let paths = Paths::discover()?;
    let cfg = Config::load(&paths);
    let lib = Library::load(&paths)?;
    let all = jot_core::sources::list(&paths, &cfg.trusted_sources);
    if all.is_empty() {
        println!(
            "{}",
            t!("还没装任何社区源。", "No community sources installed.")
        );
        println!("  jot add gh:user/repo");
        return Ok(0);
    }
    for src in all {
        let dir = jot_core::sources::notebook_dir(&src.path);
        let n: usize = lib
            .notebooks
            .iter()
            .filter(|nb| nb.path.starts_with(&dir))
            .map(|nb| nb.entries.len())
            .sum();
        println!(
            "{}",
            t!(
                "{:<20} {:>4} 条   {}   {}",
                "{:<20} {:>4}   {}   {}",
                src.name,
                n,
                if src.trusted {
                    t!("已授信", "trusted")
                } else {
                    t!("未授信", "untrusted")
                },
                src.remote_url().unwrap_or_default()
            )
        );
    }
    Ok(0)
}

fn cmd_remove(name: &str) -> Result<i32> {
    let paths = Paths::discover()?;
    jot_core::sources::remove(&paths, name)?;
    let mut cfg = Config::load(&paths);
    cfg.trusted_sources.retain(|t| t != name);
    cfg.save(&paths)?;
    eprintln!("{}", t!("jot: 已卸载 «{name}»", "jot: removed «{name}»"));
    Ok(0)
}

fn cmd_trust(name: &str, on: bool) -> Result<i32> {
    let paths = Paths::discover()?;
    let mut cfg = Config::load(&paths);
    let exists = jot_core::sources::list(&paths, &cfg.trusted_sources)
        .iter()
        .any(|s| s.name == name);
    if !exists {
        bail!(
            "{}",
            t!(
                "没有叫 «{name}» 的源。已装的看 `jot sources`",
                "no source called «{name}». See `jot sources` for what you have"
            )
        );
    }
    cfg.trusted_sources.retain(|t| t != name);
    if on {
        cfg.trusted_sources.push(name.to_string());
    }
    cfg.save(&paths)?;
    eprintln!(
        "jot: «{name}» {}",
        if on {
            t!(
                "已授信 —— 它的 from: shell 变量现在会真的执行",
                "trusted - its from: shell variables will now actually run"
            )
        } else {
            t!("已撤销授信", "no longer trusted")
        }
    );
    Ok(0)
}

fn cmd_rename(from: &str, to: &str) -> Result<i32> {
    let paths = Paths::discover()?;
    let src = paths.local_dir().join(format!("{from}.md"));
    let dst = paths.local_dir().join(format!("{to}.md"));

    if !src.is_file() {
        bail!(
            "{}",
            t!(
                "«{from}» 不是你自己的笔记本。只有 local/ 下面的能改名，内置和社区源不行",
                "«{from}» is not one of your own notebooks. Only those under local/ can be renamed"
            )
        );
    }
    if dst.exists() {
        bail!(
            "{}",
            t!("{} 已经存在了", "{} already exists", dst.display())
        );
    }

    // The frontmatter name is what the picker and @filters use, so it has to
    // move with the file or the two would disagree.
    let text = std::fs::read_to_string(&src)?;
    let renamed = text
        .lines()
        .map(|l| {
            if l.trim_start().starts_with("name:") && l.trim() == format!("name: {from}") {
                format!("name: {to}")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    std::fs::write(
        &dst,
        renamed
            + "
",
    )?;
    std::fs::remove_file(&src)?;

    eprintln!(
        "{}",
        t!("jot: «{from}» → «{to}»", "jot: «{from}» -> «{to}»")
    );
    Ok(0)
}

fn cmd_notebooks(names_only: bool) -> Result<i32> {
    let paths = Paths::discover()?;
    let lib = Library::load(&paths)?;

    if names_only {
        let mut names: Vec<&str> = lib.notebooks.iter().map(|n| n.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        for name in names {
            println!("{name}");
        }
        return Ok(0);
    }

    println!(
        "{}",
        t!(
            "在选择器里输入 @名字 就能只看这一本，#标签 按标签筛",
            "Type @name in the picker to see only that notebook, #tag to filter by tag"
        )
    );
    println!();

    let mut rows: Vec<(String, usize, String)> = lib
        .notebooks
        .iter()
        .map(|n| (n.name.clone(), n.entries.len(), n.description.clone()))
        .collect();

    rows.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, visible, description) in &rows {
        println!("  @{name:<14} {visible:>4}   {description}");
    }

    let total: usize = rows.iter().map(|(_, n, _)| n).sum();
    println!();
    println!(
        "{}",
        t!(
            "  {} 本 · {} 条",
            "  {} notebooks, {} entries",
            rows.len(),
            total
        )
    );
    Ok(0)
}

fn cmd_lang(value: Option<&str>) -> Result<i32> {
    let paths = Paths::discover()?;
    let mut cfg = Config::load(&paths);

    let Some(value) = value else {
        println!(
            "{}",
            t!(
                "当前语言        {} （来自 {}）",
                "language        {} (from {})",
                jot_core::i18n::lang().code(),
                locale::source(&cfg)
            )
        );
        println!(
            "{}",
            t!(
                "可选            en / zh / auto",
                "options         en / zh / auto"
            )
        );
        return Ok(0);
    };

    let normalised = value.trim().to_ascii_lowercase();
    cfg.lang = match normalised.as_str() {
        "auto" | "" => None,
        other => match jot_core::Lang::parse(other) {
            Some(l) => Some(l.code().to_string()),
            None => bail!(t!(
                "不认识的语言 «{value}»，支持 en / zh / auto",
                "unknown language «{value}». Supported: en / zh / auto"
            )),
        },
    };
    cfg.save(&paths)?;

    // Re-resolve and re-seed so the built-in notebooks switch language too
    let lang = locale::resolve(&cfg);
    let seeded = builtin::seed_if_missing(&paths)?;
    eprintln!(
        "{}",
        t!(
            "jot: 语言 → {}，已切换 {} 个内置笔记本",
            "jot: language -> {}, switched {} built-in notebooks",
            lang.code(),
            seeded
        )
    );
    Ok(0)
}

fn cmd_path() -> Result<i32> {
    let paths = Paths::discover()?;
    println!("{}", paths.root.display());
    Ok(0)
}

fn cmd_doctor() -> Result<i32> {
    let paths = Paths::discover()?;
    let lib = Library::load(&paths)?;
    let cfg = Config::load(&paths);
    let profiles = Profiles::load(&paths);
    let plat = jot_core::notebook::current_platform();

    println!("jot {}", env!("CARGO_PKG_VERSION"));
    println!("{}", t!("平台            {plat}", "platform        {plat}"));
    println!(
        "{}",
        t!(
            "数据目录        {}",
            "data directory  {}",
            paths.root.display()
        )
    );
    println!(
        "{}",
        t!(
            "笔记本          {} 个",
            "notebooks       {}",
            lib.notebooks.len()
        )
    );
    println!(
        "{}",
        t!(
            "条目            {} 条（其中 {} 条可在本机直接跑）",
            "entries         {} ({} run as-is on this platform)",
            lib.entry_count(),
            lib.entries().iter().filter(|e| e.runs_on(plat)).count()
        )
    );
    println!(
        "{}",
        t!(
            "加载耗时        {:.1} ms",
            "load time       {:.1} ms",
            lib.load_ms
        )
    );
    println!(
        "{}",
        t!(
            "当前 Profile    {}",
            "profile         {}",
            cfg.profile_name()
        )
    );
    println!(
        "{}",
        t!(
            "Profile 变量    {} 个",
            "profile vars    {}",
            profiles.entries(cfg.profile_name()).len()
        )
    );

    println!(
        "{}",
        t!(
            "语言            {} （来自 {}）",
            "language        {} (from {})",
            jot_core::i18n::lang().code(),
            locale::source(&cfg)
        )
    );
    let usage = jot_core::Usage::load(&paths);
    println!(
        "{}",
        t!(
            "累计使用        {} 次",
            "total uses      {}",
            usage.total_uses()
        )
    );
    // Usage ids are command hashes, so resolve them back to titles for display
    let titles: std::collections::HashMap<String, String> = lib
        .notebooks
        .iter()
        .flat_map(|n| n.entries.iter())
        .map(|e| (e.id(), format!("{} / {}", e.notebook, e.title)))
        .collect();
    let top = usage.top(5);
    if !top.is_empty() {
        println!("{}", t!("最常用", "most used"));
        for (id, stat) in top {
            let label = titles.get(id).map(String::as_str).unwrap_or(id);
            println!("  {:>4}x  {label}", stat.count);
        }
    }

    let hist = capture::history_files();
    if hist.is_empty() {
        println!(
            "{}",
            t!(
                "shell 历史      没找到（`jot save` 需要手动给命令）",
                "shell history   none found (`jot save` will need the command passed in)"
            )
        );
    } else {
        for h in &hist {
            println!(
                "{}",
                t!("shell 历史      {}", "shell history   {}", h.display())
            );
        }
    }

    if lib.load_ms > 50.0 {
        println!(
            "{}",
            t!(
                "\n⚠ 加载超过 50ms 预算，widget 会感觉到卡。",
                "\nWarning: load exceeds the 50ms budget, so the shell widget will feel sluggish."
            )
        );
    }
    let local_count: usize = lib
        .notebooks
        .iter()
        .filter(|n| n.path.starts_with(paths.local_dir()))
        .map(|n| n.entries.len())
        .sum();
    if local_count == 0 {
        println!(
            "{}",
            t!(
                "\n你还没有存过自己的命令。试试：",
                "\nYou have not saved any of your own commands yet. Try:"
            )
        );
        println!("  jot import history --top 40");
        println!(
            "{}",
            t!(
                "  jot save \"你刚敲过的那条命令\"",
                "  jot save \"the command you just ran\""
            )
        );
    }
    Ok(0)
}

/// Open a notebook in the user's editor.
///
/// Deliberately does nothing when this is not an interactive session, or when
/// EDITOR is explicitly set to empty. A script or a test calling `jot new`
/// should not make a window appear on somebody's screen.
fn open_editor(path: &Path) -> Result<()> {
    let editor = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR"));
    if let Ok(ed) = &editor {
        if ed.trim().is_empty() {
            return Ok(());
        }
    }
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Ok(());
    }
    if let Ok(ed) = editor {
        Command::new(ed).arg(path).status()?;
        return Ok(());
    }
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .status()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(path).status()
    } else {
        Command::new("xdg-open").arg(path).status()
    };
    status.with_context(|| format!("打不开 {}，设置 $EDITOR 再试", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::rewrite_bare_query;

    fn rw(args: &[&str]) -> Vec<String> {
        rewrite_bare_query(
            std::iter::once("jot")
                .chain(args.iter().copied())
                .map(String::from)
                .collect(),
        )
    }

    #[test]
    fn bare_words_become_a_query() {
        assert_eq!(
            rw(&["docker", "logs"]),
            ["jot", "pick", "--query", "docker logs"]
        );
    }

    /// Regression: flags used to be swallowed into the search term, which
    /// broke `jot docker --first`.
    #[test]
    fn flags_after_a_bare_query_stay_flags() {
        assert_eq!(
            rw(&["docker", "logs", "--first"]),
            ["jot", "pick", "--query", "docker logs", "--first"]
        );
        assert_eq!(
            rw(&["git", "--first", "undo"]),
            ["jot", "pick", "--query", "git", "--first", "undo"]
        );
    }

    #[test]
    fn real_subcommands_are_left_alone() {
        for sub in ["save", "ls", "doctor", "init", "profile"] {
            assert_eq!(
                rw(&[sub])[1],
                sub,
                "subcommand {sub} was treated as a search term"
            );
        }
    }

    #[test]
    fn leading_flags_are_left_alone() {
        assert_eq!(rw(&["--version"]), ["jot", "--version"]);
        assert_eq!(rw(&["--help"]), ["jot", "--help"]);
    }

    #[test]
    fn no_args_is_untouched() {
        assert_eq!(rw(&[]), ["jot"]);
    }
}
