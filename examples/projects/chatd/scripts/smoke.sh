#!/bin/sh
# 冒烟测试：启动服务器 → 两个客户端互发消息 → 校验广播 → 清理
set -e
cd "$(dirname "$0")/.."
if [ ! -x ./chatd-server ]; then
    echo "请先运行 scripts/build.sh" >&2
    exit 1
fi
./chatd-server >/tmp/chatd-smoke.log 2>&1 &
SRV=$!
sleep 1
# 两个客户端：alice 发送消息，bob 接收；第三个重复昵称验证占用
(printf 'NICK alice\nMSG ping from alice\n'; sleep 2) | nc -w 3 127.0.0.1 9888 >/tmp/chatd-a.log &
(printf 'NICK bob\n'; sleep 2) | nc -w 3 127.0.0.1 9888 >/tmp/chatd-b.log &
(printf 'NICK alice\nLIST\n'; sleep 2) | nc -w 3 127.0.0.1 9888 >/tmp/chatd-c.log &
sleep 4
kill "$SRV" 2>/dev/null || true
echo "=== alice ==="; cat /tmp/chatd-a.log
echo "=== bob ==="; cat /tmp/chatd-b.log
echo "=== dup-nick ==="; cat /tmp/chatd-c.log
# 校验：bob 收到 alice 的广播，重复昵称被拒
grep -q "ping from alice" /tmp/chatd-b.log || { echo "FAIL: bob 未收到广播"; exit 1; }
grep -q "nick taken" /tmp/chatd-c.log || { echo "FAIL: 昵称占用未拒绝"; exit 1; }
echo "冒烟通过"
