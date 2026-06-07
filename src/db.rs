//! SQLite persistence: feeds, cached articles, and read state.

use crate::model::{Article, Result};
use rusqlite::Connection;
use std::path::PathBuf;

/// Path to a file in the scrolls data directory.
pub fn data_path(file: &str) -> PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("scrolls");
    path.push(file);
    path
}

/// Open (creating if needed) the scrolls database and ensure the schema exists.
pub fn open_db() -> Result<Connection> {
    let path = data_path("scrolls.db");
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }

    let conn = Connection::open(path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS feeds (
            url      TEXT PRIMARY KEY,
            title    TEXT NOT NULL,
            category TEXT NOT NULL DEFAULT 'Uncategorized'
        )",
        [],
    )?;
    // migrate older DBs that predate the category column (errors if it already exists)
    let _ = conn.execute(
        "ALTER TABLE feeds ADD COLUMN category TEXT NOT NULL DEFAULT 'Uncategorized'",
        [],
    );
    conn.execute(
        "CREATE TABLE IF NOT EXISTS category_colors (
            name  TEXT PRIMARY KEY,
            color TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS articles (
            feed_url  TEXT NOT NULL,
            id        TEXT NOT NULL,
            title     TEXT NOT NULL,
            body_html TEXT NOT NULL,
            link      TEXT NOT NULL,
            published INTEGER NOT NULL,
            read      INTEGER NOT NULL DEFAULT 0,
            saved     INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (feed_url, id)
        )",
        [],
    )?;
    // migrate older DBs that predate the saved column
    let _ = conn.execute(
        "ALTER TABLE articles ADD COLUMN saved INTEGER NOT NULL DEFAULT 0",
        [],
    );
    Ok(conn)
}

pub fn load_feeds(conn: &Connection) -> Result<Vec<(String, String, String)>> {
    let mut stmt = conn.prepare("SELECT url, title, category FROM feeds ORDER BY rowid")?;
    let feeds = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(feeds)
}

/// Insert a feed or refresh its title, leaving any existing category untouched.
pub fn save_feed(conn: &Connection, url: &str, title: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO feeds (url, title) VALUES (?1, ?2)
         ON CONFLICT(url) DO UPDATE SET title = excluded.title",
        (url, title),
    )?;
    Ok(())
}

pub fn set_feed_category(conn: &Connection, url: &str, category: &str) -> Result<()> {
    conn.execute(
        "UPDATE feeds SET category = ?2 WHERE url = ?1",
        (url, category),
    )?;
    Ok(())
}

pub fn delete_feed(conn: &Connection, url: &str) -> Result<()> {
    conn.execute("DELETE FROM articles WHERE feed_url = ?1", [url])?;
    conn.execute("DELETE FROM feeds WHERE url = ?1", [url])?;
    Ok(())
}

pub fn load_category_colors(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT name, color FROM category_colors")?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn set_category_color(conn: &Connection, name: &str, color: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO category_colors (name, color) VALUES (?1, ?2)
         ON CONFLICT(name) DO UPDATE SET color = excluded.color",
        (name, color),
    )?;
    Ok(())
}

/// Insert any new articles, leaving existing rows (and their read state) untouched.
pub fn cache_articles(conn: &Connection, feed_url: &str, articles: &[Article]) -> Result<()> {
    for a in articles {
        conn.execute(
            "INSERT OR IGNORE INTO articles
                (feed_url, id, title, body_html, link, published)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                feed_url,
                &a.id,
                &a.title,
                &a.body_html,
                &a.link,
                a.published,
            ),
        )?;
    }
    Ok(())
}

pub fn load_articles(conn: &Connection, feed_url: &str) -> Result<Vec<Article>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, body_html, link, read, saved, published
         FROM articles WHERE feed_url = ?1
         ORDER BY published DESC, rowid ASC",
    )?;
    let articles = stmt
        .query_map([feed_url], |row| {
            Ok(Article {
                id: row.get(0)?,
                title: row.get(1)?,
                body_html: row.get(2)?,
                link: row.get(3)?,
                read: row.get::<_, i64>(4)? != 0,
                saved: row.get::<_, i64>(5)? != 0,
                published: row.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(articles)
}

pub fn set_read(conn: &Connection, feed_url: &str, id: &str, read: bool) -> Result<()> {
    conn.execute(
        "UPDATE articles SET read = ?3 WHERE feed_url = ?1 AND id = ?2",
        (feed_url, id, read as i64),
    )?;
    Ok(())
}

pub fn set_saved(conn: &Connection, feed_url: &str, id: &str, saved: bool) -> Result<()> {
    conn.execute(
        "UPDATE articles SET saved = ?3 WHERE feed_url = ?1 AND id = ?2",
        (feed_url, id, saved as i64),
    )?;
    Ok(())
}

pub fn set_body(conn: &Connection, feed_url: &str, id: &str, body: &str) -> Result<()> {
    conn.execute(
        "UPDATE articles SET body_html = ?3 WHERE feed_url = ?1 AND id = ?2",
        (feed_url, id, body),
    )?;
    Ok(())
}

pub fn mark_read(conn: &Connection, feed_url: &str, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE articles SET read = 1 WHERE feed_url = ?1 AND id = ?2",
        (feed_url, id),
    )?;
    Ok(())
}

pub fn mark_all_read(conn: &Connection, feed_url: &str) -> Result<()> {
    conn.execute(
        "UPDATE articles SET read = 1 WHERE feed_url = ?1",
        [feed_url],
    )?;
    Ok(())
}
