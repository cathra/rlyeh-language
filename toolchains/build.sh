#!/usr/bin/env bash
#
# Rlyeh toolchain 一键构建脚本
#
# 流程: 环境检查 → release 构建 → 测试 → 本地发布 → 冒烟验证 → 归档打包
#
# 用法:
#   ./build.sh                     # 完整流程（构建 + 测试 + 发布 + 归档）
#   ./build.sh --no-test           # 跳过测试
#   ./build.sh --no-install        # 跳过本地发布（仅构建 + 归档）
#   ./build.sh --no-tar            # 跳过归档打包
#   ./build.sh --prefix /opt/rlyeh  # 自定义安装前缀（默认 $HOME/.rlyeh）
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/.." && pwd)"

RUN_TEST=1
RUN_INSTALL=1
RUN_TAR=1
PREFIX="${RLYEH_PREFIX:-$HOME/.rlyeh}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-test)    RUN_TEST=0; shift ;;
        --no-install) RUN_INSTALL=0; shift ;;
        --no-tar)     RUN_TAR=0; shift ;;
        --prefix)     PREFIX="$2"; shift 2 ;;
        --prefix=*)   PREFIX="${1#*=}"; shift ;;
        -h|--help)
            echo "用法: ./build.sh [--no-test] [--no-install] [--no-tar] [--prefix <path>]"
            exit 0 ;;
        *) echo "未知选项: $1"; exit 2 ;;
    esac
done

VERSION="$(grep -m1 '^version = "' "$REPO/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

echo "==> Rlyeh toolchain 构建 ($VERSION, $OS-$ARCH)"
echo "    仓库: $REPO"

# 0. 环境检查
for c in cargo rustc clang git; do
    command -v "$c" >/dev/null 2>&1 || { echo "!! 缺少依赖: $c"; exit 1; }
done

# 1. release 构建
echo
echo "==> [1/5] cargo build --release"
cargo build --release --manifest-path "$REPO/Cargo.toml"

# 2. 测试
if [[ "$RUN_TEST" == 1 ]]; then
    echo
    echo "==> [2/5] cargo test --workspace"
    cargo test --workspace --manifest-path "$REPO/Cargo.toml" 2>&1 | tail -3
fi

# 3. 本地发布
if [[ "$RUN_INSTALL" == 1 ]]; then
    echo
    echo "==> [3/5] 本地发布（install.sh）"
    RLYEH_PREFIX="$PREFIX" bash "$SCRIPT_DIR/install.sh"
fi

# 4. 冒烟验证（依赖第 3 步的产物）
if [[ "$RUN_INSTALL" == 1 ]]; then
    echo
    echo "==> [4/5] 冒烟验证"
    "$PREFIX/bin/rlyeh" --version
    TMP="$(mktemp -d)"
    printf 'fn main() {\n    println("rebuild ok");\n}\n' > "$TMP/smoke.rl"
    "$PREFIX/bin/rlyeh" run "$TMP/smoke.rl"
    rm -rf "$TMP"
else
    echo
    echo "==> [4/5] 冒烟验证（跳过：--no-install）"
fi

# 5. 归档打包（可重定位 tar 包：bin + std + skills）
if [[ "$RUN_TAR" == 1 ]]; then
    DIST="$REPO/toolchains/dist"
    mkdir -p "$DIST"
    TARBALL="$DIST/rlyeh-toolchain-$VERSION-$OS-$ARCH.tar.gz"
    echo
    echo "==> [5/5] 归档打包"
    tar -C "$PREFIX" -czf "$TARBALL" bin std skills examples
    echo "    归档: $TARBALL ($(du -h "$TARBALL" | cut -f1))"
fi

echo
echo "==> 完成。toolchain $VERSION 已构建"
