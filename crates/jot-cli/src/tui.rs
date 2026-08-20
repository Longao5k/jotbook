//! The terminal interface.
//!
//! Drawn straight to the controlling terminal - `CONOUT$` on Windows,
//! `/dev/tty` elsewhere - never to stdout or stderr. Those belong to the
//! caller: stdout carries the result to the shell widget (widget protocol,
//! design doc 4.2), and either of them may be a pipe depending on how the
//! shell invoked us. Rendering to the device directly means the picker works
//! no matter what the caller redirects.

use anyhow::Result;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use jot_core::notebook::Entry;
use jot_core::resolve::Choice;
use jot_core::t;
use jot_core::Usage;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{backend::CrosstermBackend, Frame, Terminal};
use std::fs::{File, OpenOptions};

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;
const WARN: Color = Color::Yellow;

/// Open the controlling terminal for writing.
///
/// `CONOUT$` needs read *and* write access or the console APIs reject it.
/// Returns None when there is no terminal attached at all, which is the only
/// case where a picker genuinely cannot run.
pub fn open_console() -> Option<File> {
    let path = if cfg!(windows) { "CONOUT$" } else { "/dev/tty" };
    OpenOptions::new().read(true).write(true).open(path).ok()
}

pub struct Ui {
    term: Terminal<CrosstermBackend<File>>,
    /// A second handle for the alternate-screen escape sequences on teardown
    console: File,
}

impl Ui {
    pub fn new() -> Result<Ui> {
        // A console handle alone is not enough: a test harness or a build
        // script inherits one too, and opening a picker there would hang
        // forever on a keypress that never comes. Interactive use always has
        // stdin on the terminal - the shell widgets redirect it from the tty
        // explicitly for exactly this reason.
        let interactive =
            std::io::IsTerminal::is_terminal(&std::io::stdin()) && open_console().is_some();
        let Some(console) = open_console().filter(|_| interactive) else {
            anyhow::bail!("{}", t!("jot 需要一个真正的终端。要在脚本里用请加 --first，例如：\n  jot pick --query \"docker 日志\" --first", "jot needs a real terminal. For scripts add --first, for example:\n  jot pick --query \"docker logs\" --first"
            ));
        };
        let mut screen = console.try_clone()?;
        enable_raw_mode()?;
        execute!(screen, EnterAlternateScreen)?;
        let term = Terminal::new(CrosstermBackend::new(console))?;
        Ok(Ui {
            term,
            console: screen,
        })
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.console, LeaveAlternateScreen);
        let _ = self.term.show_cursor();
    }
}

/// What a question screen came back with.
///
/// Esc means leave entirely - that is what people expect of it, and taking
/// that away was a mistake. Ctrl+B is the separate, explicit way back.
///
/// Generic over the answer because the multi-select hands back indices rather
/// than a string, and "backed out" versus "left" is the same distinction there.
#[derive(Debug)]
pub enum Answer<T = String> {
    Value(T),
    Back,
    Cancel,
}

#[derive(Debug)]
pub enum Picked {
    Entry(usize),
    Edit(usize),
    Cancel,
}

/// A typed query, split into scopes and free terms.
///
/// `@name` narrows to a notebook and `#name` to a tag, so a flat list of six
/// hundred entries can be browsed a category at a time. Everything else is a
/// plain search term.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Query<'a> {
    pub notebooks: Vec<&'a str>,
    pub tags: Vec<&'a str>,
    pub terms: Vec<&'a str>,
}

