//! All rendering: the per-frame draw plus widget-styling helpers.

use crate::app::App;
use crate::model::{HomeFocus, InputKind, View};
use html2text::render::RichAnnotation;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, Padding, Paragraph},
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
const CATEGORIES_HINT: &str = " ↑↓ · →/Enter feeds · q quit ";
const FEEDS_HINT: &str =
    " ↑↓ · Enter open · a add · c cat · d del · i/e opml · r refresh · ← back ";
const ARTICLES_HINT: &str = " ↑↓ move · Enter read · A mark all · o open · Esc back · q quit ";
const READER_HINT: &str = " ↑↓ scroll · ←→ prev/next · o open · Esc back · q quit ";

/// Draw the whole UI for the current frame.
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    match app.view {
        View::Home => render_home(frame, app, area),
        View::Articles => render_articles(frame, app, area),
        View::Reader => render_reader(frame, app, area),
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
        let widget = Paragraph::new(toast.message.as_str())
            .style(Style::default().fg(Color::Red))
            .block(block("error").border_style(Style::default().fg(Color::Red)));
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
            feed_line(c, feed_count)
        })
        .collect();
    let cats_focused = app.focus == HomeFocus::Categories;
    let cat_list = List::new(cat_items)
        .block(pane_block("Categories", CATEGORIES_HINT, cats_focused))
        .highlight_style(selection_style(cats_focused))
        .highlight_symbol(">> ");
    frame.render_stateful_widget(cat_list, cat_area, &mut app.categories_state);

    // right pane: feeds in the highlighted category, with article counts
    let feed_items: Vec<Line> = app
        .visible_feed_indices()
        .iter()
        .map(|&i| {
            let f = &app.feeds[i];
            feed_line(&f.title, f.articles.len())
        })
        .collect();
    let feeds_focused = app.focus == HomeFocus::Feeds;
    let feed_list = List::new(feed_items)
        .block(pane_block("Feeds", FEEDS_HINT, feeds_focused))
        .highlight_style(selection_style(feeds_focused))
        .highlight_symbol(">> ");
    frame.render_stateful_widget(feed_list, feed_area, &mut app.feeds_state);

    if let Some((kind, buf)) = &app.input {
        let title = match kind {
            InputKind::AddFeedUrl => "Feed URL — Enter for category, Esc cancel",
            InputKind::AddFeedCategory(_) => "Category (blank = Uncategorized) — Enter to add",
            InputKind::SetCategory(_) => "Category — Enter to set, Esc cancel",
            InputKind::ImportOpml => "OPML file path — Enter to import, Esc cancel",
        };
        let input = Paragraph::new(buf.as_str()).block(block(title));
        frame.render_widget(input, rows[2]);
    }
}

fn render_articles(frame: &mut Frame, app: &mut App, area: Rect) {
    let title = app
        .current_feed()
        .map(|f| f.title.clone())
        .unwrap_or_default();
    let items: Vec<Line> = app
        .current_feed()
        .map(|f| {
            f.articles
                .iter()
                .map(|a| {
                    let (marker, style) = if a.read {
                        ("  ", Style::default().fg(Color::White))
                    } else {
                        (
                            "● ",
                            Style::default()
                                .fg(Color::LightBlue)
                                .add_modifier(Modifier::BOLD),
                        )
                    };
                    Line::from(Span::styled(format!("{marker}{}", a.title), style))
                })
                .collect()
        })
        .unwrap_or_default();
    let list = List::new(items)
        .block(page_block(&title, ARTICLES_HINT, band_padding(area.width)))
        .highlight_style(highlight_style())
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, area, &mut app.articles_state);
}

fn render_reader(frame: &mut Frame, app: &mut App, area: Rect) {
    let article = app.current_article();
    let title = article.map(|a| a.title.clone()).unwrap_or_default();

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
                    .fg(Color::Cyan)
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
        .block(page_block(&title, READER_HINT, pad_x));
    frame.render_widget(reader, area);
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
fn page_block(title: &str, hint: &'static str, pad_x: u16) -> Block<'static> {
    hint_block(title, hint)
        .padding(Padding::symmetric(pad_x, 1))
        .title_alignment(Alignment::Center)
        .title_style(
            Style::default()
                .fg(Color::LightBlue)
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

/// List highlight: full bar on the focused pane, subtle on the unfocused one.
fn selection_style(focused: bool) -> Style {
    if focused {
        highlight_style()
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    }
}

/// A list line: name plus a bold `(N)` count badge when nonzero.
fn feed_line(name: &str, count: usize) -> Line<'static> {
    if count > 0 {
        Line::from(vec![
            Span::raw(name.to_string()),
            Span::styled(
                format!("  ({count})"),
                Style::default()
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        Line::from(name.to_string())
    }
}

/// Build a ratatui Style from the stack of HTML annotations on a text run.
fn style_for(tags: &[RichAnnotation]) -> Style {
    let mut style = Style::default();
    for tag in tags {
        style = match tag {
            RichAnnotation::Strong => style.fg(Color::White).add_modifier(Modifier::BOLD),
            RichAnnotation::Emphasis => style.add_modifier(Modifier::ITALIC),
            RichAnnotation::Strikeout => style.add_modifier(Modifier::CROSSED_OUT),
            RichAnnotation::Code | RichAnnotation::Preformat(_) => style.fg(Color::Yellow),
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
            let spans: Vec<Span> = line
                .tagged_strings()
                .map(|ts| Span::styled(ts.s.clone(), style_for(&ts.tag)))
                .collect();
            Line::from(spans)
        })
        .collect()
}
