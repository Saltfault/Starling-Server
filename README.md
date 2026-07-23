# Starling Server

The **roost** server binary for **[Starling](https://forgejo.hearthhome.lol/Saltfault/Starling)** — a federated, peer-to-peer communications platform.

The shared protocol library (networking, crypto, types) was extracted to the [`starling`](https://forgejo.hearthhome.lol/Saltfault/Starling) crate, which both this server and the TUI depend on.

> ⚠️ **Early access — under active development.** Roost lifecycle (create/open/destroy) and history persistence work today. Membership and channel management are landing with roles (see the roadmap). Expect rough edges and breaking changes; bug reports and feedback are welcome.

---

# For operators — running a roost

A **roost** is a persistent bird. It has no TUI and no GUI: you drive it entirely from the command line, and it stays running in a terminal (or as a service) to keep your community online.

Each roost lives under `~/.config/starling/roosts/<name>/` (Unix) or `%APPDATA%/starling/roosts/<name>/` (Windows), with its own cryptographic identity key and sled database.

## Install

The roost server is a component of **[Starling](https://forgejo.hearthhome.lol/Saltfault/Starling)** — install the launcher, then add the server:

```bash
# one-time: install the Starling launcher
cargo install --git https://forgejo.hearthhome.lol/Saltfault/Starling.git
# add the roost server
starling install server
```

Requires Rust and a C compiler (for the bundled Opus library). See the [platform setup in the TUI README](https://forgejo.hearthhome.lol/Saltfault/Starling-TUI#platform-setup) — the toolchain requirements are identical.

## Quick start

```bash
# 1. Create a roost (mints its identity + database, prints the invite code)
starling roost create my-community

# 2. Start it — stays online until you press Ctrl+C
starling roost open my-community

# 3. Anyone joins from a client using the printed invite code:
#    starling join BIRD-...
```

## Command reference

All roost commands take the roost's `<name>`.

| Command | Status | Does |
|---------|:------:|------|
| `starling roost create <name>` | ✅ | Create a roost: identity key + sled database. Prints the invite code |
| `starling roost setup <name>` | ✅ | Alias for `create` |
| `starling roost open <name>` | ✅ | Start the roost (blocks until Ctrl+C). Persists messages, serves history |
| `starling roost close <name>` | ✅ | Prints how to stop a running roost (Ctrl+C, or your service manager) |
| `starling roost destroy <name>` | ✅ | Delete a roost and **all** its data |
| `starling roost invite <name>` | ✅ | Print the roost's invite code |
| `starling roost status <name>` | ✅ | Show roost info (identity, data path, and more) |
| `starling roost doctor <name>` | ✅ | Diagnose a roost's setup |
| `starling roost logs <name>` | ✅ | Show the roost's log location |
| `starling roost members <name>` | 📋 | List members — arrives with roles (Phase 8/9) |
| `starling roost channel add <name> <ch>` | 📋 | Add a channel — arrives with roles (Phase 8) |
| `starling roost channel remove <name> <ch>` | 📋 | Remove a channel — arrives with roles (Phase 8) |
| `starling install server` | ✅ | Install (or reinstall) the roost server |
| `starling update server` | ✅ | Update the server to the latest version |
| `starling uninstall server` | ✅ | Uninstall the server and remove config/roost data |
| `starling help` | ✅ | Print usage |

> **Note:** these `starling roost …` subcommands become available after `starling install server`. You always run them through the `starling` command; `starling-server` is the crate name (used as a library dependency, see below).

## Running as a service

`roost open` blocks, which makes it a natural fit for a process supervisor. Point a systemd unit, a `tmux`/`screen` session, or your init system of choice at `starling roost open <name>`. Because iroh handles NAT traversal and relay fallback, **no port forwarding or public IP is required** — a roost behind home Wi-Fi still serves its community.

**Backups are your responsibility.** A roost's history and identity live only under its data directory — copy `~/.config/starling/roosts/<name>/` to back it up. There is no company keeping a copy behind the scenes; that's the point.

---

---

## License

Apache 2.0
