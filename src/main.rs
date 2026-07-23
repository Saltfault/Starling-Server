//! Starling Server — binary entrypoint.

fn main() -> anyhow::Result<()> {
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
            let name = args.get(3).cloned().unwrap_or_else(|| {
                eprintln!("Usage: starling roost open <name>");
                std::process::exit(1);
            });
            tokio::runtime::Runtime::new()
                .map_err(|e| {
                    eprintln!("Failed to start tokio runtime: {e}");
                    std::process::exit(1);
                })?
                .block_on(starling_server::roost::open(&name))
        }
        Some("close") => {
            let _name = args.get(3).cloned().unwrap_or_else(|| {
                eprintln!("Usage: starling roost close <name>");
                std::process::exit(1);
            });
            eprintln!("To close a roost, press Ctrl+C in the terminal where it's running.");
            eprintln!("If the roost is running as a system service, use your service manager.");
            Ok(())
        }
        Some("setup") => {
            let name = args.get(3).cloned().unwrap_or_else(|| {
                eprintln!("Usage: starling roost setup <name>");
                std::process::exit(1);
            });
            starling_server::roost::create(&name)
        }
        Some("destroy") => {
            let name = args.get(3).cloned().unwrap_or_else(|| {
                eprintln!("Usage: starling roost destroy <name>");
                std::process::exit(1);
            });
            starling_server::roost::destroy(&name)
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
            let _name = args.get(3).cloned().unwrap_or_else(|| {
                eprintln!("Usage: starling roost members <name>");
                std::process::exit(1);
            });
            eprintln!("members: not yet implemented (coming in Phase 9)");
            Ok(())
        }
        Some("channel") => match args.get(3).map(String::as_str) {
            Some("add") => {
                let _name = args.get(4).cloned().unwrap_or_else(|| {
                    eprintln!("Usage: starling roost channel add <name> <channel>");
                    std::process::exit(1);
                });
                let _channel = args.get(5).cloned().unwrap_or_else(|| {
                    eprintln!("Usage: starling roost channel add <name> <channel>");
                    std::process::exit(1);
                });
                eprintln!("channel add: not yet implemented (coming in Phase 8)");
                Ok(())
            }
            Some("remove") => {
                let _name = args.get(4).cloned().unwrap_or_else(|| {
                    eprintln!("Usage: starling roost channel remove <name> <channel>");
                    std::process::exit(1);
                });
                let _channel = args.get(5).cloned().unwrap_or_else(|| {
                    eprintln!("Usage: starling roost channel remove <name> <channel>");
                    std::process::exit(1);
                });
                eprintln!("channel remove: not yet implemented (coming in Phase 8)");
                Ok(())
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
