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
//! │   gossip for chat (E2E encrypted) · QUIC datagrams for voice     │
//! │   mic capture (voice.rs) → place_call (call.rs)                  │
//! └──────────────────────────────────────────────────────────────────┘
//! ```
//!
//! Subcommands:
//! - `starling setup` — configure profile (name, audio devices, code)
//! - `starling open`  — start a new flock
//! - `starling join <code>` — join an existing flock
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
mod config;
mod crypto;
mod event;
mod logger;
mod net;
mod playback;
mod setup;
mod ui;
mod util;
mod voice;

use crossterm::{
    event::{self as ct_event, Event, KeyCode, KeyModifiers},
    execute,
    terminal::*,
};
use event::{AppEvent, Command};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use ui::App;

/// Generate a random room code: "BIRD" + 6 hex digits (e.g. "BIRD00CCFF").
fn generate_room_code() -> String {
    let uuid = uuid::Uuid::new_v4();
    let bytes = uuid.as_bytes();
    let hex: String = (0..3).map(|i| format!("{:02X}", bytes[i])).collect();
    format!("BIRD{hex}")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logger::init();

    let args: Vec<String> = std::env::args().collect();

    // ── Subcommand: `starling setup` ──────────────────────────────────
    if args.get(1).map(String::as_str) == Some("setup") {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let mut term = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))?;
        setup::run_setup(&mut term)?;
        disable_raw_mode()?;
        execute!(term.backend_mut(), LeaveAlternateScreen)?;
        return Ok(());
    }

    // ── Subcommand: `starling open` or `starling join <code>` ─────────
    let (topic, room_code) = match args.get(1).map(String::as_str) {
        Some("join") => {
            let code = args[2].clone();
            (net::topic_for(&format!("starling/flock/{code}")), code)
        }
        _ => {
            let code = generate_room_code();
            (net::topic_for(&format!("starling/flock/{code}")), code)
        }
    };

    // E2E encryption context derived from the room code.
    let flock_crypto = crypto::FlockCrypto::from_room_code(&room_code);

    // Load profile from disk (if it exists).
    let profile = config::Profile::load();

    // Set up the terminal.
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut term = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))?;
    let mut app = App::default();
    app.invite = Some(room_code);

    // If a profile exists, use its name. Otherwise, show the name popup.
    let (name, input_device, output_device) = if let Some(p) = &profile {
        app.name = p.name.clone();
        (
            p.name.clone(),
            p.input_device.clone(),
            p.output_device.clone(),
        )
    } else {
        // Name entry popup (fallback when no profile exists).
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
        (app.name.clone(), None, None)
    };

    // Start the network task with E2E crypto and mic device preference.
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<AppEvent>();
    let muted_flag = Arc::new(AtomicBool::new(false));

    tokio::spawn(net::run(
        topic,
        cmd_rx,
        evt_tx,
        muted_flag.clone(),
        name,
        flock_crypto,
        input_device,
    ));

    // Set up audio playback with device preference.
    let mut playback = match playback::Playback::new(output_device.as_deref()) {
        Ok(p) => Some(p),
        Err(e) => {
            logger::warn(&format!("audio playback unavailable: {e}"));
            None
        }
    };

    // Main chat loop.
    loop {
        term.draw(|f| ui::draw(f, &app))?;

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
                    if !app.peers.is_empty() {
                        app.selected_peer %= app.peers.len();
                    } else {
                        app.selected_peer = 0;
                    }
                }
                AppEvent::VoiceFrame(bytes) => {
                    if let Some(p) = &mut playback {
                        p.push_opus(&bytes);
                    }
                }
            }
        }

        if ct_event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(k) = ct_event::read()? {
                match k.code {
                    KeyCode::Enter if !app.input.is_empty() => {
                        let text = std::mem::take(&mut app.input);
                        let _ = cmd_tx.send(Command::SendText(text));
                    }

                    KeyCode::Char('k') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                        if app.in_call {
                            let _ = cmd_tx.send(Command::HangUp);
                            app.in_call = false;
                        } else if let Some(addr) = app.selected_peer_addr() {
                            let _ = cmd_tx.send(Command::StartCall(addr));
                            app.in_call = true;
                        }
                    }

                    KeyCode::Char('m') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.muted = !app.muted;
                        muted_flag.store(app.muted, Ordering::Relaxed);
                    }

                    KeyCode::Tab => {
                        app.select_next_peer();
                    }

                    KeyCode::Char(c) => app.input.push(c),

                    KeyCode::Backspace => {
                        app.input.pop();
                    }

                    KeyCode::Esc => {
                        let _ = cmd_tx.send(Command::Quit);
                        break;
                    }

                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
