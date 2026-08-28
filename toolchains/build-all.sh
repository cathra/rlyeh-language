#!/usr/bin/env bash
#
# 构建全平台 Rlyeh toolchain 归档包。
#
# 在单一 macOS 主机上可构建的平台：
#   - macOS arm64（本机）
#   - macOS x86_64（交叉，需 x86_64-apple-darwin target）
#   - Windows x86_64（交叉 MinGW，需 x86_64-pc-windows-gnu target + mingw 链接器）
#   - Linux x86_64/arm64（交叉，需 zig 或 musl-cross 工具链）
#
# 产物：toolchains/dist/rlyeh-toolchain-<platform>.tar.gz（含 bin + std + skills + examples）
#
# 用法:
#   ./build-all.sh                       # 构建本机可构建的全部平台
#   ./build-all.sh --platform macos-arm64  # 仅构建指定平台
#   ./build-all.sh --no-linux             # 跳过 Linux（无 zig 工具链时）
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/.." && pwd)"
DIST="$REPO/toolchains/dist"
VERSION="$(grep -m1 '^version = "' "$REPO/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"

# Windows GNU 链接器（MinGW）
MINGW_LINKER="x86_64-w64-mingw32-gcc"
# Linux 交叉链接器：rust-lld（Rust 自带，比 zig 更兼容 Rust 的 musl `-static-pie`/
# `-Bstatic` 链接参数；2026-08-28 实测 rust-lld 链接 Linux musl 成功，zig 0.13 失败）。
# rust-lld 路径：<rustup>/lib/rustlib/<host>/bin/rust-lld（自动探测）。
RUSTLLD="${RUSTLLD:-$(find "$HOME/.rustup/toolchains" -name rust-lld -path '*/bin/*' 2>/dev/null | head -1)}"

build_platform() {
    local name="$1" target="$2" ext="$3" linker="${4:-}"
    echo
    echo "==> 构建 $name ($target)"

    # 构建二进制（rust-lld/zig 链接器：export 后 cargo build；空 linker 直接构建）
    if [[ -n "$linker" ]]; then
        local linker_key="CARGO_TARGET_${target//-/_}_LINKER"
        local -x "$linker_key=$linker"
        cargo build --release --target "$target" --manifest-path "$REPO/Cargo.toml"
    else
        cargo build --release --target "$target" --manifest-path "$REPO/Cargo.toml"
    fi

    # 组装归档
    local out="$DIST/rlyeh-toolchain-$VERSION-$name.tar.gz"
    local stage
    stage="$(mktemp -d)"
    mkdir -p "$stage/bin"
    for b in rlyeh-driver rlyeh-fmt rlyeh-check rlyeh-doc rlyeh-bench dagon; do
        cp "$REPO/target/$target/release/$b$ext" "$stage/bin/$b$ext"
    done

    # 生成 rlyeh wrapper
    if [[ "$name" == *windows* ]]; then
        printf '@echo off\r\nset RLYEH_STD_PATH=%%~dp0..\\std\r\n"%%~dp0rlyeh-driver.exe" %%*\r\n' > "$stage/bin/rlyeh.bat"
    else
        printf '#!/usr/bin/env bash\nexport RLYEH_STD_PATH="${RLYEH_STD_PATH:-$(cd "$(dirname "$0")/.." && pwd)/std}"\nexec "$(dirname "$0")/rlyeh-driver" "$@"\n' > "$stage/bin/rlyeh"
        chmod +x "$stage/bin/rlyeh"
    fi

    # std + skills + examples
    cp -R "$REPO/crates/rlyeh-std/rlyeh" "$stage/std"
    mkdir -p "$stage/skills" "$stage/examples"
    [[ -d "$REPO/skills/rlyeh-language" ]] && cp -R "$REPO/skills/rlyeh-language" "$stage/skills/"
    [[ -d "$REPO/examples/std-demos" ]] && cp -R "$REPO/examples/std-demos" "$stage/examples/"

    tar -C "$stage" -czf "$out" bin std skills examples
    rm -rf "$stage"
    echo "    归档: $out ($(du -h "$out" | cut -f1))"
}

mkdir -p "$DIST"

# macOS arm64（本机）
build_platform "macos-arm64" "aarch64-apple-darwin" ""

