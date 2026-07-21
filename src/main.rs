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
use tokio::sync::mpsc;
use ui::App;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // decide topic + who to bootstrap from
    let args: Vec<String> = std::env::args().collect();
    let (topic, bootstrap) = match args.get(1).map(String::as_str) {
        Some("join") => {
            // a ticket carries the topic-opener's address(es)
            let ticket: EndpointTicket = args[2].parse()?;
            // register the opener's addr, then bootstrap from their id
            (
                net::topic_for("starling/global"),
                vec![ticket.endpoint_addr().id],
            )
        }
        _ => (net::topic_for("starling/global"), vec![]), // "open"
    };

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<AppEvent>();

    // network runs on its own task
    tokio::spawn(net::run(topic, bootstrap, cmd_rx, evt_tx));

    // set up the terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut term = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))?;
    let mut app = App::default();
    let mut playback = playback::Playback::new();

    loop {
        term.draw(|f| ui::draw(f, &app))?;

        // drain any network events into UI state
        while let Ok(ev) = evt_rx.try_recv() {
            match ev {
                AppEvent::Message(m) => app.messages.push(m),
                AppEvent::PeerConnected(_) => app.peers += 1,
                AppEvent::PeerDisconnected(_) => app.peers = app.peers.saturating_sub(1),
                AppEvent::Ticket(t) => app.invite = Some(t), // show in header
                AppEvent::VoiceFrame(bytes) => playback.push_opus(&bytes),
            }
        }

        // poll keyboard with a short timeout so the loop keeps spinning
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
                        app.muted = !app.muted; // gate mic_tx.send() on this in start_capture
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

    // restore the terminal
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
