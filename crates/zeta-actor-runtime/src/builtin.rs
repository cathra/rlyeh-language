//! 内置 Actor：Router（消息路由）与 Timer（周期回调）。

use std::any::Any;
use std::collections::HashMap;
use std::time::Duration;

use crate::actor::{ActorContext, ActorState};
use crate::error::ActorError;
use crate::ActorId;

/// Router 的路由消息：按 `key` 将 `payload` 转发给对应目标。
#[derive(Debug)]
pub struct RouterMsg {
    /// 路由键。
    pub key: String,
    /// 转发的消息负载。
    pub payload: Box<dyn Any + Send>,
}

/// 按键路由的消息路由器。
///
/// 维护 `key → ActorId` 映射，收到 [`RouterMsg`] 后转发给对应目标。
pub struct Router {
    routes: HashMap<String, ActorId>,
}

impl Router {
    /// 创建空路由器。
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    /// 注册路由：`key` 的消息转发到 `target`。
    pub fn add_route(&mut self, key: impl Into<String>, target: ActorId) {
        self.routes.insert(key.into(), target);
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl ActorState for Router {
    fn handle_message(
        &mut self,
        msg: Box<dyn Any + Send>,
        ctx: &mut ActorContext,
    ) -> Result<bool, ActorError> {
        if let Ok(route) = msg.downcast::<RouterMsg>() {
            let route = *route;
            if let Some(&target) = self.routes.get(&route.key) {
                let _ = ctx.send(target, route.payload);
            }
        }
        Ok(false)
    }
}

/// Timer 内部 tick 消息。
#[derive(Debug)]
pub struct TimerTick;

/// 周期性执行回调的 Timer Actor。
///
/// 由 [`crate::Runtime::spawn_timer`] 创建：后台线程按固定间隔向自身
/// 发送 [`TimerTick`]，回调在 Actor 的消息处理线程中执行。
pub struct Timer {
    /// 触发间隔。
    pub interval: Duration,
    /// 每次触发执行的回调。
    pub callback: Box<dyn Fn() + Send + Sync>,
}

impl ActorState for Timer {
    fn handle_message(
        &mut self,
        msg: Box<dyn Any + Send>,
        _ctx: &mut ActorContext,
    ) -> Result<bool, ActorError> {
        if msg.downcast_ref::<TimerTick>().is_some() {
            (self.callback)();
        }
        Ok(false)
    }
}
