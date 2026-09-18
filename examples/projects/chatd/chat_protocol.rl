// ===== 聊天协议：NICK / MSG / LIST / QUIT 行协议 =====
//
// 线格式（UTF-8 文本，\n 结尾）：
//   NICK <name>   设置昵称（注册）
//   MSG  <text>   广播一条消息
//   LIST          请求在线列表（服务器回 "> online (N): ..."）
//   QUIT          退出聊天
//
// 服务器/客户端两侧共用：encode 组装线，decode 还原消息。
// 注：模块名由 `protocol` 改为 `chat_protocol`（`protocol` 自 PC-0 起为语言保留字）。

pub enum Msg {
    Nick(String),
    Say(String),
    List,
    Quit,
}

// 消息 → 协议行（不含结尾换行）
pub fn encode(m: Msg) -> String {
    match m {
        Msg::Nick(n) => String::from("NICK ") + n,
        Msg::Say(t) => String::from("MSG ") + t,
        Msg::List => String::from("LIST"),
        Msg::Quit => String::from("QUIT"),
    }
}

// 协议行 → 消息（未知前缀按普通消息处理）
pub fn decode(line: String) -> Msg {
    if line[0..<5] == String::from("NICK ") {
        let rest = line[5..<line.len()];
        return Msg::Nick(rest);
    }
    if line[0..<4] == String::from("MSG ") {
        let rest = line[4..<line.len()];
        return Msg::Say(rest);
    }
    if line == String::from("LIST") {
        return Msg::List;
    }
    if line == String::from("QUIT") {
        return Msg::Quit;
    }
    Msg::Say(line)
}
