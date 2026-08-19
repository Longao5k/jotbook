//! 终端界面。
//!
//! 全部画到 stderr —— stdout 要留给结果本身，shell widget 靠它拿到最终命令
//! （见设计文档 §4.2 的 widget 协议）。

use anyhow::Result;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use jot_core::notebook::Entry;
use jot_core::resolve::Choice;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::Stderr;

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;
const WARN: Color = Color::Yellow;

pub struct Ui {
    term: Terminal<CrosstermBackend<Stderr>>,
}

impl Ui {
    pub fn new() -> Result<Ui> {
        // widget 模式下 stdout 是管道，但 stderr 仍然是终端 —— 界面画在 stderr 上
        if !std::io::IsTerminal::is_terminal(&std::io::stderr()) {
            anyhow::bail!(
                "jot 需要一个真正的终端。要在脚本里用请加 --first，例如：\n  jot pick --query \"docker 日志\" --first"
            );
        }
        enable_raw_mode()?;
        let mut err = std::io::stderr();
        execute!(err, EnterAlternateScreen)?;
        let term = Terminal::new(CrosstermBackend::new(std::io::stderr()))?;
        Ok(Ui { term })
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stderr(), LeaveAlternateScreen);
        let _ = self.term.show_cursor();
    }
}

#[derive(Debug)]
pub enum Picked {
    Entry(usize),
    Edit(usize),
    Cancel,
}

fn score(matcher: &SkimMatcherV2, hay: &str, needle: &str) -> Option<i64> {
    if needle.is_empty() {
        return Some(0);
    }
    // 空格分词，每段都要命中（类似 fzf 的 AND 语义）
    let mut total = 0i64;
    for part in needle.split_whitespace() {
        total += matcher.fuzzy_match(hay, part)?;
    }
    Some(total)
}

