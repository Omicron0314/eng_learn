#!/usr/bin/env bash
# Build the desktop app and connect that build to the current user's launcher.
#
# Usage:
#   tools/build-desktop.sh            # debug build
#   tools/build-desktop.sh --release  # release build
#
# The debug build is the default because the launcher is a development entry.
# It ends up as a symlink to the build output, so later builds in the same
# CARGO_TARGET_DIR are picked up without rerunning this script. Use --release
# only for a delivery check; run this script again after cargo clean or when
# the target directory changes.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE=debug
CARGO_ARGS=()
if [[ $# -gt 0 ]]; then
    case "$1" in
        --release)
            MODE=release
            CARGO_ARGS=(--release)
            ;;
        --debug)
            MODE=debug
            CARGO_ARGS=()
            ;;
        -h | --help)
            sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "用法：tools/build-desktop.sh [--release]" >&2
            exit 2
            ;;
    esac
    shift
fi
if [[ $# -ne 0 ]]; then
    echo "用法：tools/build-desktop.sh [--release]" >&2
    exit 2
fi

TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/spikes/r00/target}"
if [[ "$TARGET_DIR" != /* ]]; then
    TARGET_DIR="$ROOT/$TARGET_DIR"
fi
mkdir -p "$TARGET_DIR"
CARGO_TARGET_DIR="$TARGET_DIR" cargo build "${CARGO_ARGS[@]}" -p cijian-desktop

BIN="$TARGET_DIR/$MODE/cijian-desktop"
if [[ ! -x "$BIN" ]]; then
    echo "构建产物不存在：$BIN" >&2
    exit 1
fi

"$ROOT/tools/install-desktop.sh" "$BIN"
echo "build:    $BIN"
