//! UI state and rendering. The [`App`] struct holds all mutable state that
//! the terminal loop reads and writes. The [`draw`] function renders it.
//!
//! The app has two phases:
//!
//! 1. **Name entry** — a centered popup asks for the bird's display name.
//!    Rendered when `app.name` is empty.
//! 2. **Chat** — the full chat UI with messages, a birds panel (peer list),
//!    call status, and text input. Rendered once `app.name` is set.

use crate::event::ChatMessage;
use iroh::{EndpointAddr, EndpointId};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

/// All mutable UI state.
#[derive(Default)]
pub struct App {
    // ── Name entry phase ──────────────────────────────────────────────
    pub name: String,
    pub name_input: String,

    // ── Chat phase ────────────────────────────────────────────────────
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub peers: Vec<EndpointId>,
    pub selected_peer: usize,
    /// Room code shown in the header (e.g. "BIRDA1B2C3"). Set when the
    /// endpoint binds (opener) or from the command line (joiner).
    pub invite: Option<String>,
    /// Full node ID — the invite ticket that other birds use to join.
    /// Shown in the header so the user can share it.
    pub ticket: Option<String>,
    pub in_call: bool,
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

// ── Draw dispatcher ─────────────────────────────────────────────────────

pub fn draw(f: &mut Frame, app: &App) {
    if app.name.is_empty() {
        draw_name_entry(f, app);
    } else {
        draw_chat(f, app);
    }
}

// ── Name entry ──────────────────────────────────────────────────────────

fn draw_name_entry(f: &mut Frame, app: &App) {
    let area = f.area();
    f.render_widget(Clear, area);

    let width = 48.min(area.width);
    let height = 9.min(area.height);
    let popup = Rect::new(
        area.x + (area.width.saturating_sub(width)) / 2,
        area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    );

    f.render_widget(Clear, popup);
    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(" Welcome to Starling "),
        popup,
    );

    let inner = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .margin(1)
    .split(popup);

    f.render_widget(Paragraph::new("Join the murmuration."), inner[1]);
    f.render_widget(
        Paragraph::new(format!(" Name: {}_", app.name_input)).style(Style::new().fg(Color::Yellow)),
        inner[3],
    );
    f.render_widget(
        Paragraph::new(" Press Enter to continue ").style(Style::new().fg(Color::DarkGray)),
        inner[5],
    );
}

// ── Chat UI ─────────────────────────────────────────────────────────────

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

fn draw_chat(f: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Min(1),    // messages + birds panel
        Constraint::Length(1), // call status
        Constraint::Length(3), // input
    ])
    .split(f.area());

    // ── Header: color swatch + room code + invite ticket ──────────────
    let room_code = app.invite.as_deref().unwrap_or("waiting for endpoint...");

    let mut header_spans: Vec<Span> = Vec::new();

    if let Some((r, g, b)) = room_code_rgb(room_code) {
        let full = Color::Rgb(r, g, b);
        let half = Color::Rgb(r / 2, g / 2, b / 2);
        header_spans.push(Span::styled("▀", Style::new().fg(full).bg(half)));
        header_spans.push(Span::styled("▄", Style::new().fg(full).bg(half)));
        header_spans.push(Span::raw(" "));
    }

    header_spans.push(Span::styled(
        format!(" {} ", room_code),
        Style::new().fg(Color::DarkGray),
    ));

    // Show the invite ticket (node ID) if available, truncated to fit.
    if let Some(ticket) = &app.ticket {
        let max_len = 16;
        let display = if ticket.len() > max_len {
            format!("{}...", &ticket[..max_len])
        } else {
            ticket.clone()
        };
        header_spans.push(Span::styled(
            format!(" invite: {} ", display),
            Style::new().fg(Color::DarkGray),
        ));
    }

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
