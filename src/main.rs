//! Starling — a federated p2p communications platform where peers, known as
//! birds, communicate via the murmuration.
//!
//! Subcommands:
//! - `starling setup` — configure profile (name, audio devices, code)
//! - `starling open`  — start a new flock
//! - `starling join <node-id>` — join an existing flock
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

    // ── Subcommand: `starling open` or `starling join <node-id>` ──────
    //
    // For "open": no bootstrap — we're the opener, joiners connect to us.
    // For "join <node-id>": bootstrap with the opener's node ID so the
    // gossip protocol connects us to them.
    let bootstrap = match args.get(1).map(String::as_str) {
        Some("join") => {
            let node_id: iroh::EndpointId = args[2].parse()?;
            vec![node_id]
        }
        _ => vec![],
    };

    // Load profile from disk (if it exists).
    let profile = config::Profile::load();

    // Set up the terminal.
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut term = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))?;
    let mut app = App::default();

    // For "join", we know the room code immediately (derived from the
    // opener's node ID). For "open", it arrives via AppEvent::Ticket.
    if let Some(node_id_str) = args.get(2) {
        if let Ok(node_id) = node_id_str.parse::<iroh::EndpointId>() {
            app.invite = Some(net::room_code_from_node_id(&node_id));
            app.ticket = Some(node_id_str.clone());
        }
    }

    // If a profile exists, use its name. Otherwise, show the name popup.
    let (name, input_device, output_device) = if let Some(p) = &profile {
        app.name = p.name.clone();
        (
            p.name.clone(),
            p.input_device.clone(),
            p.output_device.clone(),
        )
    } else {
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

    // Start the network task. The topic, room code, and E2E key are all
    // derived inside net::run from the opener's node ID.
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<AppEvent>();
    let muted_flag = Arc::new(AtomicBool::new(false));

    tokio::spawn(net::run(
        bootstrap,
        cmd_rx,
        evt_tx,
        muted_flag.clone(),
        name,
        input_device,
    ));

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
                AppEvent::Ticket(node_id_str) => {
                    // For "open": this is our own node ID — derive the room
                    // code from it and update the display. This is also the
                    // invite ticket that other birds use to join.
                    if app.invite.is_none() {
                        if let Ok(node_id) = node_id_str.parse::<iroh::EndpointId>() {
                            app.invite = Some(net::room_code_from_node_id(&node_id));
                            app.ticket = Some(node_id_str);
                        }
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