impl<'a> Query<'a> {
    pub(crate) fn parse(input: &'a str) -> Query<'a> {
        let mut q = Query::default();
        for part in input.split_whitespace() {
            match (part.strip_prefix('@'), part.strip_prefix('#')) {
                // A bare @ or # is someone mid-typing, not a filter yet
                (Some(""), _) | (_, Some("")) => {}
                (Some(rest), _) => q.notebooks.push(rest),
                (_, Some(rest)) => q.tags.push(rest),
                _ => q.terms.push(part),
            }
        }
        q
    }

    fn is_empty(&self) -> bool {
        self.notebooks.is_empty() && self.tags.is_empty() && self.terms.is_empty()
    }
}

/// Score one entry against a query. `None` means it does not match at all.
///
/// Every part must match, like fzf's AND semantics: scopes narrow, terms search.
pub(crate) fn score(
    matcher: &SkimMatcherV2,
    entry: &Entry,
    hay: &str,
    q: &Query<'_>,
) -> Option<i64> {
    if q.is_empty() {
        return Some(0);
    }
    let mut total = 0i64;
    for wanted in &q.notebooks {
        total += matcher.fuzzy_match(&entry.notebook, wanted)?;
    }
    for wanted in &q.tags {
        // A tag filter matches if any one of the entry's tags does
        total += entry
            .tags
            .iter()
            .filter_map(|t| matcher.fuzzy_match(t, wanted))
            .max()?;
    }
    for term in &q.terms {
        total += matcher.fuzzy_match(hay, term)?;
    }
    Some(total)
}

/// Truncate by **terminal columns**, not by character count.
///
/// A CJK character occupies two columns. With `chars().count()` a Chinese
/// title never triggers the ellipsis and is instead clipped by the terminal,
/// taking the notebook name and the danger marker off screen with it.
fn clamp_line(s: &str, max: usize) -> String {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

    let s = s.replace('\n', " ⏎ ");
    if s.width() <= max {
        return s;
    }
    // Leave one column for the ellipsis
    let budget = max.saturating_sub(1);
    let mut out = String::new();
    let mut used = 0usize;
    for c in s.chars() {
        let w = c.width().unwrap_or(0);
        if used + w > budget {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}

/// Rendering for the picker. A free function so ratatui's TestBackend can
/// assert on it offline. The interaction cannot be driven from a test, but
/// "does it draw the right thing" and "does a narrow terminal panic" can.
pub(crate) fn draw_picker(
    f: &mut Frame,
    entries: &[&Entry],
    hits: &[(usize, i64)],
    query: &str,
    state: &mut ListState,
) {
    let plat = jot_core::notebook::current_platform();
    let sel_entry = state
        .selected()
        .and_then(|c| hits.get(c))
        .map(|(i, _)| entries[*i]);
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(5),
        ])
        .split(area);

    // ── search box ──
    let title = format!(" jot  {}/{} ", hits.len(), entries.len());
    let search = Paragraph::new(Line::from(vec![
        Span::styled("› ", Style::default().fg(ACCENT)),
        Span::raw(query),
        Span::styled("▏", Style::default().fg(ACCENT)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM))
            .title(Span::styled(
                title,
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            )),
    );
    f.render_widget(search, chunks[0]);

    // ── results ──
    let width = chunks[1].width.saturating_sub(4) as usize;
    let items: Vec<ListItem> = hits
        .iter()
        .map(|(i, _)| {
            let e = entries[*i];
            let mut head = vec![Span::styled(
                clamp_line(&e.title, width.saturating_sub(14)),
                Style::default().add_modifier(Modifier::BOLD),
            )];
            if e.confirm {
                head.push(Span::styled(" ⚠", Style::default().fg(WARN)));
            }
            head.push(Span::styled(
                format!("  {}", e.notebook),
                Style::default().fg(DIM),
            ));
            // Label the platform rather than hiding the entry: a Windows box
            // is often just the terminal you ssh from, and WSL blurs it more.
            if let Some(p) = e.platform_label() {
                let style = if e.runs_on(plat) {
                    Style::default().fg(DIM)
                } else {
                    Style::default().fg(WARN)
                };
                head.push(Span::styled(format!(" · {p}"), style));
            }
            ListItem::new(vec![
                Line::from(head),
                Line::from(Span::styled(
                    format!(
                        "  {}",
                        clamp_line(&jot_core::vars::preview(&e.command), width)
                    ),
                    Style::default().fg(Color::Gray),
                )),
            ])
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::NONE))
        .highlight_style(Style::default().bg(Color::Rgb(30, 60, 62)))
        .highlight_symbol("❯ ");
    f.render_stateful_widget(list, chunks[1], state);

    // ── detail and help ──
    let mut detail: Vec<Line> = Vec::new();
    if let Some(e) = sel_entry {
        if !e.description.is_empty() {
            detail.push(Line::from(Span::styled(
                e.description.clone(),
                Style::default().fg(Color::Gray),
            )));
        }
        if e.confirm {
            detail.push(Line::from(Span::styled(
                t!(
                    "⚠ 危险命令，使用前会再确认一次",
                    "Dangerous command - you will be asked to confirm"
                ),
                Style::default().fg(WARN),
            )));
        }
        if let Some(p) = e.platform_label().filter(|_| !e.runs_on(plat)) {
            detail.push(Line::from(Span::styled(
                t!(
                    "这是 {} 命令，本机是 {} —— ssh 或 WSL 里照样能用",
                    "A {} command; this machine is {} - still usable over ssh or in WSL",
                    p,
                    plat
                ),
                Style::default().fg(WARN),
            )));
        }
    }
    detail.push(Line::from(Span::styled(
        t!(
            "↑↓ 选择   ⏎ 使用   ^E 打开文件   esc 取消   ·   @笔记本  #标签 可缩小范围",
            "up/down move   enter use   ^E open file   esc cancel   ·   @notebook  #tag to narrow"
        ),
        Style::default().fg(DIM),
    )));

    let help = Paragraph::new(detail).wrap(Wrap { trim: true }).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(DIM)),
    );
    f.render_widget(help, chunks[2]);
}

/// The command being filled in, shown above every question.
fn context_box(f: &mut Frame, area: Rect, context: &str) {
    f.render_widget(
        Paragraph::new(Span::styled(
            clamp_line(context, area.width.saturating_sub(4) as usize),
            Style::default().fg(Color::Gray),
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(DIM))
                .title(Span::styled(
                    t!(" 命令 ", " command "),
                    Style::default().fg(DIM),
                )),
        ),
        area,
    );
}

/// The line someone is typing into, with the question as its title.
fn input_box(f: &mut Frame, area: Rect, label: &str, input: &str) {
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("› ", Style::default().fg(ACCENT)),
            Span::raw(input),
            Span::styled("▏", Style::default().fg(ACCENT)),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT))
                .title(Span::styled(
                    format!(" {label} "),
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                )),
        ),
        area,
    );
}

/// The key hints along the bottom.
fn help_bar(f: &mut Frame, area: Rect, text: &str) {
    f.render_widget(
        Paragraph::new(Span::styled(text, Style::default().fg(DIM))).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(DIM)),
        ),
        area,
    );
}

