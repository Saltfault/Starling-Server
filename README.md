# Starling Server

The **roost** server and shared protocol library for **[Starling](https://forgejo.hearthhome.lol/Saltfault/Starling)** — a federated, peer-to-peer communications platform.

This crate is two things at once:

1. **A roost server** (`starling-server` binary) — a persistent bird that stays online, stores a community's chat history to disk, and serves it to birds who join later. Your own community server, run on your own machine, over the peer-to-peer murmuration instead of a company's cloud.
2. **A shared protocol library** (`starling_server` lib) — the networking, crypto, and voice/video pipelines that every Starling client embeds. The TUI, and the planned Desktop/Android/Web clients, all build on this.

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

# For developers — using the library

Every Starling client embeds this crate for protocol handling, so a client repo contains only UI code.

```toml
[dependencies]
starling-server = { git = "https://forgejo.hearthhome.lol/Saltfault/Starling-Server.git" }
```

## Modules

| Module | Purpose |
|--------|---------|
| `net` | iroh endpoint, gossip subscription, voice/video protocol handlers |
| `roost` | Headless server mode — durable message store, history sync |
| `call` | Opens/accepts QUIC streams for voice datagrams and video |
| `voice` | Mic capture: cpal input → Opus encoder → channel |
| `playback` | Audio output: channel → Opus decoder → ring buffer → cpal output |
| `video` | Webcam capture (nokhwa) → JPEG frames → channel, plus terminal rendering |
| `opus_ffi` | Safe Rust wrappers around the pre-built Opus C library |
| `crypto` | E2E encryption (ChaCha20-Poly1305) for gossip messages |
| `event` | Shared types: `Command`, `AppEvent`, `ChatMessage`, `GossipPayload` |
| `config` | Profile persistence, identity key, 32-digit code |
| `sync` | History backfill for late-joining peers |
| `logger` | File logger with gzipped log rotation |
| `util` | Platform utilities (stderr suppression on Unix) |

The UI-facing contract is the `Command` (UI → network) and `AppEvent` (network → UI) enums in `event`. A client spawns `net::run`, feeds it `Command`s, and renders `AppEvent`s — the same shape the TUI uses.

## Clients on this library

| Client | Repo | Status |
|--------|------|--------|
| Starling TUI | [Starling-TUI](https://forgejo.hearthhome.lol/Saltfault/Starling-TUI) | 🚧 Active — text, voice, video |
| Starling Desktop | [Starling-Desktop](https://forgejo.hearthhome.lol/Saltfault/Starling-Desktop) | 📋 Planned |
| Starling Android | [Starling-Android](https://forgejo.hearthhome.lol/Saltfault/Starling-Android) | 📋 Planned |
| Starling Web | [Starling-Web](https://forgejo.hearthhome.lol/Saltfault/Starling-Web) | 📋 Planned |

---

## License

Apache 2.0
