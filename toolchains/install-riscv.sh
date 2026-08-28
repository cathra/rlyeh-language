#!/usr/bin/env bash
#
# 安装 RISC-V64 / LoongArch64 交叉编译工具链（zig，自带 LLVM/clang 后端）。
#
# 背景（2026-08-28）：rlyeh `--target` 交叉编译的汇编/链接经系统 clang（Apple clang
# 不支持 riscv64gc/loongarch64 target，报 `unknown target triple`）。zig 自带 LLVM，
# 其 `zig cc` 天然支持这些目标。本脚本：
#   1. 安装 zig（brew）
#   2. 创建 `zig-cc` 包装（`exec zig cc "$@"`）到 /usr/local/bin
#   3. 提示设置 `RLYEH_CLANG=zig-cc`（rlyeh-driver 汇编/链接改用 zig）
#
# 用法:
#   ./install-riscv.sh            # 完整安装（brew install zig + zig-cc wrapper）
#   ./install-riscv.sh --zig /path/to/zig   # 指定 zig 二进制路径（跳过 brew）
#
set -euo pipefail

ZIG="${ZIG:-$(command -v zig || true)}"

echo "==> RISC-V64 / LoongArch64 交叉工具链安装（zig）"

# 1. 安装/定位 zig
if [[ -z "$ZIG" ]]; then
    echo "==> 未找到 zig，经 brew 安装（brew install zig）..."
    command -v brew >/dev/null 2>&1 || { echo "!! 缺 brew：请先安装 Homebrew (https://brew.sh)"; exit 1; }
    brew install zig
    ZIG="$(command -v zig)"
fi
echo "    zig: $ZIG ($("$ZIG" version))"

# 2. 创建 zig-cc 包装（/usr/local/bin 可写则装那里，否则 ~/.local/bin）
# 关键：转换 target 格式——rlyeh 传 Rust 格式（riscv64gc-unknown-linux-musl），
# zig/LLVM 需 gcc 格式（riscv64-linux-musl）。
WRAP_DIR="/usr/local/bin"
if [[ ! -w "$WRAP_DIR" ]]; then
    WRAP_DIR="$HOME/.local/bin"
    mkdir -p "$WRAP_DIR"
fi
WRAP="$WRAP_DIR/zig-cc"
cat > "$WRAP" <<EOF
#!/usr/bin/env bash
# zig-cc wrapper：Rust target 格式（riscv64gc-unknown-linux-musl）→ zig gcc 格式
# （riscv64-linux-musl），再调 zig cc。
args=()
for a in "\$@"; do
  case "\$a" in
    --target=riscv64gc-unknown-linux-musl) args+=("--target=riscv64-linux-musl") ;;
    --target=loongarch64-unknown-linux-musl) args+=("--target=loongarch64-linux-musl") ;;
    --target=riscv64*) args+=("--target=riscv64-linux-musl") ;;
    --target=loongarch64*) args+=("--target=loongarch64-linux-musl") ;;
    *) args+=("\$a") ;;
  esac
done
exec "$ZIG" cc "\${args[@]}"
EOF
chmod +x "$WRAP"
echo "    zig-cc wrapper -> $WRAP"

# 3. 提示
ZIG_CC="$WRAP"
if ! command -v zig-cc >/dev/null 2>&1; then
    ZIG_CC="$WRAP"
fi
cat <<EOF

==> 完成。交叉编译 RISC-V64 / LoongArch64：
    export RLYEH_CLANG="$ZIG_CC"   # rlyeh-driver 汇编/链接改用 zig cc

    rlyeh build app.rl --target riscv64gc-unknown-linux-musl -o app-riscv
    rlyeh build app.rl --target loongarch64-unknown-linux-musl -o app-loong

说明：
- assemble_cross_elf（lib.rs）已用 rust-lld + Rust musl sysroot crt/libc 链接，
  只需 zig 提供目标架构的汇编（clang --target 阶段）。
- 需 rustup target add riscv64gc-unknown-linux-musl loongarch64-unknown-linux-musl。
EOF
