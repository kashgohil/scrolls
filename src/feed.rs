//! Fetching/parsing feeds over the network and OPML helpers.

use crate::model::{Article, ContentResult, DEFAULT_CATEGORY, Feed, FetchResult, Result};
use dom_smoothie::Readability;
use std::sync::mpsc::Sender;

/// Curated (feed URL, category) pairs seeded on first run.
pub const DEFAULT_FEEDS: &[(&str, &str)] = &[
    ("https://blog.rust-lang.org/feed.xml", "Programming"),
    ("https://github.blog/feed/", "Programming"),
    ("https://lobste.rs/rss", "Programming"),
    ("https://hnrss.org/frontpage", "Tech"),
    ("https://feeds.arstechnica.com/arstechnica/index", "Tech"),
    ("https://daringfireball.net/feeds/main", "Tech"),
    ("https://feeds.bbci.co.uk/news/world/rss.xml", "News"),
    ("https://www.theverge.com/rss/index.xml", "News"),
    ("https://www.sciencedaily.com/rss/all.xml", "Science"),
    ("https://css-tricks.com/feed/", "Design"),
    ("https://www.smashingmagazine.com/feed/", "Design"),
    ("https://xkcd.com/rss.xml", "Comics"),
];

/// Fetch + parse one feed on a background thread, sending the result down `tx`.
pub fn spawn_fetch(tx: Sender<FetchResult>, url: String, category: Option<String>) {
    std::thread::spawn(move || {
        let outcome = fetch_feed(&url).map_err(|e| e.to_string());
        let _ = tx.send(FetchResult { outcome, category });
    });
}

/// Fetch an article's page and extract its readable content on a background thread.
pub fn spawn_content(tx: Sender<ContentResult>, feed_url: String, id: String, link: String) {
    std::thread::spawn(move || {
        let body = fetch_readable(&link);
        let _ = tx.send(ContentResult { feed_url, id, body });
    });
}

/// Download `url` and extract the main article HTML via readability.
fn fetch_readable(url: &str) -> std::result::Result<String, String> {
    let bytes = ureq::get(url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    let html = String::from_utf8_lossy(&bytes);
    let article = Readability::new(html.as_ref(), Some(url), None)
        .map_err(|e| e.to_string())?
        .parse()
        .map_err(|e| e.to_string())?;
    Ok(article.content.to_string())
}

pub fn fetch_feed(url: &str) -> Result<Feed> {
    // allow bare hosts like "theverge.com"
    let normalized = if url.contains("://") {
        url.to_string()
    } else {
        format!("https://{url}")
    };
    let url = normalized.as_str();

    let bytes = ureq::get(url).call()?.body_mut().read_to_vec()?;

    // Direct feed?
    if let Ok(feed) = feed_rs::parser::parse(bytes.as_slice()) {
        return Ok(build_feed(url, feed));
    }

    // Otherwise treat it as a web page and look for a linked feed.
    let html = String::from_utf8_lossy(&bytes);
    let discovered = discover_feed_url(&html, url).ok_or("no RSS/Atom feed found at that URL")?;
    let bytes = ureq::get(&discovered).call()?.body_mut().read_to_vec()?;
    let feed = feed_rs::parser::parse(bytes.as_slice())?;
    Ok(build_feed(&discovered, feed))
}

/// Convert a parsed feed-rs feed into our model.
fn build_feed(url: &str, feed: feed_rs::model::Feed) -> Feed {
    let title = feed
        .title
        .map(|t| t.content)
        .unwrap_or_else(|| url.to_string());

    let articles = feed
        .entries
        .into_iter()
        .map(|entry| {
            let link = entry
                .links
                .into_iter()
                .next()
                .map(|l| l.href)
                .unwrap_or_default();
            // a feed entry's id can be empty; fall back to the link as a stable key
            let id = if entry.id.is_empty() {
                link.clone()
            } else {
                entry.id
            };
            let published = entry
                .published
                .or(entry.updated)
                .map(|d| d.timestamp())
                .unwrap_or(0);

            Article {
                id,
                title: entry
                    .title
                    .map(|t| t.content)
                    .unwrap_or_else(|| "(untitled)".to_string()),
                // prefer full content (<content:encoded>/<content>) over the
                // shorter summary/description
                body_html: entry
                    .content
                    .and_then(|c| c.body)
                    .or_else(|| entry.summary.map(|t| t.content))
                    .unwrap_or_default(),
                link,
                read: false,
                saved: false,
                published,
            }
        })
        .collect();

    Feed {
        url: url.to_string(),
        title,
        category: DEFAULT_CATEGORY.to_string(), // real category is applied in apply_fetch
        articles,
    }
}

/// Find an RSS/Atom feed link in an HTML page's `<link rel=alternate>` tags.
fn discover_feed_url(html: &str, base: &str) -> Option<String> {
    let lower = html.to_lowercase();
    for ty in ["application/rss+xml", "application/atom+xml"] {
        let mut from = 0;
        while let Some(pos) = lower[from..].find(ty) {
            let abs = from + pos;
            let tag_start = lower[..abs].rfind('<')?;
            let tag_end = lower[abs..]
                .find('>')
                .map(|e| abs + e)
                .unwrap_or(html.len());
            if let Some(href) = extract_attr(&html[tag_start..=tag_end], "href") {
                return Some(resolve_url(&href, base));
            }
            from = tag_end;
        }
    }
    None
}

/// Pull a quoted (or bare) attribute value out of a single HTML tag.
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let i = tag.to_lowercase().find(attr)?;
    let after = tag[i + attr.len()..].trim_start_matches([' ', '=']);
    let mut chars = after.chars();
    match chars.next()? {
        q @ ('"' | '\'') => after[1..].find(q).map(|e| after[1..=e].to_string()),
        _ => {
            let end = after
                .find(|c: char| c.is_whitespace() || c == '>')
                .unwrap_or(after.len());
            Some(after[..end].to_string())
        }
    }
}

/// Resolve a possibly-relative href against the page URL (common cases only).
fn resolve_url(href: &str, base: &str) -> String {
    let (scheme, rest) = base.split_once("://").unwrap_or(("https", base));
    let host = rest.split('/').next().unwrap_or(rest);
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_string()
    } else if let Some(pr) = href.strip_prefix("//") {
        format!("{scheme}://{pr}")
    } else if href.starts_with('/') {
        format!("{scheme}://{host}{href}")
    } else {
        let dir = base.rsplit_once('/').map(|(d, _)| d).unwrap_or(base);
        format!("{dir}/{href}")
    }
}

/// Collect (feed URL, category) from nested OPML outlines; a parent outline's
/// text becomes the category of the feeds nested under it.
pub fn collect_feeds(
    outlines: &[opml::Outline],
    parent: Option<&str>,
    out: &mut Vec<(String, Option<String>)>,
) {
    for outline in outlines {
        if let Some(url) = &outline.xml_url {
            out.push((url.clone(), parent.map(|s| s.to_string())));
        }
        collect_feeds(&outline.outlines, Some(&outline.text), out);
    }
}
