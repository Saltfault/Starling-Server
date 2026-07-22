//! UI state and rendering. The [`App`] struct holds all mutable state that
//! the terminal loop reads and writes. The [`draw`] function renders it.
//!
//! This module is purely presentational — it never touches the network or
//! audio directly. State changes happen in `main.rs` in response to keyboard
//! input or [`AppEvent`](crate::event::AppEvent)s.

use crate::event::ChatMessage;
use iroh::{EndpointAddr, EndpointId};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

/// All mutable UI state.
#[derive(Default)]
pub struct App {
    /// The bird's display name (from profile).
    pub name: String,
    /// Chat messages received (and echoed from our own broadcasts).
    pub messages: Vec<ChatMessage>,
    /// Current text input buffer.
    pub input: String,
    /// Connected peer IDs (from gossip neighbor-up/down events).
    pub peers: Vec<EndpointId>,
    /// Index into `peers` for the currently selected peer (for calling).
    pub selected_peer: usize,
    /// Room code shown in the header (e.g. "BIRD00CCFF").
    pub invite: Option<String>,
    /// Whether we are currently in a call.
    pub in_call: bool,
    /// Whether the mic is muted (display state).
    pub muted: bool,
}

impl App {
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    pub fn select_next_peer(&mut self) {
        if !self.peers.is_empty() {
            self.selected_peer = (self.selected_peer + 1) % self.peers.len();
        }
    }

    pub fn selected_peer_addr(&self) -> Option<EndpointAddr> {
        self.peers
            .get(self.selected_peer)
            .map(|id| EndpointAddr::from(*id))
    }
}

/// Render the chat UI.
pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Min(1),    // messages + birds panel
        Constraint::Length(1), // call status
        Constraint::Length(3), // input
    ])
    .split(f.area());

    // ── Header: color swatch + room code only ──────────────────────────
    let room_code = app.invite.as_deref().unwrap_or("waiting for endpoint...");

    let mut header_spans: Vec<Span> = Vec::new();

    if let Some((r, g, b)) = room_code_rgb(room_code) {
        let full = Color::Rgb(r, g, b);
        let half = Color::Rgb(r / 2, g / 2, b / 2);
        header_spans.push(Span::styled("\u{2580}", Style::new().fg(full).bg(half)));
        header_spans.push(Span::styled("\u{2584}", Style::new().fg(full).bg(half)));
        header_spans.push(Span::raw(" "));
    }

    header_spans.push(Span::styled(
        format!(" {} ", room_code),
        Style::new().fg(Color::DarkGray),
    ));

    f.render_widget(Line::from(header_spans), chunks[0]);

    // ── Messages + Birds panel ────────────────────────────────────────
    let middle = Layout::horizontal([Constraint::Min(1), Constraint::Length(24)]).split(chunks[1]);

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
                .title(format!(" #global . {} birds ", app.peer_count())),
        ),
        middle[0],
    );

    // Birds panel
    let peer_items: Vec<ListItem> = app
        .peers
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let prefix = if i == app.selected_peer { "> " } else { "  " };
            ListItem::new(format!("{prefix}{}", id.fmt_short()))
        })
        .collect();

    let peer_list = if peer_items.is_empty() {
        List::new(vec![ListItem::new("  no birds yet")])
            .block(Block::default().borders(Borders::ALL).title(" birds "))
    } else {
        List::new(peer_items).block(Block::default().borders(Borders::ALL).title(" birds "))
    };
    f.render_widget(peer_list, middle[1]);

    // ── Status ────────────────────────────────────────────────────────
    let status = if app.in_call {
        format!(
            " in call . {} . Ctrl+K to hang up",
            if app.muted { "muted" } else { "live" }
        )
    } else {
        " idle . Ctrl+K to call . Tab to cycle . Ctrl+M to mute".into()
    };
    f.render_widget(
        Paragraph::new(status).style(Style::new().fg(Color::Rgb(111, 174, 157))),
        chunks[2],
    );

    // ── Input ─────────────────────────────────────────────────────────
    f.render_widget(
        Paragraph::new(app.input.as_str())
            .block(Block::default().borders(Borders::ALL).title(" message ")),
        chunks[3],
    );
}

fn room_code_rgb(code: &str) -> Option<(u8, u8, u8)> {
    let hex = code.strip_prefix("BIRD")?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}
