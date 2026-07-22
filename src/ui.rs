//! UI state and rendering.

use crate::event::ChatMessage;
use iroh::{EndpointAddr, EndpointId};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use std::collections::HashMap;

#[derive(Default)]
pub struct App {
    pub name: String,
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub peers: Vec<EndpointId>,
    pub selected_peer: usize,
    pub invite: Option<String>,
    pub node_id: Option<String>,
    pub show_invite: bool,
    pub in_call: bool,
    pub muted: bool,
    /// Maps peer EndpointId → display name (from profile announcements).
    pub peer_names: HashMap<EndpointId, String>,
}

impl App {
    pub fn bird_count(&self) -> usize {
        self.peers.len() + 1
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

    /// Get the display name for a peer, or fall back to the short node ID.
    pub fn peer_display_name(&self, id: &EndpointId) -> String {
        self.peer_names
            .get(id)
            .cloned()
            .unwrap_or_else(|| id.fmt_short().to_string())
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(3),
    ])
    .split(f.area());

    // ── Header: all color swatches + full code ────────────────────────
    let header = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(chunks[0]);

    let full_code = app.node_id.as_deref().unwrap_or("");
    let colors = parse_color_code(full_code);

    let mut swatch_spans: Vec<Span> = Vec::new();
    for (r, g, b) in &colors {
        let full = Color::Rgb(*r, *g, *b);
        let half = Color::Rgb(r / 2, g / 2, b / 2);
        swatch_spans.push(Span::styled("\u{2580}", Style::new().fg(full).bg(half)));
        swatch_spans.push(Span::styled("\u{2584}", Style::new().fg(full).bg(half)));
        swatch_spans.push(Span::raw(" "));
    }
    if swatch_spans.is_empty() {
        swatch_spans.push(Span::styled(
            " waiting for endpoint...",
            Style::new().fg(Color::DarkGray),
        ));
    }
    f.render_widget(Line::from(swatch_spans), header[0]);

    f.render_widget(
        Paragraph::new(format!(" {}", full_code)).style(Style::new().fg(Color::DarkGray)),
        header[1],
    );

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
                .title(format!(" #global . {} birds ", app.bird_count())),
        ),
        middle[0],
    );

    // Birds panel — local user first, then remote peers with names.
    let mut peer_items: Vec<ListItem> = Vec::new();
    peer_items.push(ListItem::new(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("{} (you)", app.name),
            Style::new().fg(Color::Yellow).bold(),
        ),
    ])));

    for (i, id) in app.peers.iter().enumerate() {
        let prefix = if i == app.selected_peer { "> " } else { "  " };
        let display = app.peer_display_name(id);
        peer_items.push(ListItem::new(format!("{prefix}{display}")));
    }

    f.render_widget(
        List::new(peer_items).block(Block::default().borders(Borders::ALL).title(" birds ")),
        middle[1],
    );

    // ── Status ────────────────────────────────────────────────────────
    let status = if app.in_call {
        format!(
            " in call . {} . Ctrl+K to hang up",
            if app.muted { "muted" } else { "live" }
        )
    } else {
        " idle . Ctrl+K to call . Tab to cycle . Ctrl+M to mute . i = invite".into()
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

    // ── Invite popup ──────────────────────────────────────────────────
    if app.show_invite {
        draw_invite_popup(f, app);
    }
}

fn draw_invite_popup(f: &mut Frame, app: &App) {
    let area = f.area();
    f.render_widget(Clear, area);

    let code = app.node_id.as_deref().unwrap_or("waiting for endpoint...");
    let colors = parse_color_code(code);

    let swatch_line_len = colors.len() * 3;
    let code_len = code.len();
    let content_width = swatch_line_len.max(code_len).max(40) + 4;
    let width = content_width.min(area.width as usize) as u16;
    let height = 12.min(area.height);

    let popup = Rect::new(
        area.x + (area.width.saturating_sub(width)) / 2,
        area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    );

    f.render_widget(Clear, popup);
    f.render_widget(
        Block::default().borders(Borders::ALL).title(" Invite "),
        popup,
    );

    let inner = popup.inner(Margin {
        vertical: 1,
        horizontal: 2,
    });

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    let mut swatch_spans: Vec<Span> = Vec::new();
    for (r, g, b) in &colors {
        let full = Color::Rgb(*r, *g, *b);
        let half = Color::Rgb(r / 2, g / 2, b / 2);
        swatch_spans.push(Span::styled("\u{2580}", Style::new().fg(full).bg(half)));
        swatch_spans.push(Span::styled("\u{2584}", Style::new().fg(full).bg(half)));
        swatch_spans.push(Span::raw(" "));
    }
    f.render_widget(Line::from(swatch_spans), chunks[1]);

    let mid = code.len() / 2;
    let (code1, code2) = if code.len() > 40 {
        let split = code[mid..].find('-').map(|i| mid + i).unwrap_or(mid);
        (&code[..split], &code[split..])
    } else {
        (code, "")
    };

    f.render_widget(
        Paragraph::new(code1).style(Style::new().fg(Color::Green)),
        chunks[3],
    );
    if !code2.is_empty() {
        f.render_widget(
            Paragraph::new(code2).style(Style::new().fg(Color::Green)),
            chunks[4],
        );
    }

    f.render_widget(Paragraph::new("They join with:"), chunks[6]);
    f.render_widget(
        Paragraph::new("  starling join <code>").style(Style::new().fg(Color::Yellow)),
        chunks[7],
    );
    f.render_widget(
        Paragraph::new("  Press i or Esc to close").style(Style::new().fg(Color::DarkGray)),
        chunks[9],
    );
}

fn parse_color_code(code: &str) -> Vec<(u8, u8, u8)> {
    let mut colors = Vec::new();
    for group in code.split('-') {
        if group == "BIRD" || group.len() != 6 {
            continue;
        }
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&group[0..2], 16),
            u8::from_str_radix(&group[2..4], 16),
            u8::from_str_radix(&group[4..6], 16),
        ) {
            colors.push((r, g, b));
        }
    }
    colors
}
