//! Starling Server — binary entrypoint.
//!
//! A headless roost server. Run it to keep a community's chat history
//! online and serve it to late-joining peers. No TUI, no GUI — just
//! the command line.
//!
//! # Commands
//!
//! ```text
//! starling roost create   <name>              create a new roost
//! starling roost open     <name>              start a roost (blocks)
//! starling roost close    <name>              stop a running roost
//! starling roost destroy  <name>              delete a roost and all data
//! starling roost setup    <name>              alias for create
//! starling roost invite   <name>              show invite code
//! starling roost status   <name>              show roost info
//! starling roost doctor   <name>              diagnose a roost
//! starling roost logs     <name>              show log info
//! starling roost members  <name>              list members (coming)
//! starling roost channel add <n> <ch>        add a channel (coming)
//! starling roost channel remove <n> <ch>     remove a channel (coming)
//! starling server version                    print version
//! starling server update                     print update instructions
//! starling server uninstall                  print uninstall instructions
//! starling help                              print this help
//! ```

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let Some(cmd) = args.get(1).map(String::as_str) else {
        print_usage();
        return Ok(());
    };

    match cmd {
        // ── Roost management ────────────────────────────────────────
        "roost" => match args.get(2).map(String::as_str) {
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
                eprintln!();
                eprintln!("Commands:");
                eprintln!("  create   <name>              create a new roost");
                eprintln!("  open     <name>              start a roost (blocks)");
                eprintln!("  close    <name>              stop a running roost");
                eprintln!("  destroy  <name>              delete a roost and all data");
                eprintln!("  setup    <name>              alias for create");
                eprintln!("  invite   <name>              show invite code");
                eprintln!("  status   <name>              show roost info");
                eprintln!("  doctor   <name>              diagnose a roost");
                eprintln!("  logs     <name>              show log info");
                eprintln!("  members  <name>              list members (coming)");
                eprintln!("  channel add <n> <ch>        add a channel (coming)");
                eprintln!("  channel remove <n> <ch>     remove a channel (coming)");
                std::process::exit(1);
            }
        },

        // ── Server meta commands ────────────────────────────────────
        "server" => match args.get(2).map(String::as_str) {
            Some("version") => {
                println!("Starling Server v{}", env!("CARGO_PKG_VERSION"));
                Ok(())
            }
            Some("update") => {
                println!("To update Starling Server:");
                println!(
                    "  cargo install starling-server --git https://forgejo.hearthhome.lol/Saltfault/Starling-Server.git"
                );
                Ok(())
            }
            Some("uninstall") => {
                println!("To uninstall Starling Server:");
                println!("  cargo uninstall starling-server");
                Ok(())
            }
            _ => {
                eprintln!("Usage: starling server <version|update|uninstall>");
                std::process::exit(1);
            }
        },

        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }

        _ => {
            eprintln!("Unknown command: {cmd}");
            eprintln!();
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    println!(
        "Starling Server v{} — headless roost server",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("Usage:");
    println!("  starling roost create   <name>              create a new roost");
    println!("  starling roost open     <name>              start a roost (blocks)");
    println!("  starling roost close    <name>              stop a running roost");
    println!("  starling roost destroy  <name>              delete a roost and all data");
    println!("  starling roost setup    <name>              alias for create");
    println!("  starling roost invite   <name>              show invite code");
    println!("  starling roost status   <name>              show roost info");
    println!("  starling roost doctor   <name>              diagnose a roost");
    println!("  starling roost logs     <name>              show log info");
    println!("  starling roost members  <name>              list members (coming)");
    println!("  starling roost channel add <n> <ch>        add a channel (coming)");
    println!("  starling roost channel remove <n> <ch>     remove a channel (coming)");
    println!("  starling server version                    print version");
    println!("  starling server update                     print update instructions");
    println!("  starling server uninstall                  print uninstall instructions");
    println!("  starling help                              print this help");
    println!();
    println!("A roost is a persistent bird that stays online, stores chat");
    println!("history to disk, and serves it to late-joining peers.");
    println!();
    println!("Each roost lives under ~/.config/starling/roosts/<name>/ and");
    println!("gets its own cryptographic identity key.");
}
