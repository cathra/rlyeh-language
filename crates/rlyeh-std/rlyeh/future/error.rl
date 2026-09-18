// future/error.rl：`TimeoutError`（Future 超时错误）——2026-09-18 由 future/module.rl 拆出。
//
// 归属子模块 `future::error`。对外 `future::TimeoutError` 由 future/module.rl 的
// `pub import error::TimeoutError;` 保持。
//
// W4（2026-08-25，std-lib.md §12 错误体系扩展）：参照 `IoError` 模式——消息字符串 +
// `Error::message` 访问器。完整 Display trait 随 X4（Formatter 完整化）落地，MVP 用
// `message()`。

struct TimeoutError {
    message: String,
}

impl TimeoutError {
    // MVP：无载荷超时错误，消息固定；正式版可携带 `Duration` 等上下文。
    fn new() -> future::error::TimeoutError {
        TimeoutError {
            message: String::from("future timed out"),
        }
    }
    fn message(&self) -> String {
        self.message
    }
}

impl TimeoutError: Error {
    fn message(&self) -> String {
        self.message
    }
    // P7d-1（2026-08-29）：升级为 Option<&dyn Error>（真实错误链；P4 上转型已支持）
    fn source(&self) -> Option<&dyn Error> {
        Option::None
    }
}
