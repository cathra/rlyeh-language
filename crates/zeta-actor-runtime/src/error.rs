//! Actor 运行时错误类型。

use thiserror::Error;

use crate::ActorId;

/// Actor 运行时的统一错误类型。
#[derive(Debug, Error)]
pub enum ActorError {
    /// 邮箱已满（回压触发）。
    #[error("actor mailbox is full")]
    MailboxFull,

    /// 目标 Actor 已停止或不存在。
    #[error("actor has stopped")]
    ActorStopped,

    /// `ask` 回复类型与期望不符。
    #[error("wrong reply type")]
    WrongReplyType,

    /// Actor 处理消息时主动返回的错误（相当于 panic / 崩溃）。
    #[error("actor panicked: {reason}")]
    Panic {
        /// 崩溃原因。
        reason: String,
    },

    /// 重启频率超过上限，监督策略决定停止或升级。
    #[error("supervisor restart limit exceeded for actor {actor_id}")]
    RestartLimitExceeded {
        /// 触发超限的 Actor。
        actor_id: ActorId,
    },

    /// Actor 初始化（`ActorState::init`）失败。
    #[error("actor initialization failed: {reason}")]
    InitFailed {
        /// 初始化失败原因。
        reason: String,
    },

    /// 运行时已进入关闭流程，拒绝新消息。
    #[error("runtime is shutting down")]
    ShuttingDown,

    /// `ask` 等待回复超时（默认 [`crate::ASK_TIMEOUT`]）。
    #[error("ask timed out")]
    AskTimeout,
}
