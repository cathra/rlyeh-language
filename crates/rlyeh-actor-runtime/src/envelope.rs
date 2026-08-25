//! 消息信封：携带消息本体、可选的发送者与 `ask` 回复通道。

use std::any::Any;

use crossbeam_channel::Sender;

use crate::ActorId;

/// 投递到邮箱的消息信封。
///
/// - `message`：消息本体（类型擦除，接收方 downcast 还原）；
/// - `reply`：`ask` 模式下的回复通道，普通 `send` 为 `None`；
/// - `sender`：发送者 ID（保留字段，供未来监控/审计使用）。
pub(crate) struct Envelope {
    /// 消息发送者（`None` 表示外部 / 未知）。
    ///
    /// MVP 保留字段：用于未来监控 / 审计 / 消息溯源。
    #[allow(dead_code)]
    pub sender: Option<ActorId>,
    /// 消息本体。
    pub message: Box<dyn Any + Send>,
    /// `ask` 模式的回复通道。
    pub reply: Option<Sender<Box<dyn Any + Send>>>,
}

impl Envelope {
    /// 构造一个普通（无回复通道）的信封。
    pub fn new(sender: Option<ActorId>, message: Box<dyn Any + Send>) -> Self {
        Self {
            sender,
            message,
            reply: None,
        }
    }

    /// 构造一个带回复通道的信封（`ask` 模式）。
    pub fn with_reply(
        sender: Option<ActorId>,
        message: Box<dyn Any + Send>,
        reply: Sender<Box<dyn Any + Send>>,
    ) -> Self {
        Self {
            sender,
            message,
            reply: Some(reply),
        }
    }
}
