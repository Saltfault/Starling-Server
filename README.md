# Starling

A federated peer-to-peer communications platform where peers — known as
**birds** — can communicate from anywhere in the world thanks to a
peer-to-peer network called **the murmuration**.

Starling runs in the terminal and provides text chat via gossip protocol
and voice calls via direct QUIC streams. Birds discover each other through
the murmuration using iroh's relay and discovery infrastructure — no central
server required. A room code is all a new bird needs to join a flock.

## Platform support

| Feature | Windows | macOS | Linux | WSL2 |
|---------|:-------:|:-----:|:-----:|:----:|
| Text chat | ✓ | ✓ | ✓ | ✓ |
| Voice calls (mic + playback) | ✓ | ✓ | ✓ | ✓† |
| Room codes | ✓ | ✓ | ✓ | ✓ |

† WSL2 voice requires a one-time setup step (`just setup-wsl-audio`)
that installs the ALSA→PulseAudio bridge. See
[WSL2 setup](#wsl2-windows-subsystem-for-linux) below.

---

## Getting started

**Install Starling:**

```bash
cargo install --git https://forgejo.hearthhome.lol/Saltfault/Starling.git
```

**Configure your profile (one-time):**

```bash
starling setup
```

This opens a setup wizard where you enter your display name, select your
microphone and speaker, and get a 32-digit profile code. The code encodes
your name and can be used to restore your profile on another machine. The
profile is saved to disk automatically.

**Run it:**

```bash
starling open
```

The header shows a room code like `BIRD00CCFF`. Share it with another bird —
they join with:

```bash
starling join BIRD00CCFF
```

If you haven't run `starling setup` yet, a popup will ask for your display
name on first launch. Run `starling setup` later to change your name or audio
devices.

> **Developing?** You can also clone and run from source:
> ```bash
> git clone https://forgejo.hearthhome.lol/Saltfault/Starling.git
> cd Starling
> cargo run -- open
> ```
> The `justfile` provides `just install-deps`, `just setup-wsl-audio`,
> `just run`, and `just join <code>` as shortcuts.

---

## Platform setup

Before installing Starling, you need Rust, a C compiler, and CMake (for
building the Opus codec from source). Follow the section for your platform.

### Windows

**1. Install Visual Studio C++ Build Tools** (provides the MSVC compiler):

Download from [visualstudio.microsoft.com](https://visualstudio.microsoft.com/visual-cpp-build-tools/).
In the installer, select **"Desktop development with C++"**.

**2. Install CMake:**

```powershell
winget install Kitware.CMake
```

Or download from [cmake.org/download](https://cmake.org/download/) and add
it to your `PATH`. Restart your terminal after installing.

**3. Install Rust:**

```powershell
winget install Rustlang.Rustup
```

Or download and run [rustup-init.exe](https://win.rustup.rs/x86_64).

**4. Install Starling:**

```powershell
cargo install --git https://forgejo.hearthhome.lol/Saltfault/Starling.git
```

**5. Run:**

```powershell
starling open
```

Audio uses WASAPI (Windows Audio Session API) — works out of the box, no
extra audio packages needed.

### macOS

**1. Install Xcode Command Line Tools** (provides the C compiler):

```bash
xcode-select --install
```

**2. Install [Homebrew](https://brew.sh)** if you don't have it:

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

**3. Install CMake and pkg-config:**

```bash
brew install cmake pkg-config
```

**4. Install Rust:**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

**5. Install Starling:**

```bash
cargo install --git https://forgejo.hearthhome.lol/Saltfault/Starling.git
```

**6. Run:**

```bash
starling open
```

Audio uses CoreAudio — works out of the box, no extra audio packages needed.

### Linux (native)

**1. Install system dependencies:**

```bash
# Debian / Ubuntu
sudo apt install build-essential cmake pkg-config libasound2-dev libpulse-dev

# Fedora
sudo dnf install gcc cmake pkgconf-pkg-config alsa-lib-devel pulseaudio-libs-devel

# Arch Linux
sudo pacman -S base-devel cmake pkgconf alsa-lib pulseaudio
```

**2. Install Rust:**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

**3. Install Starling:**

```bash
cargo install --git https://forgejo.hearthhome.lol/Saltfault/Starling.git
```

**4. Run:**

```bash
starling open
```

Audio uses PulseAudio (with ALSA fallback) — works out of the box on most
Linux desktops.

| Package | Why it's needed |
|---------|----------------|
| `build-essential` / `base-devel` | C compiler (gcc) for native code |
| `cmake` | Building libopus from source |
| `pkg-config` | Locating ALSA and PulseAudio libraries at build time |
| `libasound2-dev` | ALSA headers — cpal compiles the ALSA backend on Linux |
| `libpulse-dev` | PulseAudio headers — cpal's preferred backend at runtime |

### WSL2 (Windows Subsystem for Linux)

WSL2 setup is identical to Linux, with one extra step for audio.

**1. Install WSL2** (if not already installed, from PowerShell):

```powershell
wsl --install
```

**2. Inside WSL, install system dependencies:**

```bash
sudo apt update
sudo apt install build-essential cmake pkg-config libasound2-dev libpulse-dev
```

**3. Install Rust:**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

**4. Install `just` and enable voice calls (one-time setup):**

```bash
cargo install just
just setup-wsl-audio
```

`just setup-wsl-audio` installs `libasound2-plugins` and writes
`/etc/asound.conf` to route ALSA through PulseAudio. This is needed because
the pure-Rust PulseAudio crate that cpal uses can't authenticate with WSLg's
server, but the C library (`libpulse`) that ALSA's pulse plugin uses can.

If you skip this step, text chat works but voice calls won't.

**5. Install Starling:**

```bash
cargo install --git https://forgejo.hearthhome.lol/Saltfault/Starling.git
```

**6. Run:**

```bash
starling open
```

If you're on an older Windows 10 build without WSLg, audio won't work in
WSL2 — use a [native Windows build](#windows) instead.

---

## Running Starling

### Start a new flock

```bash
starling open
```

The app starts and the header shows a room code with a color swatch:

```
▀▄ flock: BIRD00CCFF
```

Share this code with another bird so they can join your flock.

### Join an existing flock

```bash
starling join BIRD00CCFF
```

### Set your name

When you start Starling, a popup asks for your display name — the name
other birds see next to your messages in the flock. Type it and press
Enter to join the murmuration.

### Logs

Errors are written to `logs/latest.log`. On each launch, the previous log is
gzipped to `logs/<timestamp>.log.gz`. Check this file if something isn't
working.

---

## Keybindings

| Key | Action |
|-----|--------|
| `Enter` | Send typed message |
| `Ctrl+K` | Start call with selected peer / hang up |
| `Ctrl+M` | Toggle mute |
| `Tab` | Cycle selected peer |
| `Backspace` | Delete last character |
| `Esc` | Quit |

---

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

### Source layout

| File | Responsibility |
|------|---------------|
| `main.rs` | Event loop, keyboard handling, subcommand dispatch |
| `event.rs` | `Command` (UI→net) and `AppEvent` (net→UI) types |
| `net.rs` | Owns the iroh endpoint, gossip subscription, voice handler |
| `call.rs` | Opens/accepts QUIC streams for voice datagrams |
| `voice.rs` | Mic capture: cpal input → Opus encoder → channel |
| `playback.rs` | Audio output: channel → Opus decoder → ring buffer → cpal output |
| `ui.rs` | Terminal rendering and UI state (`App` struct) |
| `setup.rs` | Setup wizard TUI for profile configuration |
| `config.rs` | Profile struct, disk persistence, 32-digit code encode/decode |
| `crypto.rs` | E2E encryption (ChaCha20-Poly1305) for gossip messages |
| `logger.rs` | File logger with gzipped log rotation |
| `util.rs` | Platform utilities (stderr suppression on Unix) |

### How the murmuration works

Birds connect to the murmuration through iroh's global relay network and
node discovery. No central server coordinates them:

1. A bird opens a flock by generating a random room code (e.g.
   `BIRD00CCFF`) and subscribing to a gossip topic derived from it via
   SHA-256.
2. Other birds join by entering the same room code — they subscribe to
   the same gossip topic.
3. iroh's relay connects both peers on the topic automatically. No node
   IDs or addresses need to be exchanged.
4. Text messages broadcast over gossip reach all birds in the mesh.
5. Voice calls are direct peer-to-peer QUIC datagram streams — no relay
   needed if direct connectivity is available, with relay fallback.

Audio is encoded as 48 kHz mono Opus, 20 ms frames (960 samples per frame),
sent as QUIC datagrams. Playback uses a 2-second ring buffer to absorb
network jitter.

All text messages are end-to-end encrypted with ChaCha20-Poly1305 using a
key derived from the room code. Voice calls are E2E encrypted via iroh's
QUIC TLS 1.3. Relays and intermediaries cannot read message content.

---

## Troubleshooting

### `cmake not found`

Install CMake for your platform (see [Platform setup](#platform-setup)).

### `pkg-config failed — alsa development headers are not installed` (Linux)

```bash
sudo apt install libasound2-dev   # Debian/Ubuntu
sudo dnf install alsa-lib-devel   # Fedora
sudo pacman -S alsa-lib           # Arch
```

### No microphone / no audio output (WSL2)

Run the one-time audio setup:

```bash
just setup-wsl-audio
```

This installs `libasound2-plugins` and writes `/etc/asound.conf` to route
ALSA through PulseAudio. See [WSL2 setup](#wsl2-windows-subsystem-for-linux)
for details.

If it still doesn't work, verify PulseAudio is running:

```bash
ls /mnt/wslg/PulseServer   # should exist
echo $PULSE_SERVER          # should show unix:/mnt/wslg/PulseServer
```

If you don't have WSLg (older Windows 10), audio won't work in WSL2 —
use a [native Windows build](#windows) instead.

### `link.exe not found` (Windows)

You need the Visual Studio C++ Build Tools. Reinstall them and make sure
"Desktop development with C++" is selected.

### Build is slow on first compile

The Opus codec is compiled from source via CMake on the first build.
Subsequent builds are cached. Expect 2–5 minutes for the initial build.

### Check the logs

Errors are written to `logs/latest.log`. On each launch, the previous log is
gzipped to `logs/<timestamp>.log.gz`.

---

## License

MIT