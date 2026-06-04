//! Core data types shared across the app.

use std::time::Instant;

/// Crate-wide error-boxing Result alias.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub const DEFAULT_CATEGORY: &str = "Uncategorized";

pub struct Article {
    pub id: String,
    pub title: String,
    pub body_html: String,
    pub link: String,
    pub read: bool,
    pub saved: bool,
    pub published: i64,
}

pub struct Feed {
    pub url: String,
    pub title: String,
    pub category: String,
    pub articles: Vec<Article>,
}

#[derive(PartialEq)]
pub enum View {
    Home,
    Articles,
    Reader,
}

/// Which pane of the two-pane Home screen has focus.
#[derive(PartialEq)]
pub enum HomeFocus {
    Categories,
    Feeds,
}

/// What the text-input prompt is collecting.
pub enum InputKind {
    AddFeedUrl,
    AddFeedCategory(String), // carries the URL just entered
    SetCategory(String),     // carries the target feed's URL
    SetColor(String),        // carries the category name to recolor
    ImportOpml,
}

/// A transient error message shown in the corner until it expires.
pub struct Toast {
    pub message: String,
    pub expires_at: Instant,
}

/// Result of a background fetch, sent from a worker thread back to the UI loop.
pub struct FetchResult {
    pub outcome: std::result::Result<Feed, String>,
    /// Category to assign (Some for a fresh add/import, None to keep existing).
    pub category: Option<String>,
}
