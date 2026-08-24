#!/bin/sh
# 编译 chatd 服务器与客户端（平铺模块结构）
# 产物：chatd-server（NIO 聊天服务器）、chatd-client（交互式客户端）
set -e
cd "$(dirname "$0")/.."
DRIVER="../../../target/release/zeta-driver"
if [ ! -x "$DRIVER" ]; then
    echo "未找到编译器：$DRIVER（请先在仓库根 cargo build --release）" >&2
    exit 1
fi
"$DRIVER" build server_main.zeta -o chatd-server --force
"$DRIVER" build client_main.zeta -o chatd-client --force
echo "构建完成：chatd-server / chatd-client"
