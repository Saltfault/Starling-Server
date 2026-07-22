//! Setup wizard — a separate TUI for configuring the user profile.
//!
//! Run with `starling setup`. Guides the user through:
//!
//! 1. Optional: load a profile from a 32-digit code
//! 2. Enter display name
//! 3. Select input (microphone) device
//! 4. Select output (speaker) device
//! 5. Review summary, save, and show the profile code
//!
//! The profile is saved to disk and loaded automatically by `starling open`
//! and `starling join` on subsequent launches.

use crate::config::Profile;
use crate::util::suppress_stderr;
use cpal::traits::HostTrait;
use crossterm::event::{self as ct_event, Event, KeyCode};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

/// Which step of the setup wizard we're on.
enum Phase {
    CodeEntry,
    NameEntry,
    InputDevice,
    OutputDevice,
    Summary,
}

/// Setup wizard state.
struct SetupApp {
    phase: Phase,
    profile: Profile,
    name_input: String,
    code_input: String,
    input_devices: Vec<String>,
    output_devices: Vec<String>,
    selected_input: usize,
    selected_output: usize,
}

impl SetupApp {
    fn new() -> Self {
        let input_devices = suppress_stderr(list_input_devices);
        let output_devices = suppress_stderr(list_output_devices);
        let profile = Profile::load().unwrap_or_default();

        Self {
            phase: Phase::CodeEntry,
            name_input: profile.name.clone(),
            code_input: String::new(),
            input_devices,
            output_devices,
            selected_input: 0,
            selected_output: 0,
            profile,
        }
    }
}

fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut devices = vec!["System Default".to_string()];
    if let Ok(iter) = host.input_devices() {
        for device in iter {
            let name = device.to_string();
            if !name.is_empty() {
                devices.push(name);
            }
        }
    }
    devices
}

fn list_output_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut devices = vec!["System Default".to_string()];
    if let Ok(iter) = host.output_devices() {
        for device in iter {
            let name = device.to_string();
            if !name.is_empty() {
                devices.push(name);
            }
        }
    }
    devices
}

/// Run the setup wizard. Returns `Some(Profile)` if the user saved, or
/// `None` if they cancelled.
pub fn run_setup(
    term: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
) -> anyhow::Result<Option<Profile>> {
    let mut app = SetupApp::new();

    loop {
        term.draw(|f| draw(f, &app))?;

        if !ct_event::poll(std::time::Duration::from_millis(50))? {
            continue;
        }
        if let Event::Key(k) = ct_event::read()? {
            match app.phase {
                Phase::CodeEntry => match k.code {
                    KeyCode::Enter => {
                        if !app.code_input.is_empty() {
                            if let Some(p) = Profile::from_code(&app.code_input) {
                                app.name_input = p.name.clone();
                                app.profile = p;
                            }
                        }
                        app.phase = Phase::NameEntry;
                    }
                    KeyCode::Char(c) => app.code_input.push(c),
                    KeyCode::Backspace => {
                        app.code_input.pop();
                    }
                    KeyCode::Esc => return Ok(None),
                    _ => {}
                },
                Phase::NameEntry => match k.code {
                    KeyCode::Enter if !app.name_input.is_empty() => {
                        app.profile.name = app.name_input.clone();
                        app.phase = Phase::InputDevice;
                    }
                    KeyCode::Char(c) => app.name_input.push(c),
                    KeyCode::Backspace => {
                        app.name_input.pop();
                    }
                    KeyCode::Esc => return Ok(None),
                    _ => {}
                },
                Phase::InputDevice => match k.code {
                    KeyCode::Enter => {
                        app.profile.input_device = if app.selected_input == 0 {
                            None
                        } else {
                            Some(app.input_devices[app.selected_input].clone())
                        };
                        app.phase = Phase::OutputDevice;
                    }
                    KeyCode::Up => {
                        if app.selected_input > 0 {
                            app.selected_input -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if app.selected_input + 1 < app.input_devices.len() {
                            app.selected_input += 1;
                        }
                    }
                    KeyCode::Esc => return Ok(None),
                    _ => {}
                },
                Phase::OutputDevice => match k.code {
                    KeyCode::Enter => {
                        app.profile.output_device = if app.selected_output == 0 {
                            None
                        } else {
                            Some(app.output_devices[app.selected_output].clone())
                        };
                        app.phase = Phase::Summary;
                    }
                    KeyCode::Up => {
                        if app.selected_output > 0 {
                            app.selected_output -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if app.selected_output + 1 < app.output_devices.len() {
                            app.selected_output += 1;
                        }
                    }
                    KeyCode::Esc => return Ok(None),
                    _ => {}
                },
                Phase::Summary => match k.code {
                    KeyCode::Enter => {
                        app.profile.save()?;
                        return Ok(Some(app.profile));
                    }
                    KeyCode::Esc => return Ok(None),
                    _ => {}
                },
            }
        }
    }
}

// ── Rendering ───────────────────────────────────────────────────────────

fn draw(f: &mut Frame, app: &SetupApp) {
    let area = f.area();
    f.render_widget(Clear, area);

    let width = 60.min(area.width);
    let height = 20.min(area.height);
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
            .title(" Starling Setup "),
        popup,
    );

    let inner = popup.inner(Margin {
        vertical: 1,
        horizontal: 2,
    });

    match app.phase {
        Phase::CodeEntry => draw_code_entry(f, inner, app),
        Phase::NameEntry => draw_name_entry(f, inner, app),
        Phase::InputDevice => draw_device_list(
            f,
            inner,
            "Input Device (Microphone)",
            &app.input_devices,
            app.selected_input,
        ),
        Phase::OutputDevice => draw_device_list(
            f,
            inner,
            "Output Device (Speaker)",
            &app.output_devices,
            app.selected_output,
        ),
        Phase::Summary => draw_summary(f, inner, app),
    }
}

fn draw_code_entry(f: &mut Frame, area: Rect, app: &SetupApp) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(area);

    f.render_widget(
        Paragraph::new("Load a profile from a 32-digit code,"),
        chunks[0],
    );
    f.render_widget(Paragraph::new("or press Enter to start fresh."), chunks[1]);
    f.render_widget(
        Paragraph::new(format!(" Code: {}_", app.code_input)).style(Style::new().fg(Color::Yellow)),
        chunks[3],
    );
    f.render_widget(
        Paragraph::new(" Enter = continue . Esc = cancel").style(Style::new().fg(Color::DarkGray)),
        chunks[5],
    );
}

