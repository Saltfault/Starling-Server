//! Starling — a federated p2p communications platform where peers, known as
//! birds, communicate via the murmuration.
//!
//! Architecture (one task + one UI loop):
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────┐
//! │ main.rs (UI loop)                                                │
//! │   keyboard → Command ──┐                                         │
//! │   AppEvent ←───────────┤──── mpsc channels ────┐                │
//! │   playback ← VoiceFrame│                       │                │
//! └────────────────────────┊────────────────────────┊───────────────┘
//!                          ▼                        ▼
//! ┌──────────────────────────────────────────────────────────────────┐
//! │ net.rs (network task)                                            │
//! │   gossip for chat · QUIC datagrams for voice                     │
//! │   mic capture (voice.rs) → place_call (call.rs)                  │
//! └──────────────────────────────────────────────────────────────────┘
//! ```
//!
//! The app starts in **name-entry mode**: a popup asks for the bird's display
//! name. Once confirmed, the network task is spawned and the chat UI begins.
//!
//! Keybindings (chat mode):
//!
//! | Key        | Action                          |
//! |------------|---------------------------------|
//! | `Enter`    | Send typed message              |
//! | `Ctrl+K`   | Start call / hang up            |
//! | `Ctrl+M`   | Toggle mute                     |
//! | `Tab`      | Cycle selected peer             |
//! | `Backspace`| Delete last character           |
//! | `Esc`      | Quit                            |

mod call;
mod event;
mod net;
mod playback;
mod ui;
mod voice;

use crossterm::{
    event::{self as ct_event, Event, KeyCode, KeyModifiers},
    execute,
    terminal::*,
};
use event::{AppEvent, Command};
use iroh_tickets::endpoint::EndpointTicket;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use ui::App;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Parse CLI args: `starling open` or `starling join <ticket>` ────
    let args: Vec<String> = std::env::args().collect();
    let (topic, bootstrap) = match args.get(1).map(String::as_str) {
        Some("join") => {
            // A ticket carries the topic-opener's address(es).
            let ticket: EndpointTicket = args[2].parse()?;
            // Register the opener's addr, then bootstrap from their ID.
            (
                net::topic_for("starling/global"),
                vec![ticket.endpoint_addr().id],
            )
        }
        _ => (net::topic_for("starling/global"), vec![]), // "open"
    };

    // ── Set up the terminal ───────────────────────────────────────────
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut term = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))?;
    let mut app = App::default();

    // ── Phase 1: Name entry ───────────────────────────────────────────
    //
    // Show a popup asking for the bird's display name. The network task
    // hasn't started yet — we need the name before spawning it.
    loop {
        term.draw(|f| ui::draw(f, &app))?;

        if ct_event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(k) = ct_event::read()? {
                match k.code {
                    KeyCode::Enter if !app.name_input.is_empty() => {
                        app.name = std::mem::take(&mut app.name_input);
                        break;
                    }
                    KeyCode::Char(c) => app.name_input.push(c),
                    KeyCode::Backspace => {
                        app.name_input.pop();
                    }
                    _ => {}
                }
            }
        }
    }

    // ── Phase 2: Start the network task ───────────────────────────────
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<AppEvent>();

    // Shared mute flag (UI toggles it, mic callback reads it).
    let muted_flag = Arc::new(AtomicBool::new(false));

    tokio::spawn(net::run(
        topic,
        bootstrap,
        cmd_rx,
        evt_tx,
        muted_flag.clone(),
        app.name.clone(),
    ));

    // ── Set up audio playback (optional — app works without it) ───────
    let mut playback = match playback::Playback::new() {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("warning: audio playback unavailable: {e}");
            None
        }
    };

    // ── Phase 3: Main chat loop ───────────────────────────────────────
    loop {
        term.draw(|f| ui::draw(f, &app))?;

        // Drain any network events into UI state.
        while let Ok(ev) = evt_rx.try_recv() {
            match ev {
                AppEvent::Message(m) => app.messages.push(m),
                AppEvent::PeerConnected(id) => {
                    if !app.peers.contains(&id) {
                        app.peers.push(id);
                    }
                }
                AppEvent::PeerDisconnected(id) => {
                    app.peers.retain(|p| p != &id);
                    // Fix up the selected index if it's now out of bounds.
                    if !app.peers.is_empty() {
                        app.selected_peer %= app.peers.len();
                    } else {
                        app.selected_peer = 0;
                    }
                }
                AppEvent::Ticket(t) => app.invite = Some(t),
                AppEvent::VoiceFrame(bytes) => {
                    if let Some(p) = &mut playback {
                        p.push_opus(&bytes);
                    }
                }
            }
        }

        // Poll keyboard with a short timeout so the loop keeps spinning
        // (this lets us drain network events promptly).
        if ct_event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(k) = ct_event::read()? {
                match k.code {
                    // Send message
                    KeyCode::Enter if !app.input.is_empty() => {
                        let text = std::mem::take(&mut app.input);
                        let _ = cmd_tx.send(Command::SendText(text));
                    }

                    // Ctrl+K: start call / hang up
                    KeyCode::Char('k') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                        if app.in_call {
                            let _ = cmd_tx.send(Command::HangUp);
                            app.in_call = false;
                        } else if let Some(addr) = app.selected_peer_addr() {
                            let _ = cmd_tx.send(Command::StartCall(addr));
                            app.in_call = true;
                        }
                    }

                    // Ctrl+M: toggle mute
                    KeyCode::Char('m') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.muted = !app.muted;
                        muted_flag.store(app.muted, Ordering::Relaxed);
                    }

                    // Tab: cycle selected peer
                    KeyCode::Tab => {
                        app.select_next_peer();
                    }

                    // Type a character
                    KeyCode::Char(c) => app.input.push(c),

                    // Backspace
                    KeyCode::Backspace => {
                        app.input.pop();
                    }

                    // Esc: quit
                    KeyCode::Esc => {
                        let _ = cmd_tx.send(Command::Quit);
                        break;
                    }

                    _ => {}
                }
            }
        }
    }

    // ── Restore the terminal ──────────────────────────────────────────
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
