#!/bin/sh
# 启动 chatd 服务器（监听 127.0.0.1:9888，Ctrl-C 终止）
cd "$(dirname "$0")/.."
exec ./chatd-server
