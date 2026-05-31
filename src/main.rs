use ratatui::{
    crossterm::event::{self, Event, KeyCode},
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, List, ListState},
};

struct App {
    articles: Vec<String>,
    list_state: ListState,
}

impl App {
    fn new() -> Self {
        let articles = vec![
            "Welcome to Scrolls".to_string(),
            "Building a TUI with ratatui".to_string(),
            "Understanding the render loop".to_string(),
            "Ownership: move, borrow, clone".to_string(),
            "Stateful widgets and ListState".to_string(),
        ];

        Self {
            articles,
            list_state: ListState::default().with_selected(Some(0)),
        }
    }
}

fn main() -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::new();

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

            let list = List::new(app.articles.clone())
                .block(articles_block)
                .highlight_style(
                    Style::default()
                        .bg(Color::LightBlue)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            frame.render_widget(feeds_block, feeds_area);
            frame.render_stateful_widget(list, articles_area, &mut app.list_state);
            frame.render_widget(reader_block, reader_area);
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
