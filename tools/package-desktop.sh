#!/usr/bin/env bash
# Build a release desktop binary and refresh the local dist package.
# The user launcher is intentionally not touched: tools/build-desktop.sh is the
# single place that syncs it (debug by default).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
mkdir -p "$TARGET_DIR"
CARGO_TARGET_DIR="$TARGET_DIR" cargo build --release -p cijian-desktop

BIN="$TARGET_DIR/release/cijian-desktop"
DIST="$ROOT/dist/cijian-desktop-linux-x86_64"
ICON_SVG="$ROOT/apps/desktop/assets/cijian.svg"
PACKAGE="$ROOT/dist/cijian-desktop-linux-x86_64.tar.gz"

if [[ ! -x "$BIN" ]]; then
    echo "release binary missing: $BIN" >&2
    exit 1
fi

rm -rf "$DIST"
mkdir -p "$DIST"
install -m 0755 "$BIN" "$DIST/cijian-desktop"
install -m 0644 "$ICON_SVG" "$DIST/cijian.svg"
cat > "$DIST/README.md" <<'README'
# 词间桌面包（Linux x86_64）

本地构建，未签名、未发布。要求系统提供 Wayland/X11、GL 与 SQLite 运行环境；
远程朗读还需配置兼容服务，本机朗读在 Linux 上使用 `espeak-ng`（可选）。

运行：

```sh
./cijian-desktop
# 或在隔离目录验收：
./cijian-desktop --data-dir /path/to/new-directory
```

学习数据默认写到 `$XDG_DATA_HOME/org.cijian.cijian`（由 `directories` 解析）。
本包没有安装器、自动升级、系统凭据迁移或移动端支持。备份通过应用内“导出学习备份”
完成，恢复会建立并打开一个新的学习库。没有真实 AI 调用或用户数据随包分发。
README
(
    cd "$DIST"
    sha256sum cijian-desktop > SHA256SUMS
)
tar -czf "$PACKAGE" -C "$ROOT/dist" cijian-desktop-linux-x86_64

echo "release:  $BIN"
echo "package:  $PACKAGE"
echo "launcher: unchanged (run tools/build-desktop.sh to sync it)"