/// Rendering for the "pick a value" question. Free for the same reason as
/// `draw_picker`: a test can assert on what it puts on the screen.
pub(crate) fn draw_ask_choice(
    f: &mut Frame,
    context: &str,
    label: &str,
    input: &str,
    hits: &[&Choice],
    custom: bool,
    state: &mut ListState,
) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(area);

    context_box(f, chunks[0], context);
    input_box(f, chunks[1], label, input);

    let mut items: Vec<ListItem> = Vec::new();
    if custom {
        items.push(ListItem::new(Line::from(vec![
            Span::styled(
                t!("使用输入的值  ", "use what you typed  "),
                Style::default().fg(WARN),
            ),
            Span::raw(input.to_string()),
        ])));
    }
    items.extend(hits.iter().map(|c| {
        ListItem::new(Line::from(clamp_line(
            &c.display,
            chunks[2].width.saturating_sub(4) as usize,
        )))
    }));
    f.render_stateful_widget(
        List::new(items)
            .highlight_style(Style::default().bg(Color::Rgb(30, 60, 62)))
            .highlight_symbol("❯ "),
        chunks[2],
        state,
    );

    help_bar(
        f,
        chunks[3],
        &t!(
            "输入筛选或直接键入值   ↑↓ 选择   ⏎ 确定   ^B 上一步   esc 退出",
            "type to filter or enter a value   up/down move   enter confirm   ^B back   esc quit"
        ),
    );
}

/// Rendering for the free-text question.
pub(crate) fn draw_ask_text(f: &mut Frame, context: &str, label: &str, input: &str) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(area);

    context_box(f, chunks[0], context);
    input_box(f, chunks[1], label, input);
    help_bar(
        f,
        chunks[3],
        &t!(
            "⏎ 确定   ^B 上一步   esc 退出",
            "enter confirm   ^B back   esc quit"
        ),
    );
}

/// A headline, the thing being decided about, and the keys. Shared by the
/// dangerous-command confirmation and by `jot rm`, which ask the same question
/// about different subjects.
pub(crate) fn draw_prompt(f: &mut Frame, headline: &str, body: &str, help: &str) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(Span::styled(
            headline,
            Style::default().fg(WARN).add_modifier(Modifier::BOLD),
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(WARN)),
        ),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new(body.to_string())
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(DIM)),
            ),
        chunks[1],
    );
    f.render_widget(
        Paragraph::new(Span::styled(help, Style::default().fg(DIM))),
        chunks[2],
    );
}

/// Rendering for the second confirmation on a dangerous command.
pub(crate) fn draw_confirm(f: &mut Frame, command: &str) {
    draw_prompt(
        f,
        &t!(
            " 这条命令被标记为危险",
            " This command is marked dangerous"
        ),
        command,
        &t!(
            "y 放到命令行上   n / ^B 回列表   esc 退出   （jot 不会替你执行）",
            "y put it on the prompt   n / ^B back to the list   esc quit   (jot never runs it for you)"
        ),
    );
}

/// Rendering for `jot rm`. Deleting cannot be undone by reading the file back,
/// so it says which file it is about to rewrite.
pub(crate) fn draw_remove(f: &mut Frame, summary: &str) {
    draw_prompt(
        f,
        &t!(" 删除这条命令？", " Delete this entry?"),
        summary,
        &t!(
            "y 删除   n / ^B 回列表   esc 退出",
            "y delete   n / ^B back to the list   esc quit"
        ),
    );
}

/// Rendering for the import checklist.
pub(crate) fn draw_multi_select(
    f: &mut Frame,
    title: &str,
    rows: &[String],
    checked: &[bool],
    state: &mut ListState,
) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(area);

    let n = checked.iter().filter(|c| **c).count();
    let total = rows.len();
    f.render_widget(
        Paragraph::new(Span::styled(
            format!(
                " {title}   {} ",
                t!("已选 {n}/{total}", "{n}/{total} selected")
            ),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(DIM)),
        ),
        chunks[0],
    );

    let width = chunks[1].width.saturating_sub(6) as usize;
    let items: Vec<ListItem> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let on = checked.get(i).copied().unwrap_or(false);
            ListItem::new(Line::from(vec![
                Span::styled(
                    if on { "◼ " } else { "◻ " },
                    Style::default().fg(if on { ACCENT } else { DIM }),
                ),
                Span::raw(clamp_line(r, width)),
            ]))
        })
        .collect();
    f.render_stateful_widget(
        List::new(items)
            .highlight_style(Style::default().bg(Color::Rgb(30, 60, 62)))
            .highlight_symbol("❯ "),
        chunks[1],
        state,
    );

    help_bar(
        f,
        chunks[2],
        &t!(
            "space 勾选   a 全选   ⏎ 导入所选   ^B 上一步   esc 退出",
            "space toggle   a select all   enter import   ^B back   esc quit"
        ),
    );
}

