//! # zeta-actor-runtime
//!
//! Zeta 语言 Actor 并发模型运行时（M2.1）。
//!
//! 提供基于消息传递的 Actor 运行时，核心能力：
//!
//! - **Actor 生命周期**：创建、注册、消息处理、优雅停止、`on_stop` 清理；
//! - **消息系统**：fire-and-forget `send` 与请求-响应 `ask`（含回复类型检查）；
//! - **工作窃取调度器**：多 Worker 并发，本地队列 + 全局队列 + 随机窃取 +
//!   空闲休眠 / 唤醒；
//! - **Supervisor**：`OneForOne` / `AllForOne` / `RestartForOne` 重启策略，
//!   滑动窗口频率限制与错误升级；
//! - **崩溃隔离**：单个 Actor 崩溃不影响其他 Actor，由监督层恢复；
//! - **内置 Actor**：Router（消息路由）与 Timer（周期回调）；
//! - **优雅关闭**：排空所有消息队列后再停止 Worker 与 Actor。
//!
//! ## 快速开始
//!
//! ```rust
//! use zeta_actor_runtime::{ActorRef, ActorState, ActorContext, ActorError, RuntimeBuilder};
//!
//! struct Counter { value: i64 }
//!
//! impl ActorState for Counter {
//!     fn handle_message(
//!         &mut self,
//!         msg: Box<dyn std::any::Any + Send>,
//!         ctx: &mut ActorContext,
//!     ) -> Result<bool, ActorError> {
//!         if let Some(&delta) = msg.downcast_ref::<i64>() {
//!             self.value += delta;
//!             // 若为 ask 请求则回复当前值
//!             ctx.reply(self.value);
//!         }
//!         Ok(false)
//!     }
//! }
//!
//! let runtime = RuntimeBuilder::new().with_workers(2).build();
//! let counter: ActorRef = runtime.spawn(Counter { value: 0 });
//! let _ = counter.send(Box::new(10i64));
//! let _ = counter.send(Box::new(5i64));
//! let value: i64 = counter.ask_blocking(Box::new(0i64)).unwrap();
//! assert_eq!(value, 15);
//! runtime.shutdown();
//! ```

#![warn(missing_docs)]
#![warn(unsafe_code)]

mod actor;
mod builtin;
mod envelope;
mod error;
mod runtime;
mod scheduler;
mod supervisor;

pub use actor::{ActorContext, ActorId, ActorState, ActorStatus};
pub use builtin::{Router, RouterMsg, Timer, TimerTick};
pub use error::ActorError;
pub use runtime::{ActorRef, Runtime, RuntimeBuilder, RuntimeHandle, MAILBOX_CAPACITY};
pub use supervisor::{RestartStrategy, Supervisor, SupervisorDecision, SupervisorStrategy};

use std::time::Duration;

/// `ask` 等待回复的默认超时（10 秒）。
pub const ASK_TIMEOUT: Duration = Duration::from_secs(10);
