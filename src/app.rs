//! Application state and the actions the UI invokes on it.

use crate::db::{
    cache_articles, data_path, delete_feed, load_articles, load_category_colors, mark_all_read,
    mark_read, save_feed, set_category_color, set_feed_category,
};
use crate::feed::{collect_feeds, spawn_fetch};
use crate::model::{DEFAULT_CATEGORY, Feed, FetchResult, HomeFocus, InputKind, Toast, View};
use opml::OPML;
use ratatui::style::Color;
use ratatui::widgets::ListState;
use rusqlite::Connection;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

pub const REFRESH_INTERVAL: Duration = Duration::from_secs(600);

/// Selectable colors in the picker. Names must parse via `Color::from_str`.
pub const PALETTE: &[(&str, Color)] = &[
    ("red", Color::Red),
    ("green", Color::Green),
    ("yellow", Color::Yellow),
    ("blue", Color::Blue),
    ("magenta", Color::Magenta),
    ("cyan", Color::Cyan),
    ("gray", Color::Gray),
    ("white", Color::White),
    ("lightred", Color::LightRed),
    ("lightgreen", Color::LightGreen),
    ("lightyellow", Color::LightYellow),
    ("lightblue", Color::LightBlue),
    ("lightmagenta", Color::LightMagenta),
    ("lightcyan", Color::LightCyan),
];

/// Open color-picker popup state: which category, and the highlighted row.
pub struct ColorPicker {
    pub category: String,
    pub state: ListState,
}

pub struct App {
    pub feeds: Vec<Feed>,
    pub categories_state: ListState,
    pub feeds_state: ListState,
    pub articles_state: ListState,
    pub focus: HomeFocus,
    pub view: View,
    pub scroll: u16,
    pub input: Option<(InputKind, String)>,
    pub toast: Option<Toast>,
    pub pending: usize,
    pub last_refresh: Instant,
    pub category_colors: HashMap<String, Color>,
    pub color_picker: Option<ColorPicker>,
    pub conn: Connection,
    pub tx: Sender<FetchResult>,
    pub rx: Receiver<FetchResult>,
}

impl App {
    pub fn new(
        feeds: Vec<Feed>,
        conn: Connection,
        tx: Sender<FetchResult>,
        rx: Receiver<FetchResult>,
    ) -> Self {
        let category_colors = load_category_colors(&conn)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(name, c)| Color::from_str(&c).ok().map(|color| (name, color)))
            .collect();

