# Starling — build & run helpers
#
# Usage:
#   just install-deps   # one-time: install all system packages
#   just run            # check deps, then run the app
#   just build          # check deps, then build

# Install all system dependencies needed to build and run starling.
# Detects the distro and uses the appropriate package manager.
install-deps:
    @if command -v apt-get >/dev/null 2>&1; then \
        echo "Detected Debian/Ubuntu — installing..."; \
        sudo apt-get update && sudo apt-get install -y \
            build-essential cmake pkg-config libasound2-dev libpulse-dev; \
    elif command -v dnf >/dev/null 2>&1; then \
        echo "Detected Fedora — installing..."; \
        sudo dnf install -y \
            gcc cmake pkgconf-pkg-config alsa-lib-devel pulseaudio-libs-devel; \
    elif command -v pacman >/dev/null 2>&1; then \
        echo "Detected Arch — installing..."; \
        sudo pacman -S --noconfirm base-devel cmake pkgconf alsa-lib pulseaudio; \
    else \
        echo "Unsupported distro. Please install manually:"; \
        echo "  gcc, cmake, pkg-config, alsa-lib-dev, pulseaudio-dev"; \
        exit 1; \
    fi

# Check that all build prerequisites are present before running cargo.
# Prints clear messages for anything missing.
check-deps:
    #!/usr/bin/env bash
    missing=0
    for tool in cmake pkg-config cc; do
        if ! command -v "$tool" >/dev/null 2>&1; then
            echo "✗ '$tool' not found"
            missing=1
        fi
    done
    if ! pkg-config --exists alsa 2>/dev/null; then
        echo "✗ ALSA development headers not found (install libasound2-dev)"
        missing=1
    fi
    if ! pkg-config --exists libpulse 2>/dev/null; then
        echo "✗ PulseAudio development headers not found (install libpulse-dev)"
        missing=1
    fi
    if [ "$missing" -ne 0 ]; then
        echo ""
        echo "Run 'just install-deps' to install everything."
        exit 1
    fi
    echo "✓ All build dependencies present"

# Build the project (checks deps first).
build: check-deps
    cargo build

# Run the app in "open" mode — starts a new chat session.
run: check-deps
    cargo run -- open

# Run the app in "join" mode — joins an existing session via ticket.
join ticket: check-deps
    cargo run -- join {{ticket}}

# Run cargo check (fast, no binary output).
check: check-deps
    cargo check