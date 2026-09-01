#!/usr/bin/env bash
#
# Rlyeh 工具链本地发布脚本
#
# 将编译好的 Rlyeh 工具链（编译器 rlyeh 命令 + fmt/check/doc/bench + dagon 包管理器
# + 标准库 + rlyeh-language 技能）发布到本地目录，默认 $HOME/.rl
# （与本地 dagon 注册表 ~/.rl/registry 同根）。可由 build.sh 调用，也可单独运行。
#
# 用法:
#   ./install.sh                      # 发布到默认位置 ~/.rl
#   RLYEH_PREFIX=/opt/rlyeh ./install.sh  # 自定义前缀
#
# 安装布局:
#   <prefix>/
#   ├── bin/
#   │   ├── rlyeh              # 入口命令（可重定位 wrapper：自动注入 RLYEH_STD_PATH）
#   │   ├── rlyeh              # 真实编译器二进制（旧名 rlyeh-driver，已由 [[bin]] 改名）
#   │   ├── rlyeh-fmt|check|doc|bench   # 独立工具
#   │   └── dagon               # 包管理器
#   ├── std/                  # 标准库源码（core.rl + time/io/net/... 模块）
#   ├── skills/               # rlyeh-language 技能（SKILL.md + references/）
#   ├── examples/             # std-demos 用例项目（纯代码，不编译）
#   └── registry/             # 本地 dagon 注册表（publish 目标，自动创建）
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/.." && pwd)"
PREFIX="${RLYEH_PREFIX:-$HOME/.rl}"
RELEASE="${RLYEH_RELEASE_DIR:-$REPO/target/release}"

BINARIES=(rlyeh rlyeh-fmt rlyeh-check rlyeh-doc rlyeh-bench dagon)
STD_SRC="$REPO/crates/rlyeh-std/rlyeh"

echo "==> Rlyeh 工具链本地发布"
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
mkdir -p "$PREFIX/bin" "$PREFIX/std" "$PREFIX/skills" "$PREFIX/registry"

# 3. 复制二进制
for b in "${BINARIES[@]}"; do
    cp -f "$RELEASE/$b" "$PREFIX/bin/$b"
    chmod +x "$PREFIX/bin/$b"
done
echo "    二进制 -> $PREFIX/bin"

# 3b. 真实编译器二进制以 rlyeh-bin 部署，避免与入口 wrapper（同名 rlyeh）冲突被覆盖。
#     [[bin]] 已将编译器命名为 rlyeh，故此处把 ELF 改名为 rlyeh-bin，由 wrapper 转调。
mv -f "$PREFIX/bin/rlyeh" "$PREFIX/bin/rlyeh-bin"

# 4. 复制标准库（core.rl / future.rl + 全部模块子目录）
rm -rf "$PREFIX/std"
mkdir -p "$PREFIX/std"
cp -R "$STD_SRC"/. "$PREFIX/std/"
echo "    标准库 -> $PREFIX/std"

# 5. 复制 rlyeh-language 技能（SKILL.md + references/，随工具链发布）
#    优先项目根 skills/（仓库一级内容），回退 .codebuddy/skills/（IDE 加载副本）
SKILL_SRC="$REPO/skills/rlyeh-language"
if [[ ! -d "$SKILL_SRC" ]]; then
    SKILL_SRC="$REPO/.codebuddy/skills/rlyeh-language"
fi
if [[ -d "$SKILL_SRC" ]]; then
    rm -rf "$PREFIX/skills/rlyeh-language"
    mkdir -p "$PREFIX/skills"
    cp -R "$SKILL_SRC" "$PREFIX/skills/"
    echo "    技能 -> $PREFIX/skills/rlyeh-language（来源: ${SKILL_SRC}）"
else
    echo "!! 未找到技能目录（${SKILL_SRC}），跳过"
fi

# 5b. 复制 std-demos 用例项目（纯代码，不编译）
DEMOS_SRC="$REPO/examples/std-demos"
if [[ -d "$DEMOS_SRC" ]]; then
    rm -rf "$PREFIX/examples/std-demos"
    mkdir -p "$PREFIX/examples"
    cp -R "$DEMOS_SRC" "$PREFIX/examples/"
    echo "    用例项目 -> $PREFIX/examples/std-demos"
else
    echo "!! 未找到用例项目目录（$DEMOS_SRC），跳过"
fi

# 6. 生成 rlyeh 入口命令
#    可重定位：bin/ 与 std/ 同级，归档解压到任意位置均可自洽。
#    用户环境变量 RLYEH_STD_PATH 优先。
cat > "$PREFIX/bin/rlyeh" <<EOF
#!/usr/bin/env bash
# Rlyeh 编译器入口（本地发布版，可重定位）
export RLYEH_STD_PATH="\${RLYEH_STD_PATH:-\$(cd "\$(dirname "\$0")/.." && pwd)/std}"
exec "\$(dirname "\$0")/rlyeh-bin" "\$@"
EOF
chmod +x "$PREFIX/bin/rlyeh"

# 7. PATH 提示
case ":$PATH:" in
    *":$PREFIX/bin:"*) : ;;
    *)
        echo
        echo "==> 将工具链加入 PATH（写入 ~/.zshrc / ~/.bashrc）:"
        echo "    export PATH=\"$PREFIX/bin:\$PATH\""
        ;;
esac

# 8. 冒烟验证
echo
echo "==> 验证"
"$PREFIX/bin/rlyeh" --version || true

TMP="$(mktemp -d)"
printf 'fn main() {\n    println("toolchain ok");\n}\n' > "$TMP/hello.rl"
if "$PREFIX/bin/rlyeh" run "$TMP/hello.rl" > /dev/null 2>&1; then
    echo "    编译运行: OK"
else
    echo "!! 冒烟失败（可先 export RLYEH_STD_PATH=$PREFIX/std 排查）"
fi
rm -rf "$TMP"

echo
echo "==> 完成。工具链已发布到 $PREFIX"
