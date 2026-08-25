//! Supervisor：崩溃恢复与重启策略。

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::actor::ActorState;
use crate::error::ActorError;
use crate::ActorId;

/// 重启策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartStrategy {
    /// 只重启崩溃的 Actor。
    OneForOne,
    /// 一个崩溃，整个监督组全部重启。
    AllForOne,
    /// 重启崩溃者及其之后声明的兄弟（按创建顺序）。
    RestartForOne,
}

/// 监督策略配置（每个受监督 Actor 一份引用）。
#[derive(Clone)]
pub struct SupervisorStrategy {
    /// 重启策略。
    pub strategy: RestartStrategy,
    /// 窗口内允许的最大重启次数。
    pub max_restarts: usize,
    /// 频率限制窗口。
    pub within: Duration,
    /// 监督组内的全部子 Actor（按创建顺序）。
    pub children: Vec<ActorId>,
    /// 崩溃后重建状态对象的工厂。
    pub factory: Arc<dyn Fn() -> Box<dyn ActorState> + Send + Sync>,
}

impl SupervisorStrategy {
    /// 构造默认配置的监督策略。
    pub fn new(
        strategy: RestartStrategy,
        factory: Arc<dyn Fn() -> Box<dyn ActorState> + Send + Sync>,
    ) -> Self {
        Self {
            strategy,
            max_restarts: Self::DEFAULT_MAX_RESTARTS,
            within: Self::DEFAULT_WINDOW,
            children: Vec::new(),
            factory,
        }
    }

    /// 默认重启窗口内的最大次数。
    pub const DEFAULT_MAX_RESTARTS: usize = 10;
    /// 默认频率限制窗口。
    pub const DEFAULT_WINDOW: Duration = Duration::from_secs(30);
}

/// Supervisor 对一次崩溃的决策结果。
#[derive(Debug)]
pub enum SupervisorDecision {
    /// 重启指定的 Actor 列表。
    Restart(Vec<ActorId>),
    /// 停止崩溃的 Actor（不再重启）。
    Stop,
    /// 超出恢复能力，升级错误（由更上层决策）。
    Escalate(ActorError),
}

/// 重启频率计数器。
struct RestartCounter {
    count: usize,
    window_start: Instant,
}

/// Supervisor 管理器。
///
/// 以 `DashMap` 维护「子 Actor → 监督策略」映射，崩溃时依据策略
/// 与频率限制做出重启 / 停止 / 升级决策。
pub struct Supervisor {
    /// 子 Actor ID → 监督策略。
    strategies: DashMap<ActorId, SupervisorStrategy>,
    /// 子 Actor ID → 重启计数（窗口滑动）。
    restart_counts: DashMap<ActorId, RestartCounter>,
}

impl Supervisor {
    /// 创建空的 Supervisor 管理器。
    pub fn new() -> Self {
        Self {
            strategies: DashMap::new(),
            restart_counts: DashMap::new(),
        }
    }

    /// 注册监督策略（`child_id` 的崩溃将按 `strategy` 处理）。
    pub fn register(&self, child_id: ActorId, strategy: SupervisorStrategy) {
        self.strategies.insert(child_id, strategy);
    }

    /// 移除监督关系（Actor 正常停止时调用）。
    pub fn unregister(&self, child_id: ActorId) {
        self.strategies.remove(&child_id);
        self.restart_counts.remove(&child_id);
    }

    /// 清空全部监督状态（运行时关闭时调用）。
    pub fn clear(&self) {
        self.strategies.clear();
        self.restart_counts.clear();
    }

    /// 获取指定 Actor 的重建工厂。
    pub fn factory_for(&self, actor_id: ActorId) -> Option<Arc<dyn Fn() -> Box<dyn ActorState> + Send + Sync>> {
        self.strategies
            .get(&actor_id)
            .map(|s| s.factory.clone())
    }

    /// 处理一次 Actor 崩溃，返回监督决策。
    ///
    /// - 无监督信息 → `Escalate`（让更上层处理）；
    /// - 重启频率超限 → `Escalate(RestartLimitExceeded)`；
    /// - 否则按策略返回 `Restart` / `Stop`。
    pub fn handle_crash(&self, actor_id: ActorId, error: ActorError) -> SupervisorDecision {
        let Some(strategy) = self.strategies.get(&actor_id) else {
            return SupervisorDecision::Escalate(error);
        };

        // 滑动窗口频率限制
        let now = Instant::now();
        let mut counter = self
            .restart_counts
            .entry(actor_id)
            .or_insert(RestartCounter {
                count: 0,
                window_start: now,
            });
        if now.duration_since(counter.window_start) > strategy.within {
            counter.count = 0;
            counter.window_start = now;
        }
        counter.count += 1;
        if counter.count > strategy.max_restarts {
            return SupervisorDecision::Escalate(ActorError::RestartLimitExceeded {
                actor_id,
            });
        }
        drop(counter);

        match strategy.strategy {
            RestartStrategy::OneForOne => SupervisorDecision::Restart(vec![actor_id]),
            RestartStrategy::AllForOne => SupervisorDecision::Restart(strategy.children.clone()),
            RestartStrategy::RestartForOne => {
                let children = &strategy.children;
                let idx = children
                    .iter()
                    .position(|&c| c == actor_id)
                    .unwrap_or(children.len().saturating_sub(1));
                SupervisorDecision::Restart(children[idx..].to_vec())
            }
        }
    }
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}
