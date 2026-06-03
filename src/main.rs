use ratatui::{
    crossterm::event::{self, Event, KeyCode},
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, List, ListState, Paragraph, Wrap},
};

struct Article {
    title: String,
    summary: String,
    link: String,
}

struct App {
    articles: Vec<Article>,
    list_state: ListState,
}

impl App {
    fn new(articles: Vec<Article>) -> Self {
        let selected = if articles.is_empty() { None } else { Some(0) };

        Self {
            articles,
            list_state: ListState::default().with_selected(selected),
        }
    }
}

fn fetch_articles(url: &str) -> Result<Vec<Article>, Box<dyn std::error::Error>> {
    let bytes = ureq::get(url).call()?.body_mut().read_to_vec()?;
    let feed = feed_rs::parser::parse(bytes.as_slice())?;

    let articles = feed
        .entries
        .into_iter()
        .map(|entry| Article {
            title: entry
                .title
                .map(|t| t.content)
                .unwrap_or_else(|| "(untitled)".to_string()),
            summary: {
                let html = entry
                    .summary
                    .map(|t| t.content)
                    .or_else(|| entry.content.and_then(|c| c.body))
                    .unwrap_or_default();
                html2text::from_read(html.as_bytes(), 80).unwrap_or(html)
            },
            link: entry
                .links
                .into_iter()
                .next()
                .map(|l| l.href)
                .unwrap_or_default(),
        })
        .collect();

    Ok(articles)
}

const FEED_URL: &str = "https://blog.rust-lang.org/feed.xml";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let articles = fetch_articles(FEED_URL)?;

    let mut terminal = ratatui::init();
    let mut app = App::new(articles);

    loop {
        terminal.draw(|frame| {
            let [feeds_area, articles_area, reader_area] = Layout::horizontal([
                Constraint::Percentage(20),
                Constraint::Percentage(30),
                Constraint::Percentage(50),
            ])
            .areas(frame.area());

            let feeds_block = Block::default()
                .borders(Borders::ALL)
                .title(" Feeds ")
                .border_type(BorderType::Rounded);

            let articles_block = Block::default()
                .borders(Borders::ALL)
                .title(" Articles ")
                .border_type(BorderType::Rounded);

            let reader_block = Block::default()
                .borders(Borders::ALL)
                .title(" Reader ")
                .border_type(BorderType::Rounded);

            let list = List::new(app.articles.iter().map(|a| a.title.clone()))
                .block(articles_block)
                .highlight_style(
                    Style::default()
                        .bg(Color::LightBlue)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            let selected_summary = app
                .list_state
                .selected()
                .and_then(|i| app.articles.get(i))
                .map(|a| a.summary.clone())
                .unwrap_or_default();

            let reader = Paragraph::new(selected_summary)
                .block(reader_block)
                .wrap(Wrap { trim: true });

            frame.render_widget(feeds_block, feeds_area);
            frame.render_stateful_widget(list, articles_area, &mut app.list_state);
            frame.render_widget(reader, reader_area);
        })?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Up => app.list_state.select_previous(),
                KeyCode::Down => app.list_state.select_next(),
                _ => {}
            }
        }
    }

    ratatui::restore();
    Ok(())
}
