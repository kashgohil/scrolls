use ratatui::{
    crossterm::event::{self, Event, KeyCode},
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
            let block = Block::default()
                .borders(Borders::ALL)
                .title("Scrolls")
                .border_type(BorderType::Rounded);

            let list = List::new(app.articles.clone())
                .block(block)
                .highlight_style(
                    Style::default()
                        .bg(Color::LightBlue)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            frame.render_stateful_widget(list, frame.area(), &mut app.list_state);
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