impl Ui {
    /// The picker.
    pub fn pick(
        &mut self,
        entries: &[&Entry],
        query: &mut String,
        usage: &Usage,
    ) -> Result<Picked> {
        let matcher = SkimMatcherV2::default().ignore_case();
        let hays: Vec<String> = entries.iter().map(|e| e.haystack()).collect();
        // Frecency weights are precomputed; they are needed on every keystroke
        let now = jot_core::usage::now_secs();
        let plat = jot_core::notebook::current_platform();
        // A small nudge, not a filter: what runs here surfaces first, but a
        // linux command stays one keystroke away on a Windows machine.
        let boosts: Vec<i64> = entries
            .iter()
            .map(|e| usage.boost(&e.id(), now) + if e.runs_on(plat) { 4 } else { 0 })
            .collect();

        let mut cursor = 0usize;
        let mut state = ListState::default();

        loop {
            // Filter and rank
            let parsed = Query::parse(query);
            let mut hits: Vec<(usize, i64)> = entries
                .iter()
                .enumerate()
                .filter_map(|(i, e)| {
                    score(&matcher, e, &hays[i], &parsed).map(|s| (i, s + boosts[i]))
                })
                .collect();
            // Always sort: with an empty query this degrades to "most used first",
            // and equal scores keep file order thanks to sort_by_key being stable
            hits.sort_by_key(|h| std::cmp::Reverse(h.1));
            if cursor >= hits.len() {
                cursor = hits.len().saturating_sub(1);
            }
            state.select(if hits.is_empty() { None } else { Some(cursor) });

            self.term
                .draw(|f| draw_picker(f, entries, &hits, query, &mut state))?;
            let Event::Key(k) = event::read()? else {
                continue;
            };
            if k.kind != KeyEventKind::Press {
                continue;
            }
            let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
            match k.code {
                KeyCode::Esc => return Ok(Picked::Cancel),
                KeyCode::Char('c') if ctrl => return Ok(Picked::Cancel),
                KeyCode::Char('e') if ctrl => {
                    if let Some((i, _)) = hits.get(cursor) {
                        return Ok(Picked::Edit(*i));
                    }
                }
                KeyCode::Enter => {
                    if let Some((i, _)) = hits.get(cursor) {
                        return Ok(Picked::Entry(*i));
                    }
                }
                KeyCode::Up => cursor = cursor.saturating_sub(1),
                KeyCode::Char('p') if ctrl => cursor = cursor.saturating_sub(1),
                KeyCode::Down => {
                    if cursor + 1 < hits.len() {
                        cursor += 1;
                    }
                }
                KeyCode::Char('n') if ctrl => {
                    if cursor + 1 < hits.len() {
                        cursor += 1;
                    }
                }
                KeyCode::PageDown => cursor = (cursor + 10).min(hits.len().saturating_sub(1)),
                KeyCode::PageUp => cursor = cursor.saturating_sub(10),
                KeyCode::Backspace => {
                    query.pop();
                    cursor = 0;
                }
                KeyCode::Char('u') if ctrl => {
                    query.clear();
                    cursor = 0;
                }
                KeyCode::Char(c) => {
                    query.push(c);
                    cursor = 0;
                }
                _ => {}
            }
        }
    }

    /// Ask for a variable: filterable when there are candidates, and a typed value is always allowed.
    pub fn ask_choice(
        &mut self,
        context: &str,
        label: &str,
        options: &[Choice],
        default: Option<&str>,
    ) -> Result<Answer> {
        let matcher = SkimMatcherV2::default().ignore_case();
        let mut input = default.unwrap_or("").to_string();
        let mut cursor = 0usize;
        let mut state = ListState::default();

        loop {
            let hits: Vec<&Choice> = options
                .iter()
                .filter(|c| input.is_empty() || matcher.fuzzy_match(&c.display, &input).is_some())
                .collect();
            // If what was typed is not among the candidates, offer it as-is
            let custom = !input.is_empty() && !options.iter().any(|c| c.value == input);
            let total = hits.len() + usize::from(custom);
            if cursor >= total {
                cursor = total.saturating_sub(1);
            }
            state.select(if total == 0 { None } else { Some(cursor) });

            self.term
                .draw(|f| draw_ask_choice(f, context, label, &input, &hits, custom, &mut state))?;

            let Event::Key(k) = event::read()? else {
                continue;
            };
            if k.kind != KeyEventKind::Press {
                continue;
            }
            let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
            match k.code {
                KeyCode::Esc => return Ok(Answer::Cancel),
                KeyCode::Char('c') if ctrl => return Ok(Answer::Cancel),
                KeyCode::Char('b') if ctrl => return Ok(Answer::Back),
                KeyCode::Enter => {
                    if custom && cursor == 0 {
                        return Ok(Answer::Value(input));
                    }
                    let idx = cursor - usize::from(custom);
                    if let Some(c) = hits.get(idx) {
                        return Ok(Answer::Value(c.value.clone()));
                    }
                    if !input.is_empty() {
                        return Ok(Answer::Value(input));
                    }
                }
                KeyCode::Up => cursor = cursor.saturating_sub(1),
                KeyCode::Down => {
                    if cursor + 1 < total {
                        cursor += 1;
                    }
                }
                KeyCode::Backspace => {
                    input.pop();
                    cursor = 0;
                }
                KeyCode::Char('u') if ctrl => {
                    input.clear();
                    cursor = 0;
                }
                KeyCode::Char(c) => {
                    input.push(c);
                    cursor = 0;
                }
                _ => {}
            }
        }
    }