        Self {
            feeds,
            categories_state: ListState::default().with_selected(Some(0)),
            feeds_state: ListState::default().with_selected(Some(0)),
            articles_state: ListState::default(),
            focus: HomeFocus::Categories,
            view: View::Home,
            scroll: 0,
            input: None,
            toast: None,
            pending: 0,
            last_refresh: Instant::now(),
            category_colors,
            color_picker: None,
            conn,
            tx,
            rx,
        }
    }

    /// The assigned color for a category, or a neutral default.
    pub fn category_color(&self, name: &str) -> Color {
        self.category_colors
            .get(name)
            .copied()
            .unwrap_or(Color::White)
    }

    /// Open the color picker for the category highlighted on the left pane.
    pub fn open_color_picker(&mut self) {
        let cats = self.categories();
        let Some(name) = self.categories_state.selected().and_then(|i| cats.get(i)) else {
            return;
        };
        if name == "All" {
            self.set_toast("Can't color the All view".to_string());
            return;
        }
        self.color_picker = Some(ColorPicker {
            category: name.clone(),
            state: ListState::default().with_selected(Some(0)),
        });
    }

    pub fn picker_prev(&mut self) {
        if let Some(p) = &mut self.color_picker {
            p.state.select_previous();
        }
    }

    pub fn picker_next(&mut self) {
        if let Some(p) = &mut self.color_picker {
            // last selectable row is the "custom hex" entry at index PALETTE.len()
            let next = p
                .state
                .selected()
                .unwrap_or(0)
                .saturating_add(1)
                .min(PALETTE.len());
            p.state.select(Some(next));
        }
    }

    pub fn picker_cancel(&mut self) {
        self.color_picker = None;
    }

    /// Apply the highlighted palette color, or fall back to a hex text prompt.
    pub fn picker_confirm(&mut self) {
        let Some(p) = self.color_picker.take() else {
            return;
        };
        match PALETTE.get(p.state.selected().unwrap_or(0)) {
            Some((name, color)) => {
                self.category_colors.insert(p.category.clone(), *color);
                let _ = set_category_color(&self.conn, &p.category, name);
            }
            // the extra "custom hex" row: open the typed prompt instead
            None => self.input = Some((InputKind::SetColor(p.category), String::new())),
        }
    }

    pub fn set_toast(&mut self, message: String) {
        self.toast = Some(Toast {
            message,
            expires_at: Instant::now() + Duration::from_secs(4),
        });
    }

    /// Time left before the toast should disappear, if one is showing.
    pub fn toast_remaining(&self) -> Option<Duration> {
        self.toast
            .as_ref()
            .map(|t| t.expires_at.saturating_duration_since(Instant::now()))
    }

    /// How long the loop may sleep: short while fetches are in flight or a toast
    /// is counting down, otherwise block until input.
    pub fn poll_timeout(&self) -> Duration {
        let mut timeout = REFRESH_INTERVAL.saturating_sub(self.last_refresh.elapsed());
        if self.pending > 0 {
            timeout = timeout.min(Duration::from_millis(100));
        }
        if let Some(remaining) = self.toast_remaining() {
            timeout = timeout.min(remaining.max(Duration::from_millis(1)));
        }
        timeout
    }

    /// Mark every article in the selected feed as read.
    pub fn mark_feed_read(&mut self) {
        let Some(fi) = self.current_feed_idx() else {
            return;
        };
        let feed = &mut self.feeds[fi];
        for article in &mut feed.articles {
            article.read = true;
        }
        let _ = mark_all_read(&self.conn, &feed.url);
    }

    /// Distinct categories ("All" first), each shown on the Categories pane.
    pub fn categories(&self) -> Vec<String> {
        let mut cats: Vec<String> = Vec::new();
        for feed in &self.feeds {
            if !cats.contains(&feed.category) {
                cats.push(feed.category.clone());
            }
        }
        cats.sort();
        cats.insert(0, "All".to_string());
        cats
    }

    /// The highlighted category on the left pane (None = "All").
    pub fn active_category(&self) -> Option<String> {
        let sel = self.categories_state.selected()?;
        let cats = self.categories();
        match cats.get(sel) {
            Some(c) if c != "All" => Some(c.clone()),
            _ => None,
        }
    }

    /// Indices into `self.feeds` matching the active category filter, in order.
    pub fn visible_feed_indices(&self) -> Vec<usize> {
        let active = self.active_category();
        self.feeds
            .iter()
            .enumerate()
            .filter(|(_, f)| match &active {
                None => true,
                Some(c) => &f.category == c,
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// The real `self.feeds` index of the highlighted feed (maps through the filter).
    pub fn current_feed_idx(&self) -> Option<usize> {
        let sel = self.feeds_state.selected()?;
        self.visible_feed_indices().get(sel).copied()
    }

    pub fn spawn_fetch(&mut self, url: String, category: Option<String>) {
        self.pending += 1;
        spawn_fetch(self.tx.clone(), url, category);
    }

    /// Apply a background fetch result: cache new articles, refresh the feed in place.
    pub fn apply_fetch(&mut self, msg: FetchResult) {
        self.pending = self.pending.saturating_sub(1);
        let feed = match msg.outcome {
            Ok(feed) => feed,
            Err(e) => {
                self.set_toast(format!("Fetch failed: {e}"));
                return;
            }
        };

        let _ = cache_articles(&self.conn, &feed.url, &feed.articles);
        let articles = load_articles(&self.conn, &feed.url).unwrap_or_default();
        let _ = save_feed(&self.conn, &feed.url, &feed.title);
        if let Some(category) = &msg.category {
            let _ = set_feed_category(&self.conn, &feed.url, category);
        }

        if let Some(existing) = self.feeds.iter_mut().find(|f| f.url == feed.url) {
            existing.title = feed.title;
            existing.articles = articles;
            if let Some(category) = msg.category {
                existing.category = category;
            }
        } else {
            self.feeds.push(Feed {
                url: feed.url,
                title: feed.title,
                category: msg.category.unwrap_or_else(|| DEFAULT_CATEGORY.to_string()),
                articles,
            });
        }
    }

    /// Re-fetch every subscribed feed in the background (keeping categories).
    pub fn refresh_all(&mut self) {
        self.last_refresh = Instant::now();
        let urls: Vec<String> = self.feeds.iter().map(|f| f.url.clone()).collect();
        for url in urls {
            self.spawn_fetch(url, None);
        }
    }

    pub fn current_feed(&self) -> Option<&Feed> {
        self.current_feed_idx().map(|i| &self.feeds[i])
    }

    pub fn current_article(&self) -> Option<&crate::model::Article> {
        self.current_feed().and_then(|f| {
            self.articles_state
                .selected()
                .and_then(|i| f.articles.get(i))
        })
    }

    pub fn enter(&mut self) {
        match self.view {
            // On Home: from the category pane, jump focus to the feeds pane;
            // from the feeds pane, open the selected feed's articles.
            View::Home => match self.focus {
                HomeFocus::Categories => self.focus_feeds(),
                HomeFocus::Feeds => {
                    let has_articles = self.current_feed().is_some_and(|f| !f.articles.is_empty());
                    if has_articles {
                        self.articles_state.select(Some(0));
                        self.view = View::Articles;
                    }
                }
            },
            View::Articles => {
                if self.current_article().is_some() {
                    self.mark_current_read();
                    self.scroll = 0;
                    self.view = View::Reader;
                }
            }
            View::Reader => {}
        }
    }

    /// Move Home focus to the feeds pane, selecting the first feed.
    fn focus_feeds(&mut self) {
        self.focus = HomeFocus::Feeds;
        let has_feeds = !self.visible_feed_indices().is_empty();
        self.feeds_state.select(has_feeds.then_some(0));
    }

    fn mark_current_read(&mut self) {
        let (Some(fi), Some(ai)) = (self.current_feed_idx(), self.articles_state.selected()) else {
            return;
        };
        let feed = &mut self.feeds[fi];
        let Some(article) = feed.articles.get_mut(ai) else {
            return;
        };
        if article.read {
            return;
        }
        article.read = true;
        let _ = mark_read(&self.conn, &feed.url, &article.id);
    }

    pub fn back(&mut self) {
        match self.view {
            View::Reader => self.view = View::Articles,
            View::Articles => self.view = View::Home, // returns to the feeds pane
            // On Home, Esc steps from the feeds pane back to the category pane.
            View::Home => self.focus = HomeFocus::Categories,
        }
    }

    // Up/Down: move the active list/pane, or scroll the reader.
    pub fn select_previous(&mut self) {
        match self.view {
            View::Home => match self.focus {
                HomeFocus::Categories => {
                    self.categories_state.select_previous();
                    self.feeds_state.select(Some(0)); // category changed → reset feed list
                }
                HomeFocus::Feeds => self.feeds_state.select_previous(),
            },
            View::Articles => self.articles_state.select_previous(),
            View::Reader => self.scroll = self.scroll.saturating_sub(1),
        }
    }

    pub fn select_next(&mut self) {
        match self.view {
            View::Home => match self.focus {
                HomeFocus::Categories => {
                    self.categories_state.select_next();
                    self.feeds_state.select(Some(0));
                }
                HomeFocus::Feeds => self.feeds_state.select_next(),
            },
            View::Articles => self.articles_state.select_next(),
            View::Reader => self.scroll = self.scroll.saturating_add(1), // clamped in render
        }
    }

    /// Left arrow: focus categories on Home, else previous article in the reader.
    pub fn go_left(&mut self) {
        match self.view {
            View::Home => self.focus = HomeFocus::Categories,
            View::Reader => self.prev_article(),
            _ => {}
        }
    }

    /// Right arrow: focus feeds on Home, else next article in the reader.
    pub fn go_right(&mut self) {
        match self.view {
            View::Home => self.focus_feeds(),
            View::Reader => self.next_article(),
            _ => {}
        }
    }

    fn prev_article(&mut self) {
        if self.view == View::Reader {
            self.articles_state.select_previous();
            self.scroll = 0;
        }
    }

    fn next_article(&mut self) {
        if self.view == View::Reader {
            self.articles_state.select_next();
            self.scroll = 0;
        }
    }

    /// Open the selected article's link in the system browser.
    pub fn open_current(&mut self) {
        let Some(link) = self.current_article().map(|a| a.link.clone()) else {
            return;
        };
        if link.is_empty() {
            self.set_toast("No link for this article".to_string());
            return;
        }
        if let Err(e) = open::that(&link) {
            self.set_toast(format!("Couldn't open browser: {e}"));
        }
    }

    pub fn submit_input(&mut self) {
        let Some((kind, buffer)) = self.input.take() else {
            return;
        };
        let text = buffer.trim().to_string();
        match kind {
            // After the URL, ask for the category (even if the URL was blank: bail).
            InputKind::AddFeedUrl => {
                if !text.is_empty() {
                    self.input = Some((InputKind::AddFeedCategory(text), String::new()));
                }
            }
            // URL collected, category now in `text` (blank → Uncategorized). Fetch it.
            InputKind::AddFeedCategory(url) => {
                let category = if text.is_empty() {
                    DEFAULT_CATEGORY.to_string()
                } else {
                    text
                };
                self.spawn_fetch(url, Some(category));
            }
            InputKind::SetCategory(url) => {
                let category = if text.is_empty() {
                    DEFAULT_CATEGORY.to_string()
                } else {
                    text
                };
                if let Some(feed) = self.feeds.iter_mut().find(|f| f.url == url) {
                    feed.category = category.clone();
                }
                let _ = set_feed_category(&self.conn, &url, &category);
            }
            InputKind::SetColor(name) => match Color::from_str(&text) {
                Ok(color) => {
                    self.category_colors.insert(name.clone(), color);
                    let _ = set_category_color(&self.conn, &name, &text);
                }
                Err(_) => self.set_toast(format!("Invalid color: {text}")),
            },
            InputKind::ImportOpml => {
                if !text.is_empty() {
                    self.import_opml(&text);
                }
            }
        }
    }

    /// Open the category prompt for the highlighted feed.
    pub fn prompt_category(&mut self) {
        if let Some(feed) = self.current_feed() {
            self.input = Some((InputKind::SetCategory(feed.url.clone()), String::new()));
        }
    }

    fn import_opml(&mut self, path: &str) {
        let xml = match std::fs::read_to_string(path) {
            Ok(xml) => xml,
            Err(e) => return self.set_toast(format!("Can't read {path}: {e}")),
        };
        let doc = match OPML::from_str(&xml) {
            Ok(doc) => doc,
            Err(e) => return self.set_toast(format!("Invalid OPML: {e}")),
        };

        let mut feeds = Vec::new();
        collect_feeds(&doc.body.outlines, None, &mut feeds);

        let mut added = 0;
        for (url, category) in feeds {
            if !self.feeds.iter().any(|f| f.url == url) {
                self.spawn_fetch(
                    url,
                    Some(category.unwrap_or_else(|| DEFAULT_CATEGORY.to_string())),
                );
                added += 1;
            }
        }
        self.set_toast(match added {
            0 => "No new feeds in OPML".to_string(),
            n => format!("Importing {n} feed(s)…"),
        });
    }

    pub fn export_opml(&mut self) {
        // group feeds under one OPML outline per category
        let mut doc = OPML::default();
        for category in self.categories().into_iter().skip(1) {
            // skip "All"
            let mut group = opml::Outline {
                text: category.clone(),
                ..Default::default()
            };
            for feed in self.feeds.iter().filter(|f| f.category == category) {
                group.add_feed(&feed.title, &feed.url);
            }
            doc.body.outlines.push(group);
        }
        let path = data_path("feeds.opml");
        let result = doc
            .to_string()
            .map_err(|e| e.to_string())
            .and_then(|xml| std::fs::write(&path, xml).map_err(|e| e.to_string()));
        self.set_toast(match result {
            Ok(()) => format!("Exported to {}", path.display()),
            Err(e) => format!("Export failed: {e}"),
        });
    }

    pub fn delete_current_feed(&mut self) {
        if self.view != View::Home {
            return;
        }
        let Some(i) = self.current_feed_idx() else {
            return;
        };

        let url = self.feeds[i].url.clone();
        if let Err(e) = delete_feed(&self.conn, &url) {
            self.set_toast(format!("Failed to delete: {e}"));
            return;
        }
        self.feeds.remove(i);

        // re-clamp selection within the (possibly smaller) filtered list
        let visible = self.visible_feed_indices().len();
        let selected = self.feeds_state.selected();
        self.feeds_state.select(match (visible, selected) {
            (0, _) => None,
            (n, Some(s)) => Some(s.min(n - 1)),
            (_, None) => Some(0),
        });
    }
}
