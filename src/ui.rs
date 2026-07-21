use crate::event::ChatMessage;
use iroh::EndpointAddr;
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
    pub in_call: bool,
    pub muted: bool,
}

impl App {
    // TODO: track peer addresses from AppEvent::PeerConnected so we can
    // select one here. Returns None until peer selection is wired up.
    pub fn selected_peer_addr(&self) -> Option<EndpointAddr> {
        None
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header with invite ticket
        Constraint::Min(1),    // messages
        Constraint::Length(1), // call status
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

    let status = if app.in_call {
        format!("🔊 in call · {}", if app.muted { "muted" } else { "live" })
    } else {
        "○ not in a call · Ctrl+K to call selected peer".into()
    };
    f.render_widget(
        Paragraph::new(status).style(Style::new().fg(Color::Rgb(111, 174, 157))),
        chunks[2],
    );

    f.render_widget(
        Paragraph::new(app.input.as_str())
            .block(Block::default().borders(Borders::ALL).title(" message ")),
        chunks[3],
    );
}
