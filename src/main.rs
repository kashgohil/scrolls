use ratatui::{
    crossterm,
    widgets::{Block, BorderType, Borders},
};

fn main() -> std::io::Result<()> {
    let mut terminal = ratatui::init();

    loop {
        terminal.draw(|frame| {
            let block = Block::default()
                .borders(Borders::ALL)
                .title("Scrolls")
                .border_type(BorderType::Rounded);

            frame.render_widget(block, frame.area());
        })?;

        if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
            if key.code == crossterm::event::KeyCode::Char('q') {
                break;
            }
        }
    }

    ratatui::restore();
    Ok(())
}