    /// Free text entry.
    pub fn ask_text(
        &mut self,
        context: &str,
        label: &str,
        default: Option<&str>,
    ) -> Result<Answer> {
        let mut input = default.unwrap_or("").to_string();
        loop {
            self.term
                .draw(|f| draw_ask_text(f, context, label, &input))?;

            let Event::Key(k) = event::read()? else {
                continue;
            };
            if k.kind != KeyEventKind::Press {
                continue;
            }
            let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
            match k.code {
                KeyCode::Esc => return Ok(Answer::Cancel),
                KeyCode::Char('c') if ctrl => return Ok(Answer::Cancel),
                KeyCode::Char('b') if ctrl => return Ok(Answer::Back),
                KeyCode::Enter => return Ok(Answer::Value(input)),
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char('u') if ctrl => input.clear(),
                KeyCode::Char(c) => input.push(c),
                _ => {}
            }
        }
    }

    /// Second confirmation for a dangerous command.
    pub fn confirm(&mut self, command: &str) -> Result<Answer> {
        self.yes_no(|f| draw_confirm(f, command))
    }

    /// Confirmation before `jot rm` rewrites a notebook.
    pub fn confirm_remove(&mut self, summary: &str) -> Result<Answer> {
        self.yes_no(|f| draw_remove(f, summary))
    }

    /// The y/n key loop both confirmations share.
    ///
    /// `n` goes back rather than out: someone who says no to one entry almost
    /// always wants the list again, not to lose the session.
    fn yes_no(&mut self, draw: impl Fn(&mut Frame)) -> Result<Answer> {
        loop {
            self.term.draw(|f| draw(f))?;

            let Event::Key(k) = event::read()? else {
                continue;
            };
            if k.kind != KeyEventKind::Press {
                continue;
            }
            match k.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    return Ok(Answer::Value(String::new()))
                }
                KeyCode::Char('n') | KeyCode::Char('N') => return Ok(Answer::Back),
                KeyCode::Char('b') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(Answer::Back)
                }
                KeyCode::Esc => return Ok(Answer::Cancel),
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(Answer::Cancel)
                }
                _ => {}
            }
        }
    }

    /// Multi-select, used by both importers.
    pub fn multi_select(&mut self, title: &str, rows: &[String]) -> Result<Answer<Vec<usize>>> {
        let mut checked = vec![false; rows.len()];
        let mut cursor = 0usize;
        let mut state = ListState::default();

        loop {
            state.select(Some(cursor));
            self.term
                .draw(|f| draw_multi_select(f, title, rows, &checked, &mut state))?;

            let Event::Key(k) = event::read()? else {
                continue;
            };
            if k.kind != KeyEventKind::Press {
                continue;
            }
            let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
            match k.code {
                KeyCode::Esc => return Ok(Answer::Cancel),
                KeyCode::Char('c') if ctrl => return Ok(Answer::Cancel),
                KeyCode::Char('b') if ctrl => return Ok(Answer::Back),
                KeyCode::Enter => {
                    return Ok(Answer::Value(
                        checked
                            .iter()
                            .enumerate()
                            .filter(|(_, c)| **c)
                            .map(|(i, _)| i)
                            .collect(),
                    ))
                }
                KeyCode::Char(' ') => {
                    if cursor < checked.len() {
                        checked[cursor] = !checked[cursor];
                        if cursor + 1 < rows.len() {
                            cursor += 1;
                        }
                    }
                }
                KeyCode::Char('a') => {
                    let all = checked.iter().all(|c| *c);
                    checked.iter_mut().for_each(|c| *c = !all);
                }
                KeyCode::Up => cursor = cursor.saturating_sub(1),
                KeyCode::Down => {
                    if cursor + 1 < rows.len() {
                        cursor += 1;
                    }
                }
                KeyCode::PageUp => cursor = cursor.saturating_sub(10),
                KeyCode::PageDown => cursor = (cursor + 10).min(rows.len().saturating_sub(1)),
                _ => {}
            }
        }
    }
}

/// Serialises every test that draws, and pins the language while it does.
///
/// The language is one process-wide atomic. When the question-screen tests
/// arrived they switched it mid-run, and the picker tests beside them - which
/// had asserted English text for months - started failing on CI: two of the
/// three runners have enough cores to actually interleave them. Windows passed
/// and Linux and macOS did not, which reads like a platform bug and is not one.
///
/// Anything that renders goes through here, so the two can no longer overlap.
#[cfg(test)]
fn with_lang<T>(lang: jot_core::i18n::Lang, f: impl FnOnce() -> T) -> T {
    static LANG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    // A panicking test poisons the mutex; that is its business, not the next
    // test's, so take the guard either way.
    let _g = LANG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    jot_core::i18n::set(lang);
    f()
}

#[cfg(test)]
mod tests {
    use super::*;
    use jot_core::i18n::Lang;
    use jot_core::notebook;
    use ratatui::backend::TestBackend;
    use std::path::Path;

    const SRC: &str = "---\nname: demo\n---\n\n\
## Restart the backend service\n\n\
Required after every config change.\n\n\
```sh @confirm\nsudo systemctl restart {{service}}\n```\n\n\
## View the logs\n\n\
```sh\njournalctl -f\n```\n";

    fn entries() -> Vec<notebook::Entry> {
        notebook::parse(Path::new("demo.md"), SRC).unwrap().entries
    }

