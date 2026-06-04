//! Fetching/parsing feeds over the network and OPML helpers.

use crate::model::{Article, DEFAULT_CATEGORY, Feed, FetchResult, Result};
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

pub fn fetch_feed(url: &str) -> Result<Feed> {
    let bytes = ureq::get(url).call()?.body_mut().read_to_vec()?;
    let feed = feed_rs::parser::parse(bytes.as_slice())?;

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
                body_html: entry
                    .summary
                    .map(|t| t.content)
                    .or_else(|| entry.content.and_then(|c| c.body))
                    .unwrap_or_default(),
                link,
                read: false,
                published,
            }
        })
        .collect();

    Ok(Feed {
        url: url.to_string(),
        title,
        category: DEFAULT_CATEGORY.to_string(), // real category is applied in apply_fetch
        articles,
    })
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
