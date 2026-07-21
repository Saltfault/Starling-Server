# Starling

A minimal voice + text chat app built on [iroh](https://iroh.computer) gossip
and QUIC. Runs in the terminal, peers discover each other via a shared invite
ticket, and voice calls are direct peer-to-peer Opus streams.

## Quick start

```bash
just install-deps    # one-time: installs system packages
just run             # build + run, starts a new session
```

Share the invite ticket (shown in the header) with a friend. They join with:

```bash
just join <ticket>
```

Or without `just`:

```bash
cargo run -- open        # start a session
cargo run -- join <ticket>   # join an existing session
```

Set your display name with the `STARLING_NAME` environment variable:

```bash
STARLING_NAME=Alice just run
```

## Prerequisites

### Linux / WSL2

```bash
just install-deps
```

Or manually:

```bash
# Debian/Ubuntu
sudo apt install build-essential cmake pkg-config libasound2-dev libpulse-dev

# Fedora
sudo dnf install gcc cmake pkgconf-pkg-config alsa-lib-devel pulseaudio-libs-devel

# Arch
sudo pacman -S base-devel cmake pkgconf alsa-lib pulseaudio
```

| Package | Why it's needed |
|---------|----------------|
| `build-essential` (gcc) | Compiling native C code (Opus, ring crypto) |
| `cmake` | Building libopus from source via `audiopus_sys` |
| `pkg-config` | Locating ALSA and PulseAudio libraries at build time |
| `libasound2-dev` | ALSA headers — cpal compiles the ALSA backend on Linux |
| `libpulse-dev` | PulseAudio headers — cpal uses PulseAudio at runtime on WSLg |

**WSL2 audio:** No extra setup needed. WSLg (Windows 11) provides a
PulseAudio server automatically at `/mnt/wslg/PulseServer`. The app connects
to it directly — no `libasound2-plugins` or `/etc/asound.conf` required.

### Windows

Install [CMake](https://cmake.org/download/) and add it to your `PATH`.
A C/C++ compiler (MSVC via Visual Studio Build Tools) is also required.

### macOS

```bash
brew install cmake pkg-config
```

CoreAudio is used for audio I/O (no extra audio packages needed).

## Keybindings

| Key | Action |
|-----|--------|
| `Enter` | Send typed message |
| `Ctrl+K` | Start call with selected peer / hang up |
| `Ctrl+M` | Toggle mute |
| `Tab` | Cycle selected peer |
| `Backspace` | Delete last character |
| `Esc` | Quit |

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│ main.rs (UI loop)                                                │
│   keyboard → Command ──┐                                         │
│   AppEvent ←───────────┤──── mpsc channels ────┐                │
│   playback ← VoiceFrame│                       │                │
└────────────────────────┊────────────────────────┊───────────────┘
                         ▼                        ▼
┌──────────────────────────────────────────────────────────────────┐
│ net.rs (network task)                                            │
│   gossip for chat · QUIC datagrams for voice                     │
│   mic capture (voice.rs) → place_call (call.rs)                  │
└──────────────────────────────────────────────────────────────────┘
```

- **`event.rs`** — `Command` (UI→net) and `AppEvent` (net→UI) types
- **`net.rs`** — owns the iroh endpoint, gossip subscription, voice handler
- **`call.rs`** — opens/accepts QUIC streams for voice datagrams
- **`voice.rs`** — mic capture: cpal input → Opus encoder → channel
- **`playback.rs`** — audio output: channel → Opus decoder → ring buffer → cpal output
- **`ui.rs`** — terminal rendering and UI state (`App` struct)
- **`main.rs`** — event loop, keyboard handling, wires everything together

Audio is encoded as 48 kHz mono Opus, 20 ms frames (960 samples), sent as
QUIC datagrams. Playback uses a 2-second ring buffer to absorb jitter.

## License

MIT