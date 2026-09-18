// ===== 聊天客户端：单线程 NIO（Poller 同时监听键盘与网络）=====
//
// MVP 无全局变量 + 线程函数零参数 → 无法在子线程共享连接，
// 故与服务器同构采用单线程事件循环：poll 同时监听 fd 0（stdin）
// 与 socket，可读事件分派"读键盘 → 发协议行" / "收服务器消息 → 打印"。
//
// 命令：普通输入 = 广播消息；/list = 在线列表；/quit = 退出；Ctrl-D = 退出。

import chat_protocol::Msg;
import chat_protocol::encode;

// 运行聊天客户端（阻塞直到退出）
pub fn run_client(host: String, port: i64, nick: String) -> i64 {
    let addr = SocketAddr::new(host, port);
    let stream = match TcpStream::connect(addr) {
        Result::Ok(v) => v,
        Result::Err(e) => {
            println("connect failed");
            return 0
        }
    };
    let _ = stream.set_nonblocking(true);
    let sfd = stream.fd;

    let poller0 = match Poller::new() {
        Result::Ok(v) => v,
        Result::Err(e) => return 0,
    };
    let mut poller = poller0;
    let _ = poller.register(sfd, sfd, Interest::Readable);
    let _ = poller.register(0, 0, Interest::Readable);

    // 连接建立即注册昵称
    let nline = encode(Msg::Nick(nick)) + "\n";
    let _ = stream.write(nline);

    let mut running = 1;
    while running == 1 {
        let evs = poller.poll(100);
        match evs {
            Result::Err(e) => {}
            Result::Ok(events) => {
                let n = events.len();
                let mut i = 0;
                while i < n {
                    let ev = events[i];
                    let tok = ev.token;
                    if tok == 0 {
                        // stdin 可读：处理用户输入
                        let k = handle_stdin(&stream);
                        if k == 0 {
                            running = 0;
                        }
                    } else {
                        // socket 可读：处理服务器消息
                        let k = handle_socket(&stream);
                        if k == 0 {
                            running = 0;
                        }
                    }
                    i = i + 1;
                }
            }
        }
    }
    1
}

// 读一行用户输入并发送；返回 1 继续，0 退出
fn handle_stdin(stream: &TcpStream) -> i64 {
    let rl = read_line();
    match rl {
        Result::Err(e) => 1,
        Result::Ok(line) => {
            if line.len() == 0 {
                // Ctrl-D（EOF）：礼貌退出
                let _ = stream.write(encode(Msg::Quit) + "\n");
                return 0;
            }
            let q = String::from("/quit");
            let e2 = String::from("/exit");
            if line == q || line == e2 {
                let _ = stream.write(encode(Msg::Quit) + "\n");
                return 0;
            }
            let l = String::from("/list");
            let payload = if line == l {
                encode(Msg::List)
            } else {
                encode(Msg::Say(line))
            };
            let _ = stream.write(payload + "\n");
            1
        }
    }
}

// 读服务器一行并打印；返回 1 继续，0 已断开
fn handle_socket(stream: &TcpStream) -> i64 {
    let rl = stream.read_line();
    match rl {
        Result::Err(e) => {
            // 非阻塞下无数据（EAGAIN）忽略，其他错误断开
            let k = e.kind();
            let would = match k {
                IoErrorKind::WouldBlock => 1,
                _ => 0,
            };
            if would == 1 {
                return 1;
            }
            0
        }
        Result::Ok(line) => {
            if line.len() == 0 {
                println("> disconnected");
                return 0;
            }
            println(line);
            1
        }
    }
}
