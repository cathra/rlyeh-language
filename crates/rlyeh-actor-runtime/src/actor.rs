//! Actor 抽象：状态 protocol、运行上下文与状态机。

use std::any::Any;
use std::fmt;

use crossbeam_channel::Sender;

use crate::error::ActorError;
use crate::runtime::RuntimeHandle;
use crate::supervisor::RestartStrategy;
use crate::ActorRef;

/// 唯一标识一个 Actor 实例。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActorId(uuid::Uuid);

impl ActorId {
    /// 生成新的随机 Actor ID（UUID v4）。
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for ActorId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Actor 生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorStatus {
    /// 空闲，等待消息。
    Idle,
    /// 正在处理消息。
    Processing,
    /// 已请求停止，处理完当前消息后退出。
    Stopping,
    /// 已停止。
    Stopped,
    /// 处理消息时崩溃。
    Crashed,
}

impl ActorStatus {
    /// 编码为 `u8`（供原子存储）。
    pub(crate) fn as_u8(self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Processing => 1,
            Self::Stopping => 2,
            Self::Stopped => 3,
            Self::Crashed => 4,
        }
    }

    /// 从 `u8` 解码（与 [`as_u8`](Self::as_u8) 对应）。
    pub(crate) fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Idle,
            1 => Self::Processing,
            2 => Self::Stopping,
            3 => Self::Stopped,
            _ => Self::Crashed,
        }
    }
}

/// Actor 的私有状态。
///
/// 每个 Actor 实例持有独立的 `ActorState` 对象，由运行时在单个 Worker
/// 上串行访问，因此无需内部加锁。
pub trait ActorState: Any + Send + Sync + 'static {
    /// Actor 创建后的初始化钩子（注册前调用）。
    fn init(&mut self) -> Result<(), ActorError> {
        Ok(())
    }

    /// 处理一条消息。
    ///
    /// 返回 `true` 表示处理完本条后请求停止，`false` 表示继续运行。
    /// 返回 `Err` 视为崩溃，交由 Supervisor 决策。
    fn handle_message(
        &mut self,
        msg: Box<dyn Any + Send>,
        ctx: &mut ActorContext,
    ) -> Result<bool, ActorError>;

    /// Actor 停止前的清理钩子。
    fn on_stop(&mut self) -> Result<(), ActorError> {
        Ok(())
    }
}

/// 消息处理期间可用的运行上下文。
///
/// 提供向其他 Actor 发消息、创建子 Actor、回复 `ask` 请求、请求停止等能力。
pub struct ActorContext {
    /// 当前 Actor 的 ID。
    pub self_id: ActorId,
    /// 运行时句柄。
    pub(crate) runtime: std::sync::Arc<RuntimeHandle>,
    /// 当前消息的回复通道（`ask` 模式下存在）。
    reply_to: Option<Sender<Box<dyn Any + Send>>>,
}

impl ActorContext {
    /// 构造上下文（仅运行时内部调用）。
    pub(crate) fn new(
        self_id: ActorId,
        runtime: std::sync::Arc<RuntimeHandle>,
        reply_to: Option<Sender<Box<dyn Any + Send>>>,
    ) -> Self {
        Self {
            self_id,
            runtime,
            reply_to,
        }
    }

    /// 向另一个 Actor 发送消息（fire-and-forget）。
    pub fn send(&self, target: ActorId, msg: Box<dyn Any + Send>) -> Result<(), ActorError> {
        self.runtime.send(target, msg)
    }

    /// 创建子 Actor。
    pub fn spawn(&self, state: Box<dyn ActorState>) -> Result<ActorId, ActorError> {
        self.runtime.spawn(state)
    }

    /// 创建受监督的子 Actor（崩溃时按策略重启）。
    ///
    /// `factory` 用于崩溃后重建状态对象。
    pub fn spawn_supervised<F>(
        &self,
        factory: F,
        strategy: RestartStrategy,
    ) -> Result<ActorId, ActorError>
    where
        F: Fn() -> Box<dyn ActorState> + Send + Sync + 'static,
    {
        self.runtime.spawn_supervised(factory, strategy)
    }

    /// 请求当前 Actor 停止（处理完当前消息后退出）。
    pub fn stop(&self) {
        self.runtime.request_stop(self.self_id);
    }

    /// 回复 `ask` 请求。若当前消息不是 `ask`（无回复通道）则静默忽略。
    pub fn reply<R: Any + Send>(&mut self, value: R) {
        if let Some(tx) = self.reply_to.take() {
            let _ = tx.send(Box::new(value));
        }
    }

    /// 以已装箱消息回复 `ask` 请求（用于原样转发消息的场景）。
    ///
    /// 与 [`reply`](Self::reply) 的区别：不会再次装箱。
    pub fn reply_boxed(&mut self, value: Box<dyn Any + Send>) {
        if let Some(tx) = self.reply_to.take() {
            let _ = tx.send(value);
        }
    }

    /// 获取当前 Actor 的引用句柄（可用于消息处理中给自己发消息）。
    pub fn self_ref(&self) -> ActorRef {
        self.runtime.create_ref(self.self_id)
    }
}