    /// Dump the rendered buffer as plain text so it can be asserted on.
    ///
    /// CJK characters are double width and ratatui stores one per two cells, so
    /// this must advance by display width or the dump comes out spaced apart.
    fn render(w: u16, h: u16, query: &str, n_hits: usize, selected: Option<usize>) -> String {
        render_entries(&entries(), w, h, query, n_hits, selected)
    }

    fn render_entries(
        owned: &[notebook::Entry],
        w: u16,
        h: u16,
        query: &str,
        n_hits: usize,
        selected: Option<usize>,
    ) -> String {
        render_entries_in(Lang::En, owned, w, h, query, n_hits, selected)
    }

    fn render_entries_in(
        lang: Lang,
        owned: &[notebook::Entry],
        w: u16,
        h: u16,
        query: &str,
        n_hits: usize,
        selected: Option<usize>,
    ) -> String {
        use unicode_width::UnicodeWidthStr;

        let refs: Vec<&Entry> = owned.iter().collect();
        let hits: Vec<(usize, i64)> = (0..n_hits.min(refs.len())).map(|i| (i, 0)).collect();
        let mut state = ListState::default();
        state.select(selected);

        with_lang(lang, || {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| draw_picker(f, &refs, &hits, query, &mut state))
                .unwrap();

            let buf = term.backend().buffer();
            let mut out = String::new();
            for y in 0..buf.area.height {
                let mut x = 0u16;
                while x < buf.area.width {
                    let sym = buf[(x, y)].symbol();
                    out.push_str(sym);
                    x += (UnicodeWidthStr::width(sym).max(1)) as u16;
                }
                out.push('\n');
            }
            out
        })
    }

    /// The picker draws in whichever language is set, and the test that proves
    /// it is also the one that would catch the lock going missing again.
    #[test]
    fn the_picker_follows_the_language() {
        let en = render_entries_in(Lang::En, &entries(), 90, 24, "", 2, Some(0));
        assert!(en.contains("Dangerous command"), "{en}");

        let zh = render_entries_in(Lang::Zh, &entries(), 90, 24, "", 2, Some(0));
        assert!(zh.contains("危险命令"), "{zh}");
    }

    /// Regression: truncation used to count characters, so Chinese titles never
    /// triggered the ellipsis and were clipped by the terminal instead.
    #[test]
    fn clamp_measures_columns_not_characters() {
        use unicode_width::UnicodeWidthStr;

        // Deliberately Chinese: this is exactly the case being tested.
        // 11 double-width characters occupy 22 terminal columns.
        let cjk = "重启后端服务的那条命令";
        assert_eq!(cjk.chars().count(), 11);
        assert_eq!(cjk.width(), 22);
        assert_eq!(
            clamp_line(cjk, 30),
            cjk,
            "should not truncate when there is room"
        );

        let cut = clamp_line(cjk, 10);
        assert!(cut.ends_with('…'), "no ellipsis: {cut}");
        assert!(
            cut.width() <= 10,
            "still {} columns after truncation: {cut}",
            cut.width()
        );

        // Pure ASCII behaviour is unchanged
        assert_eq!(clamp_line("abcdefghij", 5), "abcd…");
        assert_eq!(clamp_line("abc", 5), "abc");
    }

    #[test]
    fn draws_query_titles_and_counts() {
        let s = render(80, 24, "Restart", 2, Some(0));
        assert!(s.contains("Restart"), "query not drawn:\n{s}");
        assert!(
            s.contains("Restart the backend service"),
            "entry title not drawn:\n{s}"
        );
        assert!(s.contains("2/2"), "count is wrong:\n{s}");
        assert!(s.contains("demo"), "notebook name not drawn:\n{s}");
    }

    #[test]
    fn variables_render_as_readable_placeholders() {
        let s = render(80, 24, "", 2, Some(0));
        assert!(
            s.contains("⟨service⟩"),
            "variables not rendered as readable placeholders:\n{s}"
        );
        assert!(
            !s.contains("{{service}}"),
            "raw braces shown in the list:\n{s}"
        );
    }

    #[test]
    fn dangerous_entries_are_marked() {
        let s = render(80, 24, "", 2, Some(0));
        assert!(s.contains('⚠'), "dangerous command not marked:\n{s}");
        assert!(
            s.contains("Dangerous command"),
            "no hint in the detail area:\n{s}"
        );
    }

    #[test]
    fn selection_marker_follows_the_cursor() {
        assert!(render(80, 24, "", 2, Some(0)).contains('❯'));
        let second = render(80, 24, "", 2, Some(1));
        let line = second
            .lines()
            .find(|l| l.contains('❯'))
            .expect("no selection marker");
        assert!(
            line.contains("View the logs"),
            "selection marker is on the wrong row: {line}"
        );
    }

    /// Narrow and extreme terminal sizes are the classic source of TUI panics.
    #[test]
    fn extreme_terminal_sizes_do_not_panic() {
        for (w, h) in [
            (80, 24),
            (200, 60),
            (40, 12),
            (24, 10),
            (20, 8),
            (12, 6),
            (8, 4),
            (4, 3),
            (1, 1),
        ] {
            let _ = render(w, h, "Restart", 2, Some(0));
            let _ = render(w, h, "", 2, None);
        }
    }

    #[test]
    fn empty_result_set_does_not_panic() {
        let s = render(80, 24, "nothing can possibly match this string", 0, None);
        assert!(
            s.contains("0/2"),
            "count is wrong for an empty result set:\n{s}"
        );
    }

    #[test]
    fn long_content_is_truncated_with_an_ellipsis() {
        // At 40 columns a long title and command must be truncated, not break the layout
        let src = "---
name: demo
---

## An extremely extremely extremely extremely long title used to verify truncation

```sh
echo an equally extremely extremely extremely long command body {{var}}
```
";
        let owned = notebook::parse(Path::new("demo.md"), src).unwrap().entries;
        let s = render_entries(&owned, 40, 20, "", 1, Some(0));
        assert!(
            s.contains('…'),
            "content was not truncated on a narrow terminal:
{s}"
        );
        assert_eq!(
            s.lines().count(),
            20,
            "rendered row count does not match the terminal height:
{s}"
        );
    }
}

