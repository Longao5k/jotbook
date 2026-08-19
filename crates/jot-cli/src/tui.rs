//! 终端界面。
//!
//! 全部画到 stderr —— stdout 要留给结果本身，shell widget 靠它拿到最终命令
//! （见设计文档 §4.2 的 widget 协议）。

use anyhow::Result;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use jot_core::notebook::Entry;
use jot_core::resolve::Choice;
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

/// 按**终端列数**截断，不是字符数。
///
/// 中文一个字占两列。用 `chars().count()` 的话中文标题根本不会触发省略号，
/// 而是被终端直接切掉 —— 行尾的笔记本名和 ⚠ 标记会一起消失。内置笔记本
/// 全是中文标题，所以这条一定会踩到。
fn clamp_line(s: &str, max: usize) -> String {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

    let s = s.replace('\n', " ⏎ ");
    if s.width() <= max {
        return s;
    }
    // 留一列给省略号
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

/// 主选择器的渲染。抽成自由函数是为了能用 ratatui 的 TestBackend 离线断言 ——
/// 交互没法在测试里驱动，但「画出来对不对、窄终端会不会 panic」可以。
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

    // ── 搜索框 ──
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
}

impl Ui {
    /// 主选择器。
    pub fn pick(&mut self, entries: &[&Entry], initial: &str, usage: &Usage) -> Result<Picked> {
        let matcher = SkimMatcherV2::default().ignore_case();
        let hays: Vec<String> = entries.iter().map(|e| e.haystack()).collect();
        // 常用度加权预先算好，每次按键都要用
        let now = jot_core::usage::now_secs();
        let boosts: Vec<i64> = entries.iter().map(|e| usage.boost(&e.id(), now)).collect();

        let mut query = initial.to_string();
        let mut cursor = 0usize;
        let mut state = ListState::default();

        loop {
            // 过滤 + 排序
            let mut hits: Vec<(usize, i64)> = entries
                .iter()
                .enumerate()
                .filter_map(|(i, _)| score(&matcher, &hays[i], &query).map(|s| (i, s + boosts[i])))
                .collect();
            // 总是排序：搜索词为空时这就退化成「按常用度排」，
            // 分数相同的靠 sort_by_key 的稳定性保持原有文件顺序
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
                    Paragraph::new(Span::styled("⏎ 确定   esc 取消", Style::default().fg(DIM)))
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

    /// 危险命令的二次确认。
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
                        " 这条命令被标记为危险",
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
                        "y 确认放到命令行上   n / esc 取消   （jot 不会替你执行）",
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
                        "space 勾选   a 全选   ⏎ 导入所选   esc 取消",
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
## 重启后端服务\n\n\
改完配置必须重启。\n\n\
```sh @confirm\nsudo systemctl restart {{service}}\n```\n\n\
## 查看日志\n\n\
```sh\njournalctl -f\n```\n";

    fn entries() -> Vec<notebook::Entry> {
        notebook::parse(Path::new("demo.md"), SRC).unwrap().entries
    }

    /// 把渲染出来的缓冲区导成纯文本，方便断言。
    ///
    /// CJK 是双宽字符，ratatui 用两个单元格存一个字 —— 必须按显示宽度推进，
    /// 否则导出来是「重 启 后 端」这种带空格的东西。
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

    /// 回归：截断曾经按字符数算，中文标题因此从不触发省略号，
    /// 而是被终端直接切掉，行尾的笔记本名和 ⚠ 一起消失。
    #[test]
    fn clamp_measures_columns_not_characters() {
        use unicode_width::UnicodeWidthStr;

        let cjk = "重启后端服务的那条命令"; // 11 字 = 22 列
        assert_eq!(cjk.width(), 22);
        assert_eq!(clamp_line(cjk, 30), cjk, "够宽的时候不该截断");

        let cut = clamp_line(cjk, 10);
        assert!(cut.ends_with('…'), "没有省略号: {cut}");
        assert!(
            cut.width() <= 10,
            "截断后仍然占了 {} 列: {cut}",
            cut.width()
        );

        // 纯 ASCII 的行为不变
        assert_eq!(clamp_line("abcdefghij", 5), "abcd…");
        assert_eq!(clamp_line("abc", 5), "abc");
    }

    #[test]
    fn draws_query_titles_and_counts() {
        let s = render(80, 24, "重启", 2, Some(0));
        assert!(s.contains("重启"), "搜索词没画出来:\n{s}");
        assert!(s.contains("重启后端服务"), "条目标题没画出来:\n{s}");
        assert!(s.contains("2/2"), "计数不对:\n{s}");
        assert!(s.contains("demo"), "笔记本名没画出来:\n{s}");
    }

    #[test]
    fn variables_render_as_readable_placeholders() {
        let s = render(80, 24, "", 2, Some(0));
        assert!(s.contains("⟨service⟩"), "变量没换成可读占位符:\n{s}");
        assert!(!s.contains("{{service}}"), "列表里直接显示了花括号:\n{s}");
    }

    #[test]
    fn dangerous_entries_are_marked() {
        let s = render(80, 24, "", 2, Some(0));
        assert!(s.contains('⚠'), "危险命令没有标记:\n{s}");
        assert!(s.contains("危险命令"), "详情区没有提示:\n{s}");
    }

    #[test]
    fn selection_marker_follows_the_cursor() {
        assert!(render(80, 24, "", 2, Some(0)).contains('❯'));
        let second = render(80, 24, "", 2, Some(1));
        let line = second
            .lines()
            .find(|l| l.contains('❯'))
            .expect("没有选中标记");
        assert!(line.contains("查看日志"), "选中标记停在了错误的行: {line}");
    }

    /// 窄终端和极端尺寸是 TUI 最常见的 panic 来源。
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
            let _ = render(w, h, "重启", 2, Some(0));
            let _ = render(w, h, "", 2, None);
        }
    }

    #[test]
    fn empty_result_set_does_not_panic() {
        let s = render(80, 24, "没有任何东西能匹配这一串", 0, None);
        assert!(s.contains("0/2"), "空结果的计数不对:\n{s}");
    }

    #[test]
    fn long_content_is_truncated_with_an_ellipsis() {
        // 窄终端下超长的标题和命令必须被截断，而不是把布局撑破
        let src = "---
name: demo
---

## 一个特别特别特别特别特别特别特别特别长的标题用来验证截断

```sh
echo 这是一条同样特别特别特别特别特别特别长的命令内容 {{var}}
```
";
        let owned = notebook::parse(Path::new("demo.md"), src).unwrap().entries;
        let s = render_entries(&owned, 40, 20, "", 1, Some(0));
        assert!(
            s.contains('…'),
            "窄终端下内容没有被截断:
{s}"
        );
        assert_eq!(
            s.lines().count(),
            20,
            "渲染出来的行数和终端高度对不上:
{s}"
        );
    }
}
