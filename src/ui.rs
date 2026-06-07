//! All rendering: the per-frame draw plus widget-styling helpers.

use crate::app::{App, PALETTE};
use crate::model::{HomeFocus, InputKind, ToastKind, View};
use html2text::render::RichAnnotation;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Padding, Paragraph},
};

const BANNER: &str = r#"
███████╗ ██████╗██████╗  ██████╗ ██╗     ██╗     ███████╗
██╔════╝██╔════╝██╔══██╗██╔═══██╗██║     ██║     ██╔════╝
███████╗██║     ██████╔╝██║   ██║██║     ██║     ███████╗
╚════██║██║     ██╔══██╗██║   ██║██║     ██║     ╚════██║
███████║╚██████╗██║  ██║╚██████╔╝███████╗███████╗███████║
╚══════╝ ╚═════╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚══════╝╚══════╝
                  your terminal scrolls
"#;

// Per-page shortcut hints, shown bottom-right on each view's block.
const CATEGORIES_HINT: &str = " ↑↓ · →/Enter feeds · p color · q quit ";
const FEEDS_HINT: &str =
    " ↑↓ · Enter open · a add · c cat · d del · i/e opml · r refresh · ← back ";
const ARTICLES_HINT: &str =
    " ↑↓ · Enter read · / search · s save · t toggle · A mark all · o open · Esc back ";
const READER_HINT: &str = " ↑↓ scroll · ←→ prev/next · f full · o open · Esc back · q quit ";

/// Draw the whole UI for the current frame.
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    match app.view {
        View::Home => render_home(frame, app, area),
        View::Articles => render_articles(frame, app, area),
        View::Reader => render_reader(frame, app, area),
    }

    // color-picker popup (centered), when open
    if let Some(picker) = &mut app.color_picker {
        let mut items: Vec<Line> = PALETTE
            .iter()
            .map(|(name, color)| {
                Line::from(vec![
                    Span::styled("███ ", Style::default().fg(*color)),
                    Span::raw(*name),
                ])
            })
            .collect();
        items.push(Line::from("custom (hex)"));

        let height = items.len() as u16 + 2;
        let rect = centered_rect(28, height, area);
        let list = List::new(items)
            .block(block(&format!("Color: {}", picker.category)))
            .highlight_style(highlight_style())
            .highlight_symbol(">> ");
        frame.render_widget(Clear, rect);
        frame.render_stateful_widget(list, rect, &mut picker.state);
    }

    // toast overlay (bottom-right), drawn on top of everything
    if let Some(toast) = &app.toast {
        let width = (toast.message.chars().count() as u16 + 4).clamp(10, area.width.max(10));
        let rect = Rect {
            x: area.width.saturating_sub(width),
            y: area.height.saturating_sub(3),
            width,
            height: 3,
        };
        let (color, label) = match toast.kind {
            ToastKind::Info => (Color::Green, "info"),
            ToastKind::Error => (Color::Red, "error"),
        };
        let widget = Paragraph::new(toast.message.as_str())
            .style(Style::default().fg(color))
            .block(block(label).border_style(Style::default().fg(color)));
        frame.render_widget(Clear, rect);
        frame.render_widget(widget, rect);
    }
}

