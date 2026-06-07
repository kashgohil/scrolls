mod app;
mod db;
mod feed;
mod model;
mod ui;

use app::{App, REFRESH_INTERVAL};
use db::{load_articles, load_feeds, open_db};
use feed::DEFAULT_FEEDS;
use model::{Feed, InputKind, Result, View};
use ratatui::crossterm::event::{self, Event, KeyCode};
use std::time::Duration;

fn main() -> Result<()> {
    let conn = open_db()?;

    // Show whatever's cached immediately; the network refresh runs in the background.
    let stored = load_feeds(&conn)?;
    let mut feeds = Vec::new();
    for (url, title, category) in &stored {
        let articles = load_articles(&conn, url)?;
        feeds.push(Feed {
            url: url.clone(),
            title: title.clone(),
            category: category.clone(),
            articles,
        });
    }

    // Progressive: only enable images if the terminal speaks a real graphics
    // protocol (Kitty/iTerm2/Sixel). Halfblocks/failure => no images at all.
    let picker = ratatui_image::picker::Picker::from_query_stdio()
        .ok()
        .filter(|p| p.protocol_type() != ratatui_image::picker::ProtocolType::Halfblocks);

    let (tx, rx) = std::sync::mpsc::channel();
    let mut terminal = ratatui::init();
    let mut app = App::new(feeds, conn, tx, rx, picker);

    if stored.is_empty() {
        // first run: seed the curated, pre-categorized defaults
        for (url, category) in DEFAULT_FEEDS {
            app.spawn_fetch(url.to_string(), Some(category.to_string()));
        }
    } else {
        for (url, _, _) in &stored {
            app.spawn_fetch(url.clone(), None);
        }
    }

    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    let mut dirty = true;
    loop {
        // apply any feeds that finished fetching in the background
        while let Ok(msg) = app.rx.try_recv() {
            app.apply_fetch(msg);
            dirty = true;
        }
        // apply any on-demand full-article fetches
        while let Ok(msg) = app.content_rx.try_recv() {
            app.apply_content(msg);
            dirty = true;
        }
        // apply any decoded inline images
        while let Ok(msg) = app.img_rx.try_recv() {
            app.apply_image(msg);
            dirty = true;
        }
        // expire the toast once its time is up
        if app.toast_remaining() == Some(Duration::ZERO) {
            app.toast = None;
            dirty = true;
        }
        // periodic background refresh (refresh_all resets the timer)
        if app.last_refresh.elapsed() >= REFRESH_INTERVAL {
            app.refresh_all();
        }

        if dirty {
            terminal.draw(|frame| ui::render(frame, app))?;
            dirty = false;
        }

        if event::poll(app.poll_timeout())? {
            match event::read()? {
                Event::Key(key) => {
                    dirty = true;
                    if app.color_picker.is_some() {
                        match key.code {
                            KeyCode::Up => app.picker_prev(),
                            KeyCode::Down => app.picker_next(),
                            KeyCode::Enter => app.picker_confirm(),
                            KeyCode::Esc => app.picker_cancel(),
                            _ => {}
                        }
                    } else if app.input.is_some() {
                        match key.code {
                            KeyCode::Char(c) => app.input.as_mut().unwrap().1.push(c),
                            KeyCode::Backspace => {
                                app.input.as_mut().unwrap().1.pop();
                            }
                            KeyCode::Enter => app.submit_input(),
                            KeyCode::Esc => app.input = None,
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Char('a') if app.view == View::Home => {
                                app.input = Some((InputKind::AddFeedUrl, String::new()));
                            }
                            KeyCode::Char('c') if app.view == View::Home => app.prompt_category(),
                            KeyCode::Char('p') if app.view == View::Home => app.open_color_picker(),
                            KeyCode::Char('i') if app.view == View::Home => {
                                app.input = Some((InputKind::ImportOpml, String::new()));
                            }
                            KeyCode::Char('e') if app.view == View::Home => app.export_opml(),
                            KeyCode::Char('S') if app.view == View::Home => app.open_saved(),
                            KeyCode::Char('d') => app.delete_current_feed(),
                            KeyCode::Char('r') => app.refresh_all(),
                            KeyCode::Char('A') => app.mark_feed_read(),
                            KeyCode::Char('/') if app.view == View::Articles => app.start_search(),
                            KeyCode::Char('F') if app.view == View::Articles => {
                                app.cycle_article_filter()
                            }
                            KeyCode::Char('s') => app.toggle_current_saved(),
                            KeyCode::Char('t') => app.toggle_current_read(),
                            KeyCode::Char('f') => app.fetch_full_content(),
                            KeyCode::Char('o') => app.open_current(),
                            KeyCode::Up => app.select_previous(),
                            KeyCode::Down => app.select_next(),
                            KeyCode::Left => app.go_left(),
                            KeyCode::Right => app.go_right(),
                            KeyCode::Enter => app.enter(),
                            KeyCode::Esc => app.back(),
                            _ => {}
                        }
                    }
                }
                Event::Resize(_, _) => dirty = true,
                _ => {}
            }
        }
    }
    Ok(())
}
