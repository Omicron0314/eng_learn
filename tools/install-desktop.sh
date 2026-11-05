#!/usr/bin/env bash
# Install an already-built desktop binary as the current user's launcher target.
#
# Usage:
#   tools/install-desktop.sh <path-to-cijian-desktop>
#
# The launcher entry is pointed at the build output with a symlink at
# ${XDG_BIN_HOME:-~/.local/bin}/cijian-desktop. Later builds at the same path are
# therefore picked up without rerunning this script. The XDG desktop entry and
# icon cache are refreshed at the same time. If cargo clean removes the binary
# or CARGO_TARGET_DIR changes, run tools/build-desktop.sh again.
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "用法：tools/install-desktop.sh <cijian-desktop 二进制>" >&2
    exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$(realpath "$1")"
if [[ ! -f "$BIN" || ! -x "$BIN" ]]; then
    echo "桌面二进制不存在或不可执行：$BIN" >&2
    exit 1
fi

BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
APP_DIR="$DATA_DIR/applications"
ICON_SCALABLE="$DATA_DIR/icons/hicolor/scalable/apps"
ICON_256="$DATA_DIR/icons/hicolor/256x256/apps"
ICON_SVG="$ROOT/apps/desktop/assets/cijian.svg"
ICON_PNG="$ROOT/apps/desktop/assets/cijian-256.png"

install -d "$BIN_DIR" "$APP_DIR" "$ICON_SCALABLE" "$ICON_256"

# Replace the launcher symlink atomically. The target path stays valid across
# later rebuilds; a running instance keeps its old inode.
STAGE_DIR="$(mktemp -d "$BIN_DIR/.cijian-desktop.XXXXXX")"
trap 'rm -rf "$STAGE_DIR"' EXIT
ln -s "$BIN" "$STAGE_DIR/cijian-desktop"
mv -f "$STAGE_DIR/cijian-desktop" "$BIN_DIR/cijian-desktop"
rmdir "$STAGE_DIR"
trap - EXIT

install -m 0644 "$ICON_SVG" "$ICON_SCALABLE/cijian.svg"
if [[ -f "$ICON_PNG" ]]; then
    install -m 0644 "$ICON_PNG" "$ICON_256/cijian.png"
fi

cat > "$APP_DIR/cijian.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=词间
GenericName=English Learning
Comment=本地英语学习：词库、复习、文章、阅读与统计
Exec=$BIN_DIR/cijian-desktop
Icon=cijian
Terminal=false
Categories=Education;Languages;
Keywords=english;vocabulary;flashcard;cijian;词间;英语;背单词;
StartupWMClass=cijian-desktop
DESKTOP

if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "$APP_DIR/cijian.desktop"
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$DATA_DIR/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "launcher: $APP_DIR/cijian.desktop"
echo "run:      $BIN_DIR/cijian-desktop"