fn render_home(frame: &mut Frame, app: &mut App, area: Rect) {
    let rows = if app.input.is_some() {
        Layout::vertical([
            Constraint::Length(8),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area)
    } else {
        Layout::vertical([Constraint::Length(8), Constraint::Min(0)]).split(area)
    };

    let banner = Paragraph::new(BANNER)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::LightBlue));
    frame.render_widget(banner, rows[0]);

    let [cat_area, feed_area] =
        Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)]).areas(rows[1]);

    // left pane: categories with the number of feeds in each
    let cats = app.categories();
    let cat_items: Vec<Line> = cats
        .iter()
        .map(|c| {
            let feed_count = app
                .feeds
                .iter()
                .filter(|f| c == "All" || &f.category == c)
                .count();
            feed_line(c, feed_count, app.category_color(c))
        })
        .collect();
    let cats_focused = app.focus == HomeFocus::Categories;
    let cat_sel_color = app
        .categories_state
        .selected()
        .and_then(|i| cats.get(i))
        .map(|c| app.category_color(c))
        .unwrap_or(Color::LightBlue);
    let cat_list = List::new(cat_items)
        .block(pane_block("Categories", CATEGORIES_HINT, cats_focused))
        .highlight_style(selection_style(cat_sel_color, cats_focused))
        .highlight_symbol(">> ");
    frame.render_stateful_widget(cat_list, cat_area, &mut app.categories_state);

    // right pane: feeds in the highlighted category, with article counts
    let feed_items: Vec<Line> = app
        .visible_feed_indices()
        .iter()
        .map(|&i| {
            let f = &app.feeds[i];
            feed_line(&f.title, f.articles.len(), app.category_color(&f.category))
        })
        .collect();
    let feeds_focused = app.focus == HomeFocus::Feeds;
    let feed_sel_color = app
        .current_feed()
        .map(|f| app.category_color(&f.category))
        .unwrap_or(Color::LightBlue);
    let feed_list = List::new(feed_items)
        .block(pane_block("Feeds", FEEDS_HINT, feeds_focused))
        .highlight_style(selection_style(feed_sel_color, feeds_focused))
        .highlight_symbol(">> ");
    frame.render_stateful_widget(feed_list, feed_area, &mut app.feeds_state);

    if let Some((kind, buf)) = &app.input {
        let title = match kind {
            InputKind::AddFeedUrl => "Feed URL - Enter for category, Esc cancel",
            InputKind::AddFeedCategory(_) => "Category (blank = Uncategorized) - Enter to add",
            InputKind::SetCategory(_) => "Category - Enter to set, Esc cancel",
            InputKind::SetColor(_) => "Color - name (e.g. lightblue) or #rrggbb, Enter to set",
            InputKind::ImportOpml => "OPML file path - Enter to import, Esc cancel",
            InputKind::SearchArticles => "Search",
        };
        let input = Paragraph::new(buf.as_str()).block(block(title));
        frame.render_widget(input, rows[2]);
    }
}

fn render_articles(frame: &mut Frame, app: &mut App, area: Rect) {
    let searching = matches!(&app.input, Some((InputKind::SearchArticles, _)));
    let rows = if searching {
        Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(area)
    } else {
        Layout::vertical([Constraint::Min(0)]).split(area)
    };

    let feed_title = app
        .current_feed()
        .map(|f| f.title.clone())
        .unwrap_or_default();
    let color = app
        .current_feed()
        .map(|f| app.category_color(&f.category))
        .unwrap_or(Color::LightBlue);
    let title = match app.article_query() {
        Some(q) if !q.is_empty() => format!("{feed_title} - search: {q}"),
        _ => feed_title,
    };

    // wrap titles to the inner band, minus the markers (4) and highlight symbol (3)
    let pad_x = band_padding(rows[0].width);
    let wrap_width = (rows[0].width.saturating_sub(2 + pad_x * 2) as usize).saturating_sub(7);

    let fi = app.current_feed_idx();
    let items: Vec<ListItem> = app
        .visible_article_indices()
        .iter()
        .filter_map(|&ai| fi.map(|fi| &app.feeds[fi].articles[ai]))
        .map(|a| {
            let modifier = if a.read {
                Modifier::DIM
            } else {
                Modifier::BOLD
            };
            let style = Style::default().fg(color).add_modifier(modifier);
            let dot = if a.read { "  " } else { "● " };
            let star = if a.saved {
                Span::styled("★ ", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("  ")
            };
            let wrapped = wrap_text(&a.title, wrap_width);
            let lines: Vec<Line> = wrapped
                .into_iter()
                .enumerate()
                .map(|(i, line)| {
                    if i == 0 {
                        Line::from(vec![
                            Span::styled(dot, style),
                            star.clone(),
                            Span::styled(line, style),
                        ])
                    } else {
                        Line::from(Span::styled(format!("    {line}"), style))
                    }
                })
                .collect();
            ListItem::new(lines)
        })
        .collect();

    let list = List::new(items)
        .block(page_block(&title, ARTICLES_HINT, pad_x, color))
        .highlight_style(selection_style(color, true))
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, rows[0], &mut app.articles_state);

    if let Some((InputKind::SearchArticles, buf)) = &app.input {
        let input =
            Paragraph::new(buf.as_str()).block(block("Search - type to filter, Esc cancel"));
        frame.render_widget(input, rows[1]);
    }
}

