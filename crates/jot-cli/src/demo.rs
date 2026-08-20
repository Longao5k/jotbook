//! Renders the README's demo as an animated SVG.
//!
//! Not part of the binary: this is a `#[ignore]`d test that writes
//! `docs/demo.svg`, run on demand with
//!
//! ```text
//! cargo test -p jot-cli --bin jot -- --ignored demo
//! ```
//!
//! A recording would need someone at a keyboard, and a hand-drawn mockup drifts
//! from the real thing the first time a help line changes. These frames come
//! out of the same `draw_*` functions the picker uses, cell by cell and colour
//! by colour, so the demo is the actual interface or it is nothing.

use crate::tui::{draw_ask_text, draw_picker};
use jot_core::notebook::{self, Entry};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListState, Paragraph};
use ratatui::{Frame, Terminal};

const COLS: u16 = 84;
const ROWS: u16 = 20;

/// Cell size in pixels. Everything else is positioned from these.
const CW: f64 = 8.4;
const CH: f64 = 18.0;
const FONT_PX: f64 = 14.0;
const PAD: f64 = 18.0;

/// The frame that stands in when the animation does not run: the picker with a
/// query typed and results on screen, which is the one screen that explains
/// what jot is at a glance.
const POSTER: usize = 5;

/// The terminal the demo pretends to be running in.
///
/// Deliberately not any real theme: a demo has to stay readable on both of
/// GitHub's, so it carries its own dark ground rather than borrowing one.
const BG: &str = "#12141c";
const FG: &str = "#c9cdd6";

fn hex(c: Color) -> &'static str {
    match c {
        Color::Cyan => "#57d4dd",
        Color::DarkGray => "#5b6472",
        Color::Yellow => "#e0c07a",
        Color::Gray => "#98a0ad",
        Color::Rgb(30, 60, 62) => "#1b3a3d",
        _ => FG,
    }
}

/// One frame: the buffer contents plus how long it stays up.
struct Frame0 {
    cells: Vec<(u16, u16, String, Color, Color, bool)>,
    hold: f64,
}

fn capture(hold: f64, draw: impl FnOnce(&mut Frame)) -> Frame0 {
    let mut term = Terminal::new(TestBackend::new(COLS, ROWS)).unwrap();
    term.draw(draw).unwrap();
    let buf = term.backend().buffer();

    let mut cells = Vec::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let c = &buf[(x, y)];
            let sym = c.symbol();
            let bold = c.modifier.contains(Modifier::BOLD);
            if sym == " " && c.bg == Color::Reset {
                continue;
            }
            cells.push((x, y, sym.to_string(), c.fg, c.bg, bold));
        }
    }
    Frame0 { cells, hold }
}

/// Group a row's cells into runs sharing one style, so the SVG carries a few
/// hundred spans rather than one per character.
///
/// Runs stop at a double-width character: past one, an x derived from the
/// column no longer matches where the font actually put the glyph.
fn runs(f: &Frame0, row: u16) -> Vec<(u16, String, Color, Color, bool)> {
    use unicode_width::UnicodeWidthStr;

    let mut out: Vec<(u16, String, Color, Color, bool)> = Vec::new();
    let mut cells: Vec<_> = f.cells.iter().filter(|c| c.1 == row).collect();
    cells.sort_by_key(|c| c.0);

    for (x, _, sym, fg, bg, bold) in cells {
        let wide = sym.width() > 1;
        let joins = out.last().is_some_and(|(sx, s, f2, b2, o2)| {
            !wide && f2 == fg && b2 == bg && o2 == bold && *sx as usize + s.width() == *x as usize
        });
        if joins {
            out.last_mut().unwrap().1.push_str(sym);
        } else {
            out.push((*x, sym.clone(), *fg, *bg, *bold));
        }
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn svg(frames: &[Frame0]) -> String {
    let total: f64 = frames.iter().map(|f| f.hold).sum();
    let w = COLS as f64 * CW + PAD * 2.0;
    let h = ROWS as f64 * CH + PAD * 2.0 + 26.0;

    let mut s = String::new();
    s.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:.0} {h:.0}" width="{w:.0}" height="{h:.0}" font-family="ui-monospace,SFMono-Regular,'Cascadia Code','JetBrains Mono',Menlo,Consolas,monospace" font-size="{FONT_PX}">
<style>
  .f {{ opacity: 0 }}
  text {{ white-space: pre }}
  /* Every frame is hidden until its turn, so somewhere that will not run the
     animation - a viewer that strips it, a reader with reduced motion - would
     otherwise get an empty box. One frame stays visible as the still. */
  #f{POSTER} {{ opacity: 1 }}
  @media (prefers-reduced-motion: reduce) {{
    .f {{ animation: none !important }}
  }}
"#
    ));

    // One keyframes rule per frame, each holding its slice of the timeline.
    //
    // Two things keep the frames from bleeding into one another. `step-end`
    // stops opacity being interpolated between keyframes, and every rule
    // states its value at 0% - without that, a frame starting at 40% has no
    // value to hold before then, so the browser ramps it up from the base
    // across those 40%, and every frame fades in on top of its predecessor.
    let mut at = 0.0f64;
    for (i, f) in frames.iter().enumerate() {
        let start = at / total * 100.0;
        let end = ((at + f.hold) / total * 100.0).min(100.0);
        s.push_str(&format!(
            "  #f{i} {{ animation: k{i} {total:.2}s step-end infinite }}\n"
        ));
        if i == 0 {
            s.push_str(&format!(
                "  @keyframes k0 {{ 0% {{ opacity: 1 }} {end:.4}% {{ opacity: 0 }} }}\n"
            ));
        } else if end >= 99.999 {
            s.push_str(&format!(
                "  @keyframes k{i} {{ 0% {{ opacity: 0 }} {start:.4}% {{ opacity: 1 }} }}\n"
            ));
        } else {
            s.push_str(&format!(
                "  @keyframes k{i} {{ 0% {{ opacity: 0 }} {start:.4}% {{ opacity: 1 }} {end:.4}% {{ opacity: 0 }} }}\n"
            ));
        }
        at += f.hold;
    }

    s.push_str("</style>\n");
    s.push_str(&format!(
        r##"<rect width="100%" height="100%" rx="8" fill="{BG}"/>
<circle cx="{}" cy="16" r="5" fill="#e06c60"/><circle cx="{}" cy="16" r="5" fill="#e0c07a"/><circle cx="{}" cy="16" r="5" fill="#8bc46a"/>
"##,
        PAD,
        PAD + 17.0,
        PAD + 34.0
    ));

    let top = PAD + 26.0;
    for (i, f) in frames.iter().enumerate() {
        s.push_str(&format!("<g class=\"f\" id=\"f{i}\">\n"));
        for row in 0..ROWS {
            let line = runs(f, row);
            if line.is_empty() {
                continue;
            }
            // Backgrounds first, so the selection bar sits behind its text
            for (x, text, _, bg, _) in &line {
                if *bg == Color::Reset {
                    continue;
                }
                use unicode_width::UnicodeWidthStr;
                s.push_str(&format!(
                    r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{CH:.0}" fill="{}"/>"#,
                    PAD + *x as f64 * CW,
                    top + row as f64 * CH,
                    text.width() as f64 * CW,
                    hex(*bg)
                ));
            }
            s.push_str(&format!(
                r#"<text y="{:.1}">"#,
                top + row as f64 * CH + FONT_PX
            ));
            for (x, text, fg, _, bold) in &line {
                s.push_str(&format!(
                    r#"<tspan x="{:.1}" fill="{}"{}>{}</tspan>"#,
                    PAD + *x as f64 * CW,
                    hex(*fg),
                    if *bold { r#" font-weight="600""# } else { "" },
                    esc(text)
                ));
            }
            s.push_str("</text>\n");
        }
        s.push_str("</g>\n");
    }
    s.push_str("</svg>\n");
    s
}

