#!/bin/sh
# 编译 wordfreq 词频统计器
# 产物：wordfreq（读取 ./sample.txt → 终端排行 + 写 ./freq.txt）
set -e
cd "$(dirname "$0")/.."
DRIVER="../../../target/release/rlyeh-driver"
if [ ! -x "$DRIVER" ]; then
    echo "未找到编译器：$DRIVER（请先在仓库根 cargo build --release）" >&2
    exit 1
fi
"$DRIVER" build main.rl -o wordfreq --force
echo "构建完成：wordfreq"
