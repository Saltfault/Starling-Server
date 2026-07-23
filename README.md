# Starling Server

A headless **roost** server for [Starling](https://forgejo.hearthhome.lol/Saltfault/Starling) —
a federated peer-to-peer communications platform.

This crate provides two things:

1. **A shared protocol library** — used by all Starling clients (TUI, Desktop,
   Android, Web) for networking, cryptography, voice/video pipelines, and
   protocol handlers.
2. **A headless server binary** — a roost that stays online persistently,
   persists flock history to disk via sled, and serves history to late-joining
   peers. No TUI or GUI required.

## Modules

| Module | Purpose |
|--------|---------|
| `net` | iroh endpoint, gossip subscription, voice/video protocol handlers |
| `roost` | Headless server mode — durable message store, history sync |
| `call` | Opens/accepts QUIC streams for voice datagrams and video |
| `voice` | Mic capture: cpal input → Opus encoder → channel |
| `playback` | Audio output: channel → Opus decoder → ring buffer → cpal output |
| `video` | Webcam capture: nokhwa → JPEG frames → channel, terminal rendering |
| `opus_ffi` | Safe Rust wrappers around the pre-built Opus C library |
| `crypto` | E2E encryption (ChaCha20-Poly1305) for gossip messages |
| `event` | Shared types: `Command`, `AppEvent`, `ChatMessage`, `GossipPayload` |
| `config` | Profile persistence, identity key, 32-digit code |
| `sync` | History backfill for late-joining peers |
| `logger` | File logger with gzipped log rotation |
| `util` | Platform utilities (stderr suppression on Unix) |

## Usage

### Library

Add to your `Cargo.toml`:

```toml
[dependencies]
starling-server = { git = "https://forgejo.hearthhome.lol/Saltfault/Starling-Server.git" }
```

### Binary — Roost server (CLI only, never TUI)

A **roost** is a persistent bird that stays online to keep a community's
chat history and relay state to late-joiners. No TUI, no GUI — just the
terminal.

**Create a roost:**

```bash
starling-server roost create my-community
```

This generates a dedicated identity key and sled database. It prints the
roost's invite code so others can join.

**Start the roost server:**

```bash
starling-server roost open my-community
```

The roost stays online until you press Ctrl+C. It persists every message
to disk and serves history to birds that join later.

**Join a roost** from any Starling client:

```bash
starling join BIRD-...
```

Data lives under `~/.config/starling/roosts/<name>/`
(on Unix) or `%APPDATA%/starling/roosts/<name>/` (on Windows).

## Clients using this library

| Client | Repo | Status |
|--------|------|--------|
| Starling TUI | [Starling-TUI](https://forgejo.hearthhome.lol/Saltfault/Starling-TUI) | ✅ Text, voice, video |
| Starling Desktop | [Starling-Desktop](https://forgejo.hearthhome.lol/Saltfault/Starling-Desktop) | 📋 Planned |
| Starling Android | [Starling-Android](https://forgejo.hearthhome.lol/Saltfault/Starling-Android) | 📋 Planned |
| Starling Web | [Starling-Web](https://forgejo.hearthhome.lol/Saltfault/Starling-Web) | 📋 Planned |

## License

Apache 2.0
