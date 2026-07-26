//! Starling Server — binary entrypoint.

fn main() -> anyhow::Result<()> {
    starling::logger::init()?;
    let args: Vec<String> = std::env::args().collect();

    let Some(cmd) = args.get(1).map(String::as_str) else {
        eprintln!("Usage: starling roost <command> [args]");
        std::process::exit(1);
    };

    if cmd == "--version" {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if cmd != "roost" {
        eprintln!("Usage: starling roost <command> [args]");
        std::process::exit(1);
    }

    match args.get(2).map(String::as_str) {
        Some("create") => {
            let name = args.get(3).cloned().unwrap_or_else(|| {
                eprintln!("Usage: starling roost create <name>");
                std::process::exit(1);
            });
            starling_server::roost::create(&name)
        }
        Some("open") => {
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
                eprintln!("Usage: starling roost open <name> [--silent] [--bg|--background]");
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
        Some("close") => {
            let name = args.get(3).cloned().unwrap_or_else(|| {
                eprintln!("Usage: starling roost close <name>");
                std::process::exit(1);
            });
            starling_server::roost::request_shutdown(&name)
        }
        Some("destroy") => {
            let name = args.get(3).cloned().unwrap_or_else(|| {
                eprintln!("Usage: starling roost destroy <name> [--force]");
                std::process::exit(1);
            });
            let force = args.get(4).map(String::as_str) == Some("--force");
            starling_server::roost::destroy(&name, force)
        }
        Some("invite") => {
            let name = args.get(3).cloned().unwrap_or_else(|| {
                eprintln!("Usage: starling roost invite <name>");
                std::process::exit(1);
            });
            starling_server::roost::invite(&name)
        }
        Some("status") => {
            let name = args.get(3).cloned().unwrap_or_else(|| {
                eprintln!("Usage: starling roost status <name>");
                std::process::exit(1);
            });
            starling_server::roost::status(&name)
        }
        Some("doctor") => {
            let name = args.get(3).cloned().unwrap_or_else(|| {
                eprintln!("Usage: starling roost doctor <name>");
                std::process::exit(1);
            });
            starling_server::roost::doctor(&name)
        }
        Some("logs") => {
            let name = args.get(3).cloned().unwrap_or_else(|| {
                eprintln!("Usage: starling roost logs <name>");
                std::process::exit(1);
            });
            starling_server::roost::logs(&name)
        }
        Some("members") => {
            let name = args.get(3).cloned().unwrap_or_else(|| {
                eprintln!("Usage: starling roost members <name>");
                std::process::exit(1);
            });
            starling_server::roost::members(&name)
        }
        Some("channel") => match args.get(3).map(String::as_str) {
            Some("add") => {
                let name = args.get(4).cloned().unwrap_or_else(|| {
                    eprintln!("Usage: starling roost channel add <name> <channel>");
                    std::process::exit(1);
                });
                let channel = args.get(5).cloned().unwrap_or_else(|| {
                    eprintln!("Usage: starling roost channel add <name> <channel>");
                    std::process::exit(1);
                });
                starling_server::roost::add_channel(&name, &channel)
            }
            Some("remove") => {
                let name = args.get(4).cloned().unwrap_or_else(|| {
                    eprintln!("Usage: starling roost channel remove <name> <channel>");
                    std::process::exit(1);
                });
                let channel = args.get(5).cloned().unwrap_or_else(|| {
                    eprintln!("Usage: starling roost channel remove <name> <channel>");
                    std::process::exit(1);
                });
                starling_server::roost::remove_channel(&name, &channel)
            }
            _ => {
                eprintln!("Usage: starling roost channel add|remove <name> <channel>");
                std::process::exit(1);
            }
        },
        _ => {
            eprintln!("Usage: starling roost <command> [args]");
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