fn draw_name_entry(f: &mut Frame, area: Rect, app: &SetupApp) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(area);

    f.render_widget(
        Paragraph::new("Enter your display name . the name"),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new("other birds see next to your messages."),
        chunks[1],
    );
    f.render_widget(
        Paragraph::new(format!(" Name: {}_", app.name_input)).style(Style::new().fg(Color::Yellow)),
        chunks[3],
    );
    f.render_widget(
        Paragraph::new(" Enter = continue . Esc = cancel").style(Style::new().fg(Color::DarkGray)),
        chunks[4],
    );
}

fn draw_device_list(f: &mut Frame, area: Rect, title: &str, devices: &[String], selected: usize) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);

    f.render_widget(Paragraph::new(title), chunks[0]);
    f.render_widget(Paragraph::new(""), chunks[1]);

    let items: Vec<ListItem> = devices
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let prefix = if i == selected { "> " } else { "  " };
            ListItem::new(format!("{prefix}{name}"))
        })
        .collect();

    f.render_widget(
        List::new(items).style(Style::new().fg(Color::White)),
        chunks[2],
    );

    f.render_widget(
        Paragraph::new(" Up/Down = navigate . Enter = select . Esc = cancel")
            .style(Style::new().fg(Color::DarkGray)),
        chunks[3],
    );
}

fn draw_summary(f: &mut Frame, area: Rect, app: &SetupApp) {
    let input_name = app
        .profile
        .input_device
        .as_deref()
        .unwrap_or("System Default");
    let output_name = app
        .profile
        .output_device
        .as_deref()
        .unwrap_or("System Default");
    let code = app.profile.to_code();

    let lines = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Name:   "),
            Span::styled(&app.profile.name, Style::new().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("  Input:  "),
            Span::styled(input_name, Style::new().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("  Output: "),
            Span::styled(output_name, Style::new().fg(Color::Cyan)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Profile code: "),
            Span::styled(code, Style::new().fg(Color::Green).bold()),
        ]),
        Line::raw(""),
        Line::raw("  Save this code to restore your name on"),
        Line::raw("  another machine with: starling setup"),
        Line::raw(""),
        Line::styled(
            "  Enter = save & exit . Esc = cancel",
            Style::new().fg(Color::DarkGray),
        ),
    ];

    f.render_widget(Paragraph::new(lines), area);
}
