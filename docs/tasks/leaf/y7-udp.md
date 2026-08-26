# Y7 UDP

> **所属阶段**：阶段 Y
> **状态**：✅ 已完成
> **依赖**：O1a
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`net/udp.rl`——`UdpSocket::bind`/`send_to`/`recv_from`/`local_addr`。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`net/udp.rl` 新建——`UdpPacket{data, from}`/`UdpSocket{fd, addr}`，`bind`（socket SOCK_DGRAM + bind + 端口 0 经 getsockname 读实际端口）、`local_addr`、`send_to`/`recv_from`（sendto/recvfrom extern，sockaddr 构造/解析复用 net::byteorder）、`set_nonblocking`/`is_nonblocking`（R2 fcntl 复用）；core.rl 增加 `extern fn sendto/recvfrom` + `import net::udp::UdpSocket/UdpPacket` 提升；WASI 短路。

## 验证

`udp_test.rs` 3 用例（自回环/双 socket 互发源端口/多包按序）+ run-pass `udp_echo.rl`（输出 9/rlyeh-udp/true）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
