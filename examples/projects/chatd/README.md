# chatd — NIO 事件驱动 TCP 聊天室（Rlyeh）

纯 Rlyeh 实现的局域网聊天室：单线程事件循环服务器 + 交互式客户端，
全程不依赖任何外部框架，只用编译器内建 NIO（`poll(2)` + 非阻塞 TCP）。

## 特性

- **服务器**：`Poller`（poll(2)）+ 非阻塞 accept/读/写，单线程事件循环
- **协议**：`NICK` / `MSG` / `LIST` / `QUIT` 行协议（`chat_protocol.rl`）
- **会话**：`Hub` 双向映射（nick ↔ fd），昵称占用检测，在线列表
- **广播**：单条消息写遍所有连接（`MSG` 广播 / 上下线通知）
- **客户端**：与服务器同构的单线程 NIO——`poll` 同时监听键盘（fd 0）与 socket，
  收/发互不阻塞；支持 `/list`、`/quit`、Ctrl-D

## 构建

```sh
cd examples/projects/chatd
scripts/build.sh        # 产出 chatd-server / chatd-client
```

（需先在仓库根 `cargo build --release` 构建编译器。）

## 运行

```sh
# 终端 1：启动服务器（127.0.0.1:9888，Ctrl-C 终止）
./chatd-server

# 终端 2 / 3：开多个聊天客户端
./chatd-client
```

客户端交互：

| 输入 | 效果 |
|------|------|
| 任意文字 | 广播给聊天室所有人 |
| `/list` | 查看在线昵称列表 |
| `/quit` 或 Ctrl-D | 退出聊天 |

## 冒烟测试

```sh
scripts/smoke.sh   # 两客户端互发 + 昵称占用校验，全部通过输出"冒烟通过"
```

## 架构

```
chat_protocol.rl   Msg 枚举 + 行协议 encode/decode（两侧共用）
hub.rl        Hub：nick↔fd 双向映射、join/leave/count
server.rl     run_server：Poller 事件循环、accept、逐行处理、广播、断开清理
client.rl     run_client：Poller 监听 stdin(0)+socket，读键盘发协议行 / 收消息打印
server_main.rl 服务器入口（端口硬编码 9888，MVP 无命令行参数 API）
client_main.rl 客户端入口（地址/昵称硬编码，MVP 无命令行参数 API）
```

## Rlyeh 语言亮点（本项目用到）

- **NIO 内建**：`Poller::new/register/poll`、`Interest::Readable`、事件 `token` 分派
- **非阻塞 TCP**：`TcpListener::bind/accept/set_nonblocking`、`TcpStream::connect/fd/read_line/write`
- **泛型集合**：`HashMap<i64, TcpStream>`、`HashMap<String, i64>` + `keys()/values()/get/insert/remove`
- **引用参数**：`&mut Hub`、`&HashMap<i64, TcpStream>` 透传（无全局变量下的共享方式）
- **枚举 + match**：协议消息经 `use` 跨模块导入（`use chat_protocol::Msg`），模块内枚举经
  模块前缀定位（`fn encode(m: Msg)`）
- **`&mut` 结构体**：会话状态整体收容在 `Hub`，避免全局可变状态

## 已知 MVP 限制

- **无命令行参数 API**：地址/端口/昵称硬编码在入口文件，改后重编译
- **`console::read_line` 缓冲**：stdin 读取一次至多 256 字节并截取首行，
  管道批量输入时仅首行生效（交互式终端逐行输入不受影响）
- **客户端在线程边界不可共享连接**：MVP 无全局变量 + 线程函数零参数，
  客户端因此与服务器同构为单线程 NIO，而非独立收发双线程
