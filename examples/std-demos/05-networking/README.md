# 05 · 网络：TCP

> 规范：docs/std-lib.md §5（网络模块）
> 来源：复用 tests/run-pass 已通过回归的用例

## 功能点

- `SocketAddr`：`new(ip, port)` / `parse("ip:port")` / `ip()` / `port()`（纯字符串解析，无网络）
- TCP：`TcpListener` / `TcpStream` 监听与连接（`bind` / `accept` / `connect` / 读写）

## 示例清单

| 文件 | 说明 |
|------|------|
| `tcp_addr.zeta` | SocketAddr 构造与解析（含非法输入回退） |
| `tcp_echo.zeta` | TCP echo 服务器：监听 → accept → 回显（常驻循环，需 Ctrl-C 退出） |

## 运行

```bash
zeta run examples/std-demos/05-networking/tcp_addr.zeta
# echo 服务器为常驻进程，前台运行观察，Ctrl-C 退出：
zeta run examples/std-demos/05-networking/tcp_echo.zeta
```

> HTTP 客户端（get / post）规划见 std-lib.md §5.2，MVP 阶段以 TCP 为基础能力。