# macOS x86_64（交叉）
if rustup target list --installed | grep -q x86_64-apple-darwin; then
    build_platform "macos-x86_64" "x86_64-apple-darwin" ""
else
    echo "!! 跳过 macOS-x86_64（缺 target x86_64-apple-darwin，rustup target add x86_64-apple-darwin）"
fi

# Windows x86_64（交叉 MinGW）
if rustup target list --installed | grep -q x86_64-pc-windows-gnu && command -v "$MINGW_LINKER" >/dev/null; then
    build_platform "windows-x86_64" "x86_64-pc-windows-gnu" ".exe" "$MINGW_LINKER"
else
    echo "!! 跳过 windows-x86_64（缺 target x86_64-pc-windows-gnu 或链接器 $MINGW_LINKER）"
fi

# Windows arm64：本机不可行（2026-08-28）——缺 Windows ARM64 import 库
# （kernel32.lib 等，需 aarch64-w64-windows-gnu MinGW/LLVM sysroot），rust-lld 报
# `unable to find library -lkernel32`。Windows arm64 包由 CI（Windows ARM64 runner
# 或带 sysroot 的交叉）构建。
echo "!! 跳过 windows-arm64（本机缺 Windows ARM64 import 库/工具链；由 CI 构建）"

# Linux x86_64 / arm64（交叉，rust-lld 链接器）
# 2026-08-28：rust-lld 链接 Linux musl 成功（zig 0.13 不兼容 `-static-pie`/`-Bstatic`）。
if [[ -n "$RUSTLLD" ]]; then
    # 确保 musl target 已装
    for t in x86_64-unknown-linux-musl aarch64-unknown-linux-musl riscv64gc-unknown-linux-musl; do
        if rustup target list --installed | grep -q "$t" || rustup target add "$t" >/dev/null 2>&1; then
            :
        fi
    done
    if rustup target list --installed | grep -q x86_64-unknown-linux-musl; then
        build_platform "linux-x86_64" "x86_64-unknown-linux-musl" "" "$RUSTLLD"
    fi
    if rustup target list --installed | grep -q aarch64-unknown-linux-musl; then
        build_platform "linux-arm64" "aarch64-unknown-linux-musl" "" "$RUSTLLD"
    fi
    # RISC-V64（riscv64gc-unknown-linux-musl）：rust-lld 支持 RISC-V，self-contained
    # 提供 libc/crt；若 RISC-V 亦需外部 libgcc（与 LoongArch 类似），设 RISCV_LIBGCC_DIR。
    if rustup target list --installed | grep -q riscv64gc-unknown-linux-musl; then
        if [[ -n "${RISCV_LIBGCC_DIR:-}" ]]; then
            RUSTFLAGS="-C link-self-contained=yes -C link-arg=-L$RISCV_LIBGCC_DIR" \
                build_platform "linux-riscv64" "riscv64gc-unknown-linux-musl" "" "$RUSTLLD"
        else
            build_platform "linux-riscv64" "riscv64gc-unknown-linux-musl" "" "$RUSTLLD"
        fi
    fi
    # LoongArch64（龙芯）：需要 LoongArch64 的 libgcc_s（Loongson 官方交叉工具链提供）。
    # 本机缺 libgcc_s（2026-08-28 实测 rust-lld 报 `unable to find library -lgcc_s`，
    # musl.cc 无 loongarch64 工具链、Apple clang 不支持 loongarch），故默认跳过；
    # 提供 libgcc_s 后（设 LOONGARCH_LIBGCC_DIR 指向含 libgcc_s.so 的目录）可构建。
    if rustup target list --installed | grep -q loongarch64-unknown-linux-musl; then
        if [[ -n "${LOONGARCH_LIBGCC_DIR:-}" ]]; then
            RUSTFLAGS="-C link-self-contained=yes -C link-arg=-L$LOONGARCH_LIBGCC_DIR" \
                build_platform "linux-loongarch64" "loongarch64-unknown-linux-musl" "" "$RUSTLLD"
        else
            echo "!! 跳过 linux-loongarch64（缺 LoongArch64 libgcc_s；设 LOONGARCH_LIBGCC_DIR 指向含 libgcc_s.so 的目录）"
        fi
    fi
else
    echo "!! 跳过 Linux（未找到 rust-lld）"
fi

echo
echo "==> 完成。归档位于 $DIST"
ls -la "$DIST"