fn render_reader(frame: &mut Frame, app: &mut App, area: Rect) {
    let article = app.current_article();
    let title = article.map(|a| a.title.clone()).unwrap_or_default();
    let color = app
        .current_feed()
        .map(|f| app.category_color(&f.category))
        .unwrap_or(Color::LightBlue);

    // full-width box, but pad the sides so text sits in a centered ~80-col band
    let pad_x = band_padding(area.width);
    // inner text width = area minus borders (2) minus side padding
    let content_width = area.width.saturating_sub(2 + pad_x * 2).max(10) as usize;

    let mut lines: Vec<Line> = Vec::new();
    if let Some(a) = article {
        if !a.link.is_empty() {
            lines.push(Line::from(Span::styled(
                a.link.clone(),
                Style::default()
                    .fg(color)
                    .add_modifier(Modifier::UNDERLINED),
            )));
            lines.push(Line::from(""));
        }
        lines.extend(render_html(&a.body_html, content_width));
    }

    // clamp scroll so we can't run past the end
    let inner_height = area.height.saturating_sub(2 + 2); // borders + vert padding
    let max_scroll = (lines.len() as u16).saturating_sub(inner_height);
    app.scroll = app.scroll.min(max_scroll);

    let reader = Paragraph::new(lines)
        .scroll((app.scroll, 0))
        .block(page_block(&title, READER_HINT, pad_x, color));
    frame.render_widget(reader, area);
}

/// A `w` x `h` rectangle centered within `area` (clamped to fit).
fn centered_rect(w: u16, h: u16, area: Rect) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(w) / 2,
        y: area.y + area.height.saturating_sub(h) / 2,
        width: w.min(area.width),
        height: h.min(area.height),
    }
}

fn block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "))
        .border_type(BorderType::Rounded)
}

