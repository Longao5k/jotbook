//! jot —— 命令笔记本。
//!
//! 核心约定：jot 只把命令放到你的命令行上，从不替你执行。回车永远由人按。

mod console;
mod shellinit;
mod tui;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use jot_core::builtin;
use jot_core::capture;
use jot_core::notebook::Entry;
use jot_core::resolve::{self, Ask};
use jot_core::vars;
use jot_core::{Config, Library, Paths, Profiles, Usage};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use tui::{Picked, Ui};

/// 用户取消时的退出码：shell widget 靠它判断「别动命令行」。
const EXIT_CANCEL: i32 = 130;

const SUBCOMMANDS: &[&str] = &[
    "pick", "save", "ls", "list", "init", "edit", "new", "use", "profile", "import", "doctor",
    "path", "help",
];

#[derive(Parser)]
#[command(
    name = "jot",
    version,
    about = "命令笔记本 —— 存你自己的命令，随手调出来",
    long_about = "命令笔记本。\n\n直接运行 `jot` 打开选择器，或 `jot docker log` 带词搜索。\n选中之后 jot 把命令填到你的命令行上，回车由你自己按。"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// 打开选择器（默认命令）
    Pick {
        /// 初始搜索词
        #[arg(long, short, default_value = "")]
        query: String,
        /// 由 shell widget 调起：只把结果打到 stdout
        #[arg(long)]
        widget: bool,
        /// widget 传入的当前命令行内容，用作初始搜索词
        #[arg(long, default_value = "")]
        line: String,
        /// 不开界面，直接取最佳匹配（脚本里用）。有变量填不上就报错。
        #[arg(long)]
        first: bool,
    },
    /// 存一条命令；不给参数就取 shell 历史里的最后一条
    Save {
        command: Vec<String>,
        /// 存到哪个个人笔记本
        #[arg(long, short)]
        notebook: Option<String>,
    },
    /// 列出全部条目
    #[command(alias = "list")]
    Ls {
        #[arg(long, short)]
        notebook: Option<String>,
    },
    /// 输出 shell 集成脚本
    Init {
        /// powershell | bash | zsh | fish
        shell: String,
        /// 自定义快捷键
        #[arg(long)]
        key: Option<String>,
    },
    /// 用编辑器打开笔记本
    Edit { notebook: Option<String> },
    /// 新建个人笔记本
    New { name: String },
    /// 切换 Profile
    Use { profile: String },
    /// 查看或设置 Profile 变量
    Profile {
        #[command(subcommand)]
        action: Option<ProfileCmd>,
    },
    /// 从 shell 历史导入
    Import {
        #[command(subcommand)]
        what: ImportCmd,
    },
    /// 自检
    Doctor,
    /// 打印数据目录
    Path,
}

#[derive(Subcommand)]
enum ProfileCmd {
    /// 设置一个变量
    Set { key: String, value: String },
    /// 删除一个变量
    Unset { key: String },
    /// 列出所有 Profile
    List,
}

#[derive(Subcommand)]
enum ImportCmd {
    /// 从 shell 历史按使用频次导入
    History {
        #[arg(long, default_value_t = 60)]
        top: usize,
    },
}

/// `jot docker 日志` → `jot pick --query "docker 日志"`，省掉记子命令。
///
/// 只吞到第一个 flag 为止 —— `jot docker 日志 --first` 里的 `--first` 是选项，
/// 不是搜索词的一部分。
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
    // 作用域刻意收紧：process::exit 不跑析构函数，代码页必须在退出前还原。
    let code = {
        // 老 conhost 的代码页可能是 GBK，中文界面会整片乱码
        let _console = console::Utf8Console::enter();
        match run() {
            Ok(code) => code,
            Err(e) => {
                eprintln!("jot: {e:#}");
                1
            }
        }
    };
    std::process::exit(code);
}