#[cfg(test)]
mod query_tests {
    use super::*;
    use jot_core::notebook;
    use std::path::Path;

    const SRC: &str = "---\nname: docker\ntags: [ops]\n---\n\n\
## Follow container logs\n\n\
```sh @tags=logs,daily\ndocker logs -f\n```\n\n\
## Prune images\n\n\
```sh @tags=cleanup\ndocker system prune\n```\n";

    fn entries() -> Vec<notebook::Entry> {
        notebook::parse(Path::new("docker.md"), SRC)
            .unwrap()
            .entries
    }

    fn matches(query: &str) -> Vec<String> {
        let m = SkimMatcherV2::default().ignore_case();
        let q = Query::parse(query);
        entries()
            .iter()
            .filter(|e| score(&m, e, &e.haystack(), &q).is_some())
            .map(|e| e.title.clone())
            .collect()
    }

    #[test]
    fn plain_terms_search_everything() {
        assert_eq!(matches("logs"), ["Follow container logs"]);
        assert_eq!(matches("").len(), 2);
    }

    #[test]
    fn at_narrows_to_a_notebook() {
        assert_eq!(matches("@docker").len(), 2);
        assert!(matches("@git").is_empty(), "matched the wrong notebook");
    }

    #[test]
    fn hash_narrows_to_a_tag() {
        assert_eq!(matches("#cleanup"), ["Prune images"]);
        assert_eq!(matches("#logs"), ["Follow container logs"]);
        assert!(matches("#nosuchtag").is_empty());
    }

    #[test]
    fn scopes_and_terms_combine() {
        assert_eq!(matches("@docker prune"), ["Prune images"]);
        assert_eq!(matches("@docker #logs"), ["Follow container logs"]);
        // Every part has to match, so a contradiction yields nothing
        assert!(matches("@docker #cleanup logs").is_empty());
    }

    #[test]
    fn a_bare_prefix_is_not_a_filter_yet() {
        // Someone mid-typing must not have the list vanish under them
        assert_eq!(Query::parse("@"), Query::default());
        assert_eq!(Query::parse("#"), Query::default());
        assert_eq!(matches("@").len(), 2);
    }

    #[test]
    fn parsing_splits_the_three_kinds() {
        let q = Query::parse("@docker #deploy restart api");
        assert_eq!(q.notebooks, ["docker"]);
        assert_eq!(q.tags, ["deploy"]);
        assert_eq!(q.terms, ["restart", "api"]);
    }
}

/// The four screens the picker hands off to.
///
/// These went untested for a long time while the picker beside them did not,
/// and the difference showed: the back key was missing from every help line and
/// the checklist counted in Chinese no matter what language was set. Neither
/// survives a render assertion.
#[cfg(test)]
mod question_screens {
    use super::*;
    use jot_core::i18n::Lang;
    use ratatui::backend::TestBackend;

    /// Draw one screen in `lang`, through the shared lock, so that nothing here
    /// can change the language out from under a test drawing elsewhere.
    fn dump_in(lang: Lang, w: u16, h: u16, draw: impl FnOnce(&mut Frame)) -> String {
        use unicode_width::UnicodeWidthStr;

        with_lang(lang, || {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(draw).unwrap();
            let buf = term.backend().buffer();
            let mut out = String::new();
            for y in 0..buf.area.height {
                let mut x = 0u16;
                while x < buf.area.width {
                    let sym = buf[(x, y)].symbol();
                    out.push_str(sym);
                    x += UnicodeWidthStr::width(sym).max(1) as u16;
                }
                out.push('\n');
            }
            out
        })
    }

    fn dump(w: u16, h: u16, draw: impl FnOnce(&mut Frame)) -> String {
        dump_in(Lang::En, w, h, draw)
    }

    fn choices(vals: &[&str]) -> Vec<Choice> {
        vals.iter()
            .map(|v| Choice {
                value: v.to_string(),
                display: v.to_string(),
            })
            .collect()
    }

    /// Draw all four at a comfortable size and hand back the text of each.
    fn all_four(lang: Lang, w: u16, h: u16) -> Vec<(&'static str, String)> {
        let opts = choices(&["api", "worker"]);
        let refs: Vec<&Choice> = opts.iter().collect();
        let rows = vec!["adb devices".to_string(), "adb shell".to_string()];
        vec![
            (
                "ask_choice",
                dump_in(lang, w, h, |f| {
                    let mut st = ListState::default();
                    st.select(Some(0));
                    draw_ask_choice(
                        f,
                        "systemctl restart X",
                        "service",
                        "",
                        &refs,
                        false,
                        &mut st,
                    )
                }),
            ),
            (
                "ask_text",
                dump_in(lang, w, h, |f| draw_ask_text(f, "curl X", "url", "")),
            ),
            (
                "confirm",
                dump_in(lang, w, h, |f| draw_confirm(f, "rm -rf /tmp/x")),
            ),
            (
                "multi_select",
                dump_in(lang, w, h, |f| {
                    let mut st = ListState::default();
                    st.select(Some(0));
                    draw_multi_select(f, "Import", &rows, &[true, false], &mut st)
                }),
            ),
        ]
    }

