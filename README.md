# Starling Server

The shared protocol library for [Starling](https://forgejo.hearthhome.lol/Saltfault/Starling) —
a federated peer-to-peer communications platform.

This crate contains everything that talks to the murmuration: networking,
cryptography, voice/video pipelines, and protocol handlers. It has no UI
code — clients (TUI, Desktop, Android, Web) depend on this library and
provide their own UI layer.

## Modules

| Module | Purpose |
|--------|---------|
| `net` | iroh endpoint, gossip subscription, voice/video protocol handlers |
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

Add to your `Cargo.toml`:

```toml
[dependencies]
starling-server = { git = "https://forgejo.hearthhome.lol/Saltfault/Starling-Server.git" }
```

## Clients using this library

| Client | Repo | Status |
|--------|------|--------|
| Starling TUI | [Starling-TUI](https://forgejo.hearthhome.lol/Saltfault/Starling-TUI) | ✅ Text, voice, video |
| Starling Desktop | [Starling-Desktop](https://forgejo.hearthhome.lol/Saltfault/Starling-Desktop) | 📋 Planned |
| Starling Android | [Starling-Android](https://forgejo.hearthhome.lol/Saltfault/Starling-Android) | 📋 Planned |
| Starling Web | [Starling-Web](https://forgejo.hearthhome.lol/Saltfault/Starling-Web) | 📋 Planned |

## License

Apache 2.0
