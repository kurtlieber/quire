#!/bin/sh
# Installs Typo for the current user from this folder: the binary, its icon and its
# launcher entry. Run it from the folder you unpacked.
set -e
cd "$(dirname "$0")"
install -Dm755 typo "$HOME/.local/bin/typo"
install -Dm644 icons/typo.svg "$HOME/.local/share/icons/hicolor/scalable/apps/typo.svg"
install -Dm644 icons/typo-16.svg "$HOME/.local/share/icons/hicolor/16x16/apps/typo.svg"
install -Dm644 typo.desktop "$HOME/.local/share/applications/typo.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$HOME/.local/share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$HOME/.local/share/icons/hicolor" || true
echo "Installed. Run typo, or open Typo from the app launcher."
echo "If typo is not found, add ~/.local/bin to your PATH."