/// A bordered block with `title` top-left and a right-aligned `hint` along the bottom.
fn hint_block(title: &str, hint: &'static str) -> Block<'static> {
    block(title).title_bottom(Line::from(hint).right_aligned())
}

/// Shared look for the Articles and Reader pages: centered bold title,
/// right-aligned hint, and a padded body band.
fn page_block(title: &str, hint: &'static str, pad_x: u16, title_color: Color) -> Block<'static> {
    hint_block(title, hint)
        .padding(Padding::symmetric(pad_x, 1))
        .title_alignment(Alignment::Center)
        .title_style(
            Style::default()
                .fg(title_color)
                .add_modifier(Modifier::BOLD),
        )
}

/// Border + title styling for a Home pane, brighter when focused.
fn pane_block(title: &str, hint: &'static str, focused: bool) -> Block<'static> {
    let color = if focused {
        Color::LightBlue
    } else {
        Color::DarkGray
    };
    hint_block(title, hint)
        .border_style(Style::default().fg(color))
        .title_style(Style::default().fg(color).add_modifier(Modifier::BOLD))
}

/// Horizontal padding that centers an ~80-col band in `width`.
fn band_padding(width: u16) -> u16 {
    (width.saturating_sub(80) / 2).max(2)
}

fn highlight_style() -> Style {
    Style::default()
        .bg(Color::LightBlue)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD)
}

/// Black or white text, whichever reads better on `bg`.
fn contrast_for(bg: Color) -> Color {
    let light = match bg {
        Color::Rgb(r, g, b) => 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32 > 140.0,
        // bright backgrounds want dark text
        Color::White
        | Color::Gray
        | Color::Yellow
        | Color::LightYellow
        | Color::Green
        | Color::LightGreen
        | Color::Cyan
        | Color::LightCyan
        | Color::LightRed
        | Color::LightBlue
        | Color::LightMagenta => true,
        // dark backgrounds want light text
        _ => false,
    };
    if light { Color::Black } else { Color::White }
}

/// Selection highlight: a `bg`-colored bar with contrasting text when focused,
/// subtle bold when not.
fn selection_style(bg: Color, focused: bool) -> Style {
    if focused {
        Style::default()
            .bg(bg)
            .fg(contrast_for(bg))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    }
}

/// A list line: a `color`-tinted name plus a bold `(N)` count badge when nonzero.
fn feed_line(name: &str, count: usize, color: Color) -> Line<'static> {
    let name_span = Span::styled(name.to_string(), Style::default().fg(color));
    if count > 0 {
        Line::from(vec![
            name_span,
            Span::styled(
                format!("  ({count})"),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        Line::from(name_span)
    }
}

/// Greedily word-wrap `text` to `width` columns; never returns empty.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if cur.is_empty() {
            cur.push_str(word);
        } else if cur.chars().count() + 1 + word.chars().count() <= width {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(std::mem::take(&mut cur));
            cur.push_str(word);
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Build a ratatui Style from the stack of HTML annotations on a text run.
fn style_for(tags: &[RichAnnotation]) -> Style {
    let mut style = Style::default();
    for tag in tags {
        style = match tag {
            RichAnnotation::Strong => style.fg(Color::White).add_modifier(Modifier::BOLD),
            RichAnnotation::Emphasis => style.add_modifier(Modifier::ITALIC),
            RichAnnotation::Strikeout => style.add_modifier(Modifier::CROSSED_OUT),
            RichAnnotation::Code => style.fg(Color::Yellow),
            // code blocks get a dark background so they read as a block
            RichAnnotation::Preformat(_) => style.fg(Color::Yellow).bg(Color::Rgb(40, 40, 40)),
            RichAnnotation::Link(_) => style.fg(Color::Cyan).add_modifier(Modifier::UNDERLINED),
            _ => style,
        };
    }
    style
}

/// Render HTML into styled lines, wrapped to `width`.
fn render_html(html: &str, width: usize) -> Vec<Line<'static>> {
    let parsed = match html2text::from_read_rich(html.as_bytes(), width.max(1)) {
        Ok(lines) => lines,
        Err(_) => return vec![Line::from(html.to_string())],
    };

    parsed
        .iter()
        .map(|line| {
            // code-block lines keep their run styling verbatim (a "#" inside code
            // is not a heading)
            let is_pre = line.tagged_strings().any(|ts| {
                ts.tag
                    .iter()
                    .any(|t| matches!(t, RichAnnotation::Preformat(_)))
            });

            if !is_pre {
                let text: String = line.tagged_strings().map(|ts| ts.s.as_str()).collect();
                let trimmed = text.trim_start();

                // heading: "# " .. "###### "
                let level = trimmed.chars().take_while(|&c| c == '#').count();
                if (1..=6).contains(&level) && trimmed[level..].starts_with(' ') {
                    return Line::from(Span::styled(
                        trimmed[level + 1..].to_string(),
                        Style::default()
                            .fg(Color::LightCyan)
                            .add_modifier(Modifier::BOLD),
                    ));
                }

                // blockquote: "> "
                if let Some(rest) = trimmed.strip_prefix("> ") {
                    return Line::from(vec![
                        Span::styled("│ ", Style::default().fg(Color::Cyan)),
                        Span::styled(
                            rest.to_string(),
                            Style::default()
                                .fg(Color::Gray)
                                .add_modifier(Modifier::ITALIC),
                        ),
                    ]);
                }
            }

            let spans: Vec<Span> = line
                .tagged_strings()
                .map(|ts| Span::styled(ts.s.clone(), style_for(&ts.tag)))
                .collect();
            Line::from(spans)
        })
        .collect()
}