fn run() -> Result<i32> {
    let cli = Cli::parse_from(rewrite_bare_query(std::env::args().collect()));
    match cli.cmd {
        None => cmd_pick("", false, "", false),
        Some(Cmd::Pick {
            query,
            widget,
            line,
            first,
        }) => cmd_pick(&query, widget, &line, first),
        Some(Cmd::Save { command, notebook }) => cmd_save(command, notebook),
        Some(Cmd::Ls { notebook }) => cmd_ls(notebook.as_deref()),
        Some(Cmd::Init { shell, key }) => cmd_init(&shell, key.as_deref()),
        Some(Cmd::Edit { notebook }) => cmd_edit(notebook.as_deref()),
        Some(Cmd::New { name }) => cmd_new(&name),
        Some(Cmd::Use { profile }) => cmd_use(&profile),
        Some(Cmd::Profile { action }) => cmd_profile(action),
        Some(Cmd::Import { what }) => match what {
            ImportCmd::History { top } => cmd_import_history(top),
        },
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
            "jot: 已装好 {seeded} 个内置笔记本 → {}",
            paths.builtin_dir().display()
        );
    }

    let lib = Library::load(&paths)?;
    let cfg = Config::load(&paths);
    let profiles = Profiles::load(&paths);
    let mut usage = Usage::load(&paths);
    let entries = lib.entries();
    if entries.is_empty() {
        bail!("一条命令都没有。检查 {}", paths.notebooks().display());
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
        // @remote 的条目在 ssh 之后使用，动态候选会在本地求值，必须禁用
        let plan = resolve::plan(
            &r.name,
            r.default.as_deref(),
            decls.get(&r.name),
            &profiles,
            cfg.profile_name(),
            !entry.remote,
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

    // 用过就记一笔，下次它会自动往前排
    usage.record(&entry.id());
    let _ = usage.save(&paths);

    emit(&final_cmd, widget, entry, &paths, cfg);
    Ok(0)
}

/// 非交互取最佳匹配，给脚本用。
/// 变量只接受能自动确定的来源：Profile、内置变量、行内默认值。
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
        bail!("--first 需要一个搜索词");
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
        bail!("没有匹配 «{query}» 的条目");
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
        bail!(
            "「{}」还需要这些变量：{} —— --first 模式不能交互，先用 `jot profile set` 配好",
            entry.title,
            missing.join(", ")
        );
    }

    if entry.confirm {
        eprintln!("jot: ⚠ 「{}」被标记为危险命令，确认后再执行", entry.title);
    }
    usage.record(&entry.id());
    let _ = usage.save(paths);

    println!("{}", vars::render(&entry.command, &values));
    eprintln!("jot: {} / {}", entry.notebook, entry.title);
    Ok(0)
}

