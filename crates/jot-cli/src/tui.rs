//! The terminal interface.
//!
//! Everything is drawn to stderr: stdout belongs to the result, which is how
//! the shell widget receives the final command (widget protocol, design doc 4.2).

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
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{backend::CrosstermBackend, Frame, Terminal};
use std::io::Stderr;

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;
const WARN: Color = Color::Yellow;

pub struct Ui {
    term: Terminal<CrosstermBackend<Stderr>>,
}

impl Ui {
    pub fn new() -> Result<Ui> {
        // In widget mode stdout is a pipe but stderr is still the terminal, which is where the UI goes
        if !std::io::IsTerminal::is_terminal(&std::io::stderr()) {
            anyhow::bail!("{}", t!("jot 需要一个真正的终端。要在脚本里用请加 --first，例如：\n  jot pick --query \"docker 日志\" --first", "jot needs a real terminal. For scripts add --first, for example:\n  jot pick --query \"docker logs\" --first"
            ));
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
    // Split on spaces and require every part to match, like fzf's AND semantics
    let mut total = 0i64;
    for part in needle.split_whitespace() {
        total += matcher.fuzzy_match(hay, part)?;
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
    }
    detail.push(Line::from(Span::styled(
        t!(
            "↑↓ 选择   ⏎ 使用   ^E 打开文件   esc 取消",
            "up/down move   enter use   ^E open file   esc cancel"
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

impl Ui {
    /// The picker.
    pub fn pick(&mut self, entries: &[&Entry], initial: &str, usage: &Usage) -> Result<Picked> {
        let matcher = SkimMatcherV2::default().ignore_case();
        let hays: Vec<String> = entries.iter().map(|e| e.haystack()).collect();
        // Frecency weights are precomputed; they are needed on every keystroke
        let now = jot_core::usage::now_secs();
        let boosts: Vec<i64> = entries.iter().map(|e| usage.boost(&e.id(), now)).collect();

        let mut query = initial.to_string();
        let mut cursor = 0usize;
        let mut state = ListState::default();

        loop {
            // Filter and rank
            let mut hits: Vec<(usize, i64)> = entries
                .iter()
                .enumerate()
                .filter_map(|(i, _)| score(&matcher, &hays[i], &query).map(|s| (i, s + boosts[i])))
                .collect();
            // Always sort: with an empty query this degrades to "most used first",
            // and equal scores keep file order thanks to sort_by_key being stable
            hits.sort_by_key(|h| std::cmp::Reverse(h.1));
            if cursor >= hits.len() {
                cursor = hits.len().saturating_sub(1);
            }
            state.select(if hits.is_empty() { None } else { Some(cursor) });

            self.term
                .draw(|f| draw_picker(f, entries, &hits, &query, &mut state))?;
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
            // If what was typed is not among the candidates, offer it as-is
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
                            .title(Span::styled(t!(" 命令 ", " command "), Style::default().fg(DIM))),
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
                        Span::styled(t!("使用输入的值  ", "use what you typed  "), Style::default().fg(WARN)),
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
                        t!("输入筛选或直接键入值   ↑↓ 选择   ⏎ 确定   esc 取消", "type to filter or enter a value   up/down move   enter confirm   esc cancel"),
                        Style::default().fg(DIM),
                    ))
                    .block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_style(Style::default().fg(DIM)),
                    ),
                    chunks[3],
                );
            })?;

            let Event::Key(k) = event::read()? else {
                continue;
            };
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

    /// Free text entry.
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
                            .title(Span::styled(
                                t!(" 命令 ", " command "),
                                Style::default().fg(DIM),
                            )),
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
                        t!("⏎ 确定   esc 取消", "enter confirm   esc cancel"),
                        Style::default().fg(DIM),
                    ))
                    .block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_style(Style::default().fg(DIM)),
                    ),
                    chunks[3],
                );
            })?;

            let Event::Key(k) = event::read()? else {
                continue;
            };
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

    /// Second confirmation for a dangerous command.
    pub fn confirm(&mut self, command: &str) -> Result<bool> {
        loop {
            self.term.draw(|f| {
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
                        t!(" 这条命令被标记为危险", " This command is marked dangerous"),
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
                    Paragraph::new(command.to_string())
                        .wrap(Wrap { trim: false })
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(DIM)),
                        ),
                    chunks[1],
                );
                f.render_widget(
                    Paragraph::new(Span::styled(
                        t!(
                            "y 确认放到命令行上   n / esc 取消   （jot 不会替你执行）",
                            "y put it on the prompt   n / esc cancel   (jot never runs it for you)"
                        ),
                        Style::default().fg(DIM),
                    )),
                    chunks[2],
                );
            })?;

            let Event::Key(k) = event::read()? else {
                continue;
            };
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

    /// Multi-select, used by history import.
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
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(3),
                        Constraint::Length(2),
                    ])
                    .split(area);

                let n = checked.iter().filter(|c| **c).count();
                f.render_widget(
                    Paragraph::new(Span::styled(
                        format!(" {title}   已选 {n}/{} ", rows.len()),
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
                        t!(
                            "space 勾选   a 全选   ⏎ 导入所选   esc 取消",
                            "space toggle   a select all   enter import   esc cancel"
                        ),
                        Style::default().fg(DIM),
                    ))
                    .block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_style(Style::default().fg(DIM)),
                    ),
                    chunks[2],
                );
            })?;

            let Event::Key(k) = event::read()? else {
                continue;
            };
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

#[cfg(test)]
mod tests {
    use super::*;
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
        use unicode_width::UnicodeWidthStr;

        let refs: Vec<&Entry> = owned.iter().collect();
        let hits: Vec<(usize, i64)> = (0..n_hits.min(refs.len())).map(|i| (i, 0)).collect();
        let mut state = ListState::default();
        state.select(selected);

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