fn clamp_line(s: &str, max: usize) -> String {
    let s = s.replace('\n', " ⏎ ");
    if s.chars().count() <= max {
        return s;
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

impl Ui {
    /// 主选择器。
    pub fn pick(&mut self, entries: &[&Entry], initial: &str) -> Result<Picked> {
        let matcher = SkimMatcherV2::default().ignore_case();
        let hays: Vec<String> = entries.iter().map(|e| e.haystack()).collect();

        let mut query = initial.to_string();
        let mut cursor = 0usize;
        let mut state = ListState::default();

        loop {
            // 过滤 + 排序
            let mut hits: Vec<(usize, i64)> = entries
                .iter()
                .enumerate()
                .filter_map(|(i, _)| score(&matcher, &hays[i], &query).map(|s| (i, s)))
                .collect();
            if !query.is_empty() {
                hits.sort_by(|a, b| b.1.cmp(&a.1));
            }
            if cursor >= hits.len() {
                cursor = hits.len().saturating_sub(1);
            }
            state.select(if hits.is_empty() { None } else { Some(cursor) });

            let sel_entry = hits.get(cursor).map(|(i, _)| entries[*i]);

            self.term.draw(|f| {
                let area = f.area();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(4),
                        Constraint::Length(5),
                    ])
                    .split(area);

                // ── 搜索框 ──
                let title = format!(" jot  {}/{} ", hits.len(), entries.len());
                let search = Paragraph::new(Line::from(vec![
                    Span::styled("› ", Style::default().fg(ACCENT)),
                    Span::raw(&query),
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

                // ── 结果列表 ──
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
                        ListItem::new(vec![
                            Line::from(head),
                            Line::from(Span::styled(
                                format!("  {}", clamp_line(&jot_core::vars::preview(&e.command), width)),
                                Style::default().fg(Color::Gray),
                            )),
                        ])
                    })
                    .collect();

                let list = List::new(items)
                    .block(Block::default().borders(Borders::NONE))
                    .highlight_style(Style::default().bg(Color::Rgb(30, 60, 62)))
                    .highlight_symbol("❯ ");
                f.render_stateful_widget(list, chunks[1], &mut state);

                // ── 详情 + 帮助 ──
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
                            "⚠ 危险命令，使用前会再确认一次",
                            Style::default().fg(WARN),
                        )));
                    }
                }
                detail.push(Line::from(Span::styled(
                    "↑↓ 选择   ⏎ 使用   ^E 打开文件   esc 取消",
                    Style::default().fg(DIM),
                )));

                let help = Paragraph::new(detail).wrap(Wrap { trim: true }).block(
                    Block::default()
                        .borders(Borders::TOP)
                        .border_style(Style::default().fg(DIM)),
                );
                f.render_widget(help, chunks[2]);
            })?;

            let Event::Key(k) = event::read()? else { continue };
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

    /// 变量取值：带候选列表时可筛选，也允许直接输入自定义值。
    pub fn ask_choice(
        &mut self,
        context: &str,
        label: &str,
        options: &[Choice],
        default: Option<&str>,
    ) -> Result<Option<String>> {
        let matcher = SkimMatcherV2::default().ignore_case();
        let mut input = default.unwrap_or("").to_string();
        let mut cursor = 0usize;
        let mut state = ListState::default();

        loop {
            let hits: Vec<&Choice> = options
                .iter()
                .filter(|c| input.is_empty() || matcher.fuzzy_match(&c.display, &input).is_some())
                .collect();
            // 输入的内容不在候选里时，允许直接用它
            let custom = !input.is_empty() && !options.iter().any(|c| c.value == input);
            let total = hits.len() + usize::from(custom);
            if cursor >= total {
                cursor = total.saturating_sub(1);
            }
            state.select(if total == 0 { None } else { Some(cursor) });

            self.term.draw(|f| {
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

                f.render_widget(
                    Paragraph::new(Span::styled(
                        clamp_line(context, area.width.saturating_sub(4) as usize),
                        Style::default().fg(Color::Gray),
                    ))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(DIM))
                            .title(Span::styled(" 命令 ", Style::default().fg(DIM))),
                    ),
                    chunks[0],
                );

                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled("› ", Style::default().fg(ACCENT)),
                        Span::raw(&input),
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
                    chunks[1],
                );

                let mut items: Vec<ListItem> = Vec::new();
                if custom {
                    items.push(ListItem::new(Line::from(vec![
                        Span::styled("使用输入的值  ", Style::default().fg(WARN)),
                        Span::raw(input.clone()),
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
                    &mut state,
                );

                f.render_widget(
                    Paragraph::new(Span::styled(
                        "输入筛选或直接键入值   ↑↓ 选择   ⏎ 确定   esc 取消",
                        Style::default().fg(DIM),
                    ))
                    .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(DIM))),
                    chunks[3],
                );
            })?;

            let Event::Key(k) = event::read()? else { continue };
            if k.kind != KeyEventKind::Press {
                continue;
            }
            let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
            match k.code {
                KeyCode::Esc => return Ok(None),
                KeyCode::Char('c') if ctrl => return Ok(None),
                KeyCode::Enter => {
                    if custom && cursor == 0 {
                        return Ok(Some(input));
                    }
                    let idx = cursor - usize::from(custom);
                    if let Some(c) = hits.get(idx) {
                        return Ok(Some(c.value.clone()));
                    }
                    if !input.is_empty() {
                        return Ok(Some(input));
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

    /// 自由输入。
    pub fn ask_text(
        &mut self,
        context: &str,
        label: &str,
        default: Option<&str>,
    ) -> Result<Option<String>> {
        let mut input = default.unwrap_or("").to_string();
        loop {
            self.term.draw(|f| {
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

                f.render_widget(
                    Paragraph::new(Span::styled(
                        clamp_line(context, area.width.saturating_sub(4) as usize),
                        Style::default().fg(Color::Gray),
                    ))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(DIM))
                            .title(Span::styled(" 命令 ", Style::default().fg(DIM))),
                    ),
                    chunks[0],
                );
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled("› ", Style::default().fg(ACCENT)),
                        Span::raw(&input),
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
                    chunks[1],
                );
                f.render_widget(
                    Paragraph::new(Span::styled(
                        "⏎ 确定   esc 取消",
                        Style::default().fg(DIM),
                    ))
                    .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(DIM))),
                    chunks[3],
                );
            })?;

            let Event::Key(k) = event::read()? else { continue };
            if k.kind != KeyEventKind::Press {
                continue;
            }
            let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
            match k.code {
                KeyCode::Esc => return Ok(None),
                KeyCode::Char('c') if ctrl => return Ok(None),
                KeyCode::Enter => return Ok(Some(input)),
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char('u') if ctrl => input.clear(),
                KeyCode::Char(c) => input.push(c),
                _ => {}
            }
        }
    }

    /// 危险命令的二次确认。
    pub fn confirm(&mut self, command: &str) -> Result<bool> {
        loop {
            self.term.draw(|f| {
                let area = f.area();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(3), Constraint::Length(2)])
                    .split(area);

                f.render_widget(
                    Paragraph::new(Span::styled(
                        " 这条命令被标记为危险",
                        Style::default().fg(WARN).add_modifier(Modifier::BOLD),
                    ))
                    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(WARN))),
                    chunks[0],
                );
                f.render_widget(
                    Paragraph::new(command.to_string())
                        .wrap(Wrap { trim: false })
                        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(DIM))),
                    chunks[1],
                );
                f.render_widget(
                    Paragraph::new(Span::styled(
                        "y 确认放到命令行上   n / esc 取消   （jot 不会替你执行）",
                        Style::default().fg(DIM),
                    )),
                    chunks[2],
                );
            })?;

            let Event::Key(k) = event::read()? else { continue };
            if k.kind != KeyEventKind::Press {
                continue;
            }
            match k.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => return Ok(true),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => return Ok(false),
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(false)
                }
                _ => {}
            }
        }
    }

    /// 多选（历史导入用）。
    pub fn multi_select(&mut self, title: &str, rows: &[String]) -> Result<Option<Vec<usize>>> {
        let mut checked = vec![false; rows.len()];
        let mut cursor = 0usize;
        let mut state = ListState::default();

        loop {
            state.select(Some(cursor));
            self.term.draw(|f| {
                let area = f.area();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(3), Constraint::Length(2)])
                    .split(area);

                let n = checked.iter().filter(|c| **c).count();
                f.render_widget(
                    Paragraph::new(Span::styled(
                        format!(" {title}   已选 {n}/{} ", rows.len()),
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                    ))
                    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(DIM))),
                    chunks[0],
                );

                let width = chunks[1].width.saturating_sub(6) as usize;
                let items: Vec<ListItem> = rows
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        let mark = if checked[i] { "◼ " } else { "◻ " };
                        ListItem::new(Line::from(vec![
                            Span::styled(
                                mark,
                                Style::default().fg(if checked[i] { ACCENT } else { DIM }),
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
                    &mut state,
                );

                f.render_widget(
                    Paragraph::new(Span::styled(
                        "space 勾选   a 全选   ⏎ 导入所选   esc 取消",
                        Style::default().fg(DIM),
                    ))
                    .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(DIM))),
                    chunks[2],
                );
            })?;

            let Event::Key(k) = event::read()? else { continue };
            if k.kind != KeyEventKind::Press {
                continue;
            }
            match k.code {
                KeyCode::Esc => return Ok(None),
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(None)
                }
                KeyCode::Enter => {
                    return Ok(Some(
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
