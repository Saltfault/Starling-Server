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

/// All mutable UI state. Updated by `main.rs` in response to keyboard input
/// and network events; read by [`draw`] every frame.
#[derive(Default)]
pub struct App {
    /// Chat messages received (and echoed from our own broadcasts).
    pub messages: Vec<ChatMessage>,
    /// Current text input buffer.
    pub input: String,
    /// Connected peer IDs (from gossip neighbor-up/down events).
    pub peers: Vec<EndpointId>,
    /// Index into `peers` for the currently selected peer (for calling).
    pub selected_peer: usize,
    /// Shareable invite ticket string (shown in the header).
    pub invite: Option<String>,
    /// Whether we are currently in a call.
    pub in_call: bool,
    /// Whether the mic is muted (display state; the actual gate is an
    /// `Arc<AtomicBool>` in `main.rs`).
    pub muted: bool,
}

impl App {
    /// Number of connected peers.
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Cycle the selected peer to the next one in the list (wraps around).
    /// Does nothing if no peers are connected.
    pub fn select_next_peer(&mut self) {
        if !self.peers.is_empty() {
            self.selected_peer = (self.selected_peer + 1) % self.peers.len();
        }
    }

    /// Return the [`EndpointAddr`] of the currently selected peer, if any.
    ///
    /// The `EndpointAddr` is constructed from just the `EndpointId`; iroh's
    /// discovery system resolves the actual address when we connect.
    pub fn selected_peer_addr(&self) -> Option<EndpointAddr> {
        self.peers
            .get(self.selected_peer)
            .map(|id| EndpointAddr::from(*id))
    }

    /// Short label for the currently selected peer (first 8 hex chars of the
    /// node ID), for display in the status line.
    pub fn selected_peer_label(&self) -> String {
        match self.peers.get(self.selected_peer) {
            Some(id) => id.fmt_short().to_string(),
            None => "none".into(),
        }
    }
}

/// Render the app state to the terminal.
///
/// Layout (top to bottom):
/// 1. Header — invite ticket string
/// 2. Messages — scrollable chat log
/// 3. Status — call state + selected peer
/// 4. Input — text entry
pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header: invite ticket
        Constraint::Min(1),    // messages
        Constraint::Length(1), // call status
        Constraint::Length(3), // input
    ])
    .split(f.area());

    // ── Header: invite ticket ──────────────────────────────────────────
    let invite = app.invite.as_deref().unwrap_or("waiting for endpoint...");
    f.render_widget(
        Paragraph::new(format!(" invite: {} ", invite)).style(Style::new().fg(Color::DarkGray)),
        chunks[0],
    );

    // ── Messages ───────────────────────────────────────────────────────
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
                .title(format!(" #global · {} peers ", app.peer_count())),
        ),
        chunks[1],
    );

    // ── Status: call state + selected peer ─────────────────────────────
    let status = if app.in_call {
        format!("🔊 in call · {}", if app.muted { "muted" } else { "live" })
    } else {
        format!(
            "○ idle · peer: {} · Ctrl+K to call · Tab to cycle",
            app.selected_peer_label()
        )
    };
    f.render_widget(
        Paragraph::new(status).style(Style::new().fg(Color::Rgb(111, 174, 157))),
        chunks[2],
    );

    // ── Input ──────────────────────────────────────────────────────────
    f.render_widget(
        Paragraph::new(app.input.as_str())
            .block(Block::default().borders(Borders::ALL).title(" message ")),
        chunks[3],
    );
}
