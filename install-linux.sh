#!/usr/bin/env bash
# Builds Codelab and installs it for the current user:
# binary, desktop entry and icons (the compositor finds the icon via the app id "codelab").
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release

install -Dm755 target/release/codelab            "$HOME/.local/bin/codelab"
install -Dm644 assets/codelab.desktop            "$HOME/.local/share/applications/codelab.desktop"
install -Dm644 assets/icon.svg                   "$HOME/.local/share/icons/hicolor/scalable/apps/codelab.svg"
install -Dm644 assets/icon.png                   "$HOME/.local/share/icons/hicolor/512x512/apps/codelab.png"

gtk-update-icon-cache -qtf "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true

echo "Installed. Make sure ~/.local/bin is in your PATH, then run: codelab"
