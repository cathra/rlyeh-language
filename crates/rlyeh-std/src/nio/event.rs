//! NIO 事件类型：关注标志与就绪事件。
//!
//! 对应 Zeta 标准库 `std::nio` 中的 `Interest` 与 `Event` 类型。

/// 事件关注标志（readiness interest）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Interest {
    /// 只关注可读事件。
    Readable,
    /// 只关注可写事件。
    Writable,
    /// 同时关注可读与可写事件。
    ReadableWritable,
}

impl Interest {
    /// 是否包含可读关注。
    pub fn is_readable(self) -> bool {
        matches!(self, Self::Readable | Self::ReadableWritable)
    }

    /// 是否包含可写关注。
    pub fn is_writable(self) -> bool {
        matches!(self, Self::Writable | Self::ReadableWritable)
    }
}

/// 一个就绪事件：`token` 用于在应用中定位对应的 fd。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Event {
    /// 注册时关联的应用侧 token。
    pub token: u64,
    /// 该事件触发的关注标志。
    pub interest: Interest,
}

impl Event {
    /// 构造一个就绪事件。
    pub fn new(token: u64, interest: Interest) -> Self {
        Self { token, interest }
    }

    /// 是否可读。
    pub fn is_readable(&self) -> bool {
        self.interest.is_readable()
    }

    /// 是否可写。
    pub fn is_writable(&self) -> bool {
        self.interest.is_writable()
    }
}