/// Real entries, taken verbatim from `notebooks/en/docker.md`.
const SRC: &str = "---\nname: docker\n---\n\n\
## Follow a container's logs\n\n\
```sh\ndocker logs -f --tail 200 {{container}}\n```\n\n\
## Recent logs with timestamps\n\n\
```sh\ndocker logs --timestamps --since 30m {{container}}\n```\n\n\
## Get a shell in a container\n\n\
Alpine images only have sh, not bash.\n\n\
```sh\ndocker exec -it {{container}} {{sh}}\n```\n\n\
## List running containers\n\n\
```sh\ndocker ps\n```\n";

/// A shell prompt with the finished command on it, waiting for Enter.
///
/// The one frame that is not a screen jot draws - jot has already exited by
/// this point, which is exactly the thing worth showing.
fn prompt(hold: f64, typed: &str, hint: &str) -> Frame0 {
    use ratatui::style::Style;

    capture(hold, |f: &mut Frame| {
        let area = f.area();
        f.render_widget(
            Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("PS C:\\work> ", Style::default().fg(Color::Cyan)),
                    Span::styled(typed, Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled("▏", Style::default().fg(Color::Cyan)),
                ]),
                Line::from(""),
                Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray))),
            ]),
            Rect::new(0, 1, area.width, 4),
        );
    })
}

#[test]
#[ignore = "writes docs/demo.svg; run with --ignored when the interface changes"]
fn demo() {
    jot_core::i18n::set(jot_core::i18n::Lang::En);

    let owned: Vec<Entry> = notebook::parse(std::path::Path::new("docker.md"), SRC)
        .unwrap()
        .entries;
    let refs: Vec<&Entry> = owned.iter().collect();

    // Which entries match, at each stage of typing "docker log"
    let picker = |hold: f64, query: &str, hits: &[usize], sel: usize| {
        let hits: Vec<(usize, i64)> = hits.iter().map(|i| (*i, 0)).collect();
        let mut state = ListState::default();
        state.select(Some(sel));
        capture(hold, |f: &mut Frame| {
            draw_picker(f, &refs, &hits, query, &mut state)
        })
    };

    let all = [0, 1, 2, 3];
    let logs = [0, 1];
    let mut frames = vec![
        prompt(1.4, "", "Ctrl+J"),
        picker(1.1, "", &all, 0),
        picker(0.28, "d", &all, 0),
        picker(0.28, "doc", &all, 0),
        picker(0.28, "docker l", &logs, 0),
        picker(1.7, "docker log", &logs, 0),
        picker(1.2, "docker log", &logs, 1),
    ];

    // Filling in the one variable the entry needs
    let ctx = "docker logs --timestamps --since 30m ⟨container⟩";
    for (hold, typed) in [(0.9, ""), (0.3, "api"), (1.4, "api-worker")] {
        frames.push(capture(hold, |f: &mut Frame| {
            draw_ask_text(f, ctx, "container", typed)
        }));
    }

    frames.push(prompt(
        3.4,
        "docker logs --timestamps --since 30m api-worker",
        "jot never presses Enter for you",
    ));

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/demo.svg")
        .canonicalize()
        .unwrap_or_else(|_| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/demo.svg")
        });
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(&out, svg(&frames)).unwrap();
    eprintln!("wrote {} ({} frames)", out.display(), frames.len());
}