    /// Ctrl+B shipped working but unadvertised, which is the same as not
    /// shipping it. Every screen that accepts it has to say so.
    #[test]
    fn every_screen_that_takes_the_back_key_says_so() {
        for lang in [Lang::En, Lang::Zh] {
            for (name, text) in all_four(lang, 100, 24) {
                assert!(
                    text.contains("^B"),
                    "{name} does not mention the back key in {lang:?}:\n{text}"
                );
            }
        }
    }

    /// Esc is the way out and has to stay visible next to it.
    #[test]
    fn every_screen_still_offers_esc() {
        for (name, text) in all_four(Lang::En, 100, 24) {
            assert!(text.contains("esc"), "{name} lost its esc hint:\n{text}");
        }
    }

    /// The checklist header was `format!("已选 {n}/{}")` with no `t!` around it,
    /// so an English user counted their selection in Chinese.
    #[test]
    fn the_checklist_counts_in_the_active_language() {
        let rows = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let draw = |f: &mut Frame| {
            let mut st = ListState::default();
            st.select(Some(0));
            draw_multi_select(f, "Import", &rows, &[true, true, false], &mut st)
        };

        let en = dump_in(Lang::En, 80, 12, draw);
        assert!(en.contains("2/3 selected"), "no English count:\n{en}");
        assert!(!en.contains('已'), "Chinese leaked into English:\n{en}");

        let zh = dump_in(Lang::Zh, 80, 12, draw);
        assert!(zh.contains("已选 2/3"), "no Chinese count:\n{zh}");
    }

    /// The one affordance that lets someone name a notebook that does not exist
    /// yet. Losing it would make `jot import text` unable to start a notebook.
    #[test]
    fn a_typed_value_that_is_not_on_the_list_is_offered_as_is() {
        let opts = choices(&["my", "docker"]);
        let refs: Vec<&Choice> = opts.iter().collect();
        let text = dump(80, 16, |f| {
            let mut st = ListState::default();
            st.select(Some(0));
            draw_ask_choice(f, "ctx", "notebook", "brand-new", &refs, true, &mut st)
        });
        assert!(text.contains("use what you typed"), "{text}");
        assert!(text.contains("brand-new"), "{text}");
    }

    /// The promise the whole tool rests on, printed where it is read.
    #[test]
    fn the_confirm_screen_says_jot_will_not_run_it() {
        let text = dump(90, 12, |f| draw_confirm(f, "rm -rf /var/lib/data"));
        assert!(text.contains("rm -rf /var/lib/data"), "{text}");
        assert!(text.contains("never runs it"), "{text}");
        assert!(text.contains("dangerous"), "{text}");
    }

    /// The tick marks are the only way to tell what is selected.
    #[test]
    fn the_checklist_marks_what_is_ticked() {
        let rows = vec!["first".to_string(), "second".to_string()];
        let text = dump(60, 10, |f| {
            let mut st = ListState::default();
            st.select(Some(1));
            draw_multi_select(f, "T", &rows, &[true, false], &mut st)
        });
        let first = text.lines().find(|l| l.contains("first")).unwrap();
        let second = text.lines().find(|l| l.contains("second")).unwrap();
        assert!(first.contains('◼'), "ticked row not marked: {first:?}");
        assert!(second.contains('◻'), "unticked row marked: {second:?}");
        assert!(second.contains('❯'), "cursor not on row two: {second:?}");
    }

    /// A CJK label is two columns per character; the box has to clip it with an
    /// ellipsis rather than let the terminal cut it off mid-frame.
    #[test]
    fn a_long_cjk_command_is_clipped_with_an_ellipsis() {
        let long = "sudo systemctl restart 一个非常非常非常长的服务名字用来把这一行撑满";
        let text = dump(40, 10, |f| draw_ask_text(f, long, "service", ""));
        assert!(text.contains('…'), "not truncated:\n{text}");
        for line in text.lines() {
            use unicode_width::UnicodeWidthStr;
            assert!(
                line.width() <= 40,
                "line is {} columns wide, past the 40 the terminal has: {line:?}",
                line.width()
            );
        }
    }

    /// Someone will open one of these in a split pane. None of them may panic.
    #[test]
    fn extreme_terminal_sizes_do_not_panic() {
        for (w, h) in [(1, 1), (3, 2), (10, 4), (20, 5), (200, 60), (400, 3)] {
            for (name, _) in all_four(Lang::En, w, h) {
                let _ = name; // drawing without panicking is the assertion
            }
        }
    }

    /// An empty checklist happens whenever a paste yields nothing usable.
    #[test]
    fn an_empty_checklist_does_not_panic() {
        let _ = dump(40, 10, |f| {
            let mut st = ListState::default();
            st.select(Some(0));
            draw_multi_select(f, "empty", &[], &[], &mut st)
        });
    }
}
