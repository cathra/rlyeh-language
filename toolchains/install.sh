#!/usr/bin/env bash
#
# Zeta 工具链本地发布脚本
#
# 将编译好的 Zeta 工具链（编译器 zeta 命令 + fmt/check/doc/bench + zep 包管理器
# + 标准库）发布到本地目录，默认 $HOME/.zeta（与本地 zep 注册表 ~/.zeta/registry 同根）。
# 可由 build.sh 调用，也可单独运行。
#
# 用法:
#   ./install.sh                      # 发布到默认位置 ~/.zeta
#   ZETA_PREFIX=/opt/zeta ./install.sh  # 自定义前缀
#
# 安装布局:
#   <prefix>/
#   ├── bin/
#   │   ├── zeta              # 入口命令（可重定位 wrapper：自动注入 ZETA_STD_PATH）
#   │   ├── zeta-driver       # 真实编译器二进制
#   │   ├── zeta-fmt|check|doc|bench   # 独立工具
#   │   └── zep               # 包管理器
#   ├── std/                  # 标准库源码（core.zeta + time/io/net/... 模块）
#   └── registry/             # 本地 zep 注册表（publish 目标，自动创建）
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/.." && pwd)"
PREFIX="${ZETA_PREFIX:-$HOME/.zeta}"
RELEASE="$REPO/target/release"

BINARIES=(zeta-driver zeta-fmt zeta-check zeta-doc zeta-bench zep)
STD_SRC="$REPO/crates/zeta-std/zeta"

echo "==> Zeta 工具链本地发布"
echo "    仓库:   $REPO"
echo "    前缀:   $PREFIX"

# 1. 检查 release 产物
missing=()
for b in "${BINARIES[@]}"; do
    if [[ ! -x "$RELEASE/$b" ]]; then
        missing+=("$b")
    fi
done
if [[ ${#missing[@]} -gt 0 ]]; then
    echo "!! 缺少 release 产物: ${missing[*]}"
    echo "   请先执行: cargo build --release"
    exit 1
fi

# 2. 创建目录布局
mkdir -p "$PREFIX/bin" "$PREFIX/std" "$PREFIX/registry"

# 3. 复制二进制
for b in "${BINARIES[@]}"; do
    cp -f "$RELEASE/$b" "$PREFIX/bin/$b"
    chmod +x "$PREFIX/bin/$b"
done
echo "    二进制 -> $PREFIX/bin"

# 4. 复制标准库（core.zeta / future.zeta + 全部模块子目录）
rm -rf "$PREFIX/std"
mkdir -p "$PREFIX/std"
cp -R "$STD_SRC"/. "$PREFIX/std/"
echo "    标准库 -> $PREFIX/std"

# 5. 生成 zeta 入口命令
#    可重定位：bin/ 与 std/ 同级，归档解压到任意位置均可自洽。
#    用户环境变量 ZETA_STD_PATH 优先。
cat > "$PREFIX/bin/zeta" <<EOF
#!/usr/bin/env bash
# Zeta 编译器入口（本地发布版，可重定位）
export ZETA_STD_PATH="\${ZETA_STD_PATH:-\$(cd "\$(dirname "\$0")/.." && pwd)/std}"
exec "\$(dirname "\$0")/zeta-driver" "\$@"
EOF
chmod +x "$PREFIX/bin/zeta"

# 6. PATH 提示
case ":$PATH:" in
    *":$PREFIX/bin:"*) : ;;
    *)
        echo
        echo "==> 将工具链加入 PATH（写入 ~/.zshrc / ~/.bashrc）:"
        echo "    export PATH=\"$PREFIX/bin:\$PATH\""
        ;;
esac

# 7. 冒烟验证
echo
echo "==> 验证"
"$PREFIX/bin/zeta" --version || true

TMP="$(mktemp -d)"
printf 'fn main() {\n    println("toolchain ok");\n}\n' > "$TMP/hello.zeta"
if "$PREFIX/bin/zeta" run "$TMP/hello.zeta" > /dev/null 2>&1; then
    echo "    编译运行: OK"
else
    echo "!! 冒烟失败（可先 export ZETA_STD_PATH=$PREFIX/std 排查）"
fi
rm -rf "$TMP"

echo
echo "==> 完成。工具链已发布到 $PREFIX"
