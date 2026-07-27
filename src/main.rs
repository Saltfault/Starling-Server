//! Starling Server — binary entrypoint.

fn main() -> anyhow::Result<()> {
    starling::logger::init()?;
    let args: Vec<String> = std::env::args().collect();

    let Some(cmd) = args.get(1).map(String::as_str) else {
        usage_exit();
    };

    if cmd == "--version" {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if cmd != "roost" {
        usage_exit();
    }

    match args.get(2).map(String::as_str) {
        Some("create") => starling_server::roost::create(&arg_or_exit(&args, 3, U_CREATE)),
        Some("open") => run_open(&args),
        Some("close") => starling_server::roost::request_shutdown(&arg_or_exit(&args, 3, U_CLOSE)),
        Some("destroy") => {
            let name = arg_or_exit(&args, 3, U_DESTROY);
            let force = args.get(4).map(String::as_str) == Some("--force");
            starling_server::roost::destroy(&name, force)
        }
        Some("invite") => starling_server::roost::invite(&arg_or_exit(&args, 3, U_INVITE)),
        Some("status") => starling_server::roost::status(&arg_or_exit(&args, 3, U_STATUS)),
        Some("doctor") => starling_server::roost::doctor(&arg_or_exit(&args, 3, U_DOCTOR)),
        Some("logs") => starling_server::roost::logs(&arg_or_exit(&args, 3, U_LOGS)),
        Some("members") => starling_server::roost::members(&arg_or_exit(&args, 3, U_MEMBERS)),
        Some("channel") => run_channel(&args),
        _ => usage_exit(),
    }
}

const U_CREATE: &str = "Usage: starling roost create <name>";
const U_CLOSE: &str = "Usage: starling roost close <name>";
const U_DESTROY: &str = "Usage: starling roost destroy <name> [--force]";
const U_INVITE: &str = "Usage: starling roost invite <name>";
const U_STATUS: &str = "Usage: starling roost status <name>";
const U_DOCTOR: &str = "Usage: starling roost doctor <name>";
const U_LOGS: &str = "Usage: starling roost logs <name>";
const U_MEMBERS: &str = "Usage: starling roost members <name>";
const U_OPEN: &str = "Usage: starling roost open <name> [--silent] [--bg|--background]";
const U_CHANNEL_ADD: &str = "Usage: starling roost channel add <name> <channel>";
const U_CHANNEL_REMOVE: &str = "Usage: starling roost channel remove <name> <channel>";
const U_CHANNEL: &str = "Usage: starling roost channel add|remove <name> <channel>";
const U_ROOT: &str = "Usage: starling roost <command> [args]";

fn usage_exit() -> ! {
    eprintln!("{U_ROOT}");
    std::process::exit(1);
}

fn arg_or_exit(args: &[String], idx: usize, usage: &str) -> String {
    args.get(idx).cloned().unwrap_or_else(|| {
        eprintln!("{usage}");
        std::process::exit(1);
    })
}

fn run_open(args: &[String]) -> anyhow::Result<()> {
    let mut name: Option<String> = None;
    let mut silent = false;
    let mut bg = false;
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--silent" => silent = true,
            "--bg" | "--background" => bg = true,
            arg if !arg.starts_with("--") => {
                name = Some(arg.to_string());
            }
            _ => {
                eprintln!("Unknown flag: {}", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }
    let name = name.unwrap_or_else(|| {
        eprintln!("{U_OPEN}");
        std::process::exit(1);
    });

    starling::logger::info(&format!("Starling server starting roost '{name}'"));

    if bg {
        daemonize(&name);
    }

    // Console command channel for interactive mode
    let (console_tx, console_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    if !bg {
        // Spawn a blocking thread to read stdin commands
        std::thread::spawn(move || {
            let mut line = String::new();
            loop {
                line.clear();
                match std::io::stdin().read_line(&mut line) {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let trimmed = line.trim().to_string();
                        if console_tx.send(trimmed).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    tokio::runtime::Runtime::new()
        .map_err(|e| {
            eprintln!("Failed to start tokio runtime: {e}");
            std::process::exit(1);
        })?
        .block_on(starling_server::roost::open(&name, silent, console_rx))
}

fn run_channel(args: &[String]) -> anyhow::Result<()> {
    match args.get(3).map(String::as_str) {
        Some("add") => {
            let name = arg_or_exit(args, 4, U_CHANNEL_ADD);
            let channel = arg_or_exit(args, 5, U_CHANNEL_ADD);
            starling_server::roost::add_channel(&name, &channel)
        }
        Some("remove") => {
            let name = arg_or_exit(args, 4, U_CHANNEL_REMOVE);
            let channel = arg_or_exit(args, 5, U_CHANNEL_REMOVE);
            starling_server::roost::remove_channel(&name, &channel)
        }
        _ => {
            eprintln!("{U_CHANNEL}");
            std::process::exit(1);
        }
    }
}

#[cfg(unix)]
fn daemonize(name: &str) {
    use std::process;
    // First fork — create background child
    match unsafe { libc::fork() } {
        -1 => {
            eprintln!("Failed to fork background process");
            process::exit(1);
        }
        0 => {
            // Child: create new session, detach from terminal
            if unsafe { libc::setsid() } == -1 {
                eprintln!("Failed to create new session");
                process::exit(1);
            }
            // Second fork — ensure we're not a session leader
            match unsafe { libc::fork() } {
                -1 => {
                    eprintln!("Failed to double-fork background process");
                    process::exit(1);
                }
                0 => {
                    // Grandchild: the actual daemon
                }
                pid => {
                    println!("Roost '{}' started in background (PID: {})", name, pid);
                    process::exit(0);
                }
            }
        }
        pid => {
            println!("Roost '{}' started in background (PID: {})", name, pid);
            process::exit(0);
        }
    }
}

#[cfg(not(unix))]
fn daemonize(name: &str) {
    println!(
        "Roost '{}' starting in foreground (--bg not supported on this platform)",
        name
    );
}