/// 交付最终命令。jot 到此为止 —— 不执行，不发回车。
fn emit(cmd: &str, widget: bool, entry: &Entry, paths: &Paths, mut cfg: Config) {
    println!("{cmd}");
    if widget {
        return;
    }
    match copy_to_clipboard(cmd) {
        Ok(()) => eprintln!("jot: 已复制到剪贴板"),
        Err(_) => eprintln!("jot: 剪贴板不可用，命令已打印在上面"),
    }
    if entry.is_reference() {
        eprintln!("jot: 这是一条参考内容，不是可直接执行的命令");
    }
    // 只在最初几次提示装 shell 集成，之后闭嘴。写配置也只发生在这几次。
    if cfg.hints_shown < jot_core::config::HINT_LIMIT {
        let shell = if cfg!(target_os = "windows") {
            "powershell"
        } else {
            "bash"
        };
        eprintln!("jot: 装上 shell 集成就能直接填进命令行 → jot init {shell}");
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

fn cmd_save(command: Vec<String>, notebook: Option<String>) -> Result<i32> {
    let paths = Paths::discover()?;
    let cfg = Config::load(&paths);
    let profiles = Profiles::load(&paths);

    let raw = if command.is_empty() {
        capture::last_command()
            .context("shell 历史里没找到可用的命令。直接写：jot save \"你的命令\"")?
    } else {
        command.join(" ")
    };

    if capture::looks_secret(&raw) {
        bail!("这条命令看起来含有密钥或口令，jot 拒绝存它：\n  {raw}");
    }

    // 反向参数化：命令里出现当前 Profile 的值就换成变量
    let (parameterized, applied) = capture::parameterize(&raw, &profiles, cfg.profile_name());

    let mut ui = Ui::new()?;
    let title = match ui.ask_text(&parameterized, "标题", Some(&capture::guess_title(&raw)))? {
        Some(t) if !t.trim().is_empty() => t,
        _ => {
            drop(ui);
            return Ok(EXIT_CANCEL);
        }
    };
    let desc = ui
        .ask_text(&parameterized, "说明（可留空）", Some(""))?
        .unwrap_or_default();
    drop(ui);

    let lang = capture::guess_lang(&raw);
    let path = capture::append(
        &paths,
        notebook.as_deref(),
        &title,
        &desc,
        lang,
        &parameterized,
    )?;

    eprintln!("jot: 已存入 {}", path.display());
    if !applied.is_empty() {
        eprintln!(
            "jot: 自动参数化了 {} → {}",
            applied.join(", "),
            parameterized
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
        bail!("没找到任何 shell 历史文件");
    }
    let items = capture::history_ranked(top);
    if items.is_empty() {
        bail!("历史里没有值得导入的命令");
    }

    let rows: Vec<String> = items
        .iter()
        .map(|i| format!("{:>3}×  {}", i.count, i.command))
        .collect();

    let mut ui = Ui::new()?;
    let picked = ui.multi_select("从 shell 历史导入（按使用频次排序）", &rows)?;
    drop(ui);

    let Some(picked) = picked else {
        return Ok(EXIT_CANCEL);
    };
    if picked.is_empty() {
        eprintln!("jot: 没有选中任何命令");
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
        "jot: 导入了 {n} 条 → {}",
        builtin::ensure_personal_notebook(&paths)?.display()
    );
    eprintln!("jot: 建议现在跑一次 `jot edit my` 把标题改成你看得懂的话");
    Ok(0)
}

// ─────────────────────────── 其它子命令 ───────────────────────────

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
        let visible: Vec<&Entry> = nb.entries.iter().filter(|e| e.visible_on(plat)).collect();
        if visible.is_empty() {
            continue;
        }
        println!(
            "\n# {}  ({} 条)  {}",
            nb.name,
            visible.len(),
            nb.description
        );
        for e in visible {
            println!("  {:<40} {}", e.title, vars::preview(&e.command));
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
            "不认识的 shell «{shell}»，支持：{}",
            shellinit::SHELLS.join(" / ")
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
                bail!("没有叫 «{n}» 的笔记本。已有的看 `jot ls`");
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
        bail!("{} 已经存在了", path.display());
    }
    std::fs::write(
        &path,
        format!(
            "---\nname: {name}\ndescription: \ntags: []\n---\n\n## 第一条命令\n\n说明写在这里。\n\n```sh\necho {{{{name}}}}\n```\n"
        ),
    )?;
    eprintln!("jot: 建好了 {}", path.display());
    open_editor(&path)?;
    Ok(0)
}

fn cmd_use(profile: &str) -> Result<i32> {
    let paths = Paths::discover()?;
    let mut cfg = Config::load(&paths);
    cfg.profile = Some(profile.to_string());
    cfg.save(&paths)?;
    let profiles = Profiles::load(&paths);
    eprintln!("jot: 当前 Profile → {profile}");
    let entries = profiles.entries(profile);
    if entries.is_empty() {
        eprintln!("jot: 这个 Profile 还没有变量，用 `jot profile set 键 值` 加一个");
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
            println!("当前 Profile: {current}");
            let entries = profiles.entries(&current);
            if entries.is_empty() {
                println!("（还没有变量）");
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
            eprintln!("jot: [{current}] 已删除 {key}");
        }
    }
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
    println!("平台            {plat}");
    println!("数据目录        {}", paths.root.display());
    println!("笔记本          {} 个", lib.notebooks.len());
    println!(
        "条目            {} 条（当前平台可见 {} 条）",
        lib.notebooks.iter().map(|n| n.entries.len()).sum::<usize>(),
        lib.entry_count()
    );
    println!("加载耗时        {:.1} ms", lib.load_ms);
    println!("当前 Profile    {}", cfg.profile_name());
    println!(
        "Profile 变量    {} 个",
        profiles.entries(cfg.profile_name()).len()
    );

    let usage = jot_core::Usage::load(&paths);
    println!("累计使用        {} 次", usage.total_uses());
    let top = usage.top(5);
    if !top.is_empty() {
        println!("最常用");
        for (id, stat) in top {
            println!("  {:>4}×  {id}", stat.count);
        }
    }

    let hist = capture::history_files();
    if hist.is_empty() {
        println!("shell 历史      没找到（`jot save` 需要手动给命令）");
    } else {
        for h in &hist {
            println!("shell 历史      {}", h.display());
        }
    }

    if lib.load_ms > 50.0 {
        println!("\n⚠ 加载超过 50ms 预算，widget 会感觉到卡。");
    }
    let local_count: usize = lib
        .notebooks
        .iter()
        .filter(|n| n.path.starts_with(paths.local_dir()))
        .map(|n| n.entries.len())
        .sum();
    if local_count == 0 {
        println!("\n你还没有存过自己的命令。试试：");
        println!("  jot import history --top 40");
        println!("  jot save \"你刚敲过的那条命令\"");
    }
    Ok(0)
}

fn open_editor(path: &Path) -> Result<()> {
    if let Ok(ed) = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")) {
        if !ed.trim().is_empty() {
            Command::new(ed).arg(path).status()?;
            return Ok(());
        }
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
            rw(&["docker", "日志"]),
            ["jot", "pick", "--query", "docker 日志"]
        );
    }

    /// 回归：flag 曾经被一起吞进搜索词，导致 `jot docker --first` 不工作。
    #[test]
    fn flags_after_a_bare_query_stay_flags() {
        assert_eq!(
            rw(&["docker", "日志", "--first"]),
            ["jot", "pick", "--query", "docker 日志", "--first"]
        );
        assert_eq!(
            rw(&["git", "--first", "撤销"]),
            ["jot", "pick", "--query", "git", "--first", "撤销"]
        );
    }

    #[test]
    fn real_subcommands_are_left_alone() {
        for sub in ["save", "ls", "doctor", "init", "profile"] {
            assert_eq!(rw(&[sub])[1], sub, "子命令 {sub} 被当成搜索词了");
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
