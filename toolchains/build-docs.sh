#!/usr/bin/env bash
# 打包 Rlyeh 文档归档（docs/ -> rlyeh-docs-<version>.tar.gz + SHA256）
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/.." && pwd)"
VERSION="$(grep -m1 '^version = "' "$REPO/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"
DIST="$REPO/toolchains/dist"
OUT="$DIST/rlyeh-docs-$VERSION.tar.gz"
mkdir -p "$DIST"
# 排除 macOS 资源分叉/编辑器缓存；-C 保证归档顶层为 docs/
COPYFILE_DISABLE=1 tar -C "$REPO" --exclude='.DS_Store' -czf "$OUT" docs
shasum -a 256 "$OUT" > "$DIST/rlyeh-docs-$VERSION-SHA256"
echo "文档归档: $OUT"
echo "校验和:   $DIST/rlyeh-docs-$VERSION-SHA256"
