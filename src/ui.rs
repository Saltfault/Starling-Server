use crate::event::ChatMessage;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

#[derive(Default)]
pub struct App {
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub peers: usize,
    pub invite: Option<String>,
}

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header with invite ticket
        Constraint::Min(1),    // messages
        Constraint::Length(3), // input
    ])
    .split(f.area());

    let invite = app.invite.as_deref().unwrap_or("waiting for endpoint...");
    f.render_widget(
        Paragraph::new(format!(" invite: {} ", invite)).style(Style::new().fg(Color::DarkGray)),
        chunks[0],
    );

    let items: Vec<ListItem> = app
        .messages
        .iter()
        .map(|m| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{}: ", m.author),
                    Style::new().fg(Color::Rgb(244, 138, 82)).bold(),
                ),
                Span::raw(&m.body),
            ]))
        })
        .collect();

    f.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" #global · {} peers ", app.peers)),
        ),
        chunks[1],
    );

    f.render_widget(
        Paragraph::new(app.input.as_str())
            .block(Block::default().borders(Borders::ALL).title(" message ")),
        chunks[2],
    );
}
