// future/interface.rl：`Future` protocol —— 2026-09-18 由 future/module.rl 拆出。
//
// 归属子模块 `future::interface`。对外 `future::Future` 由 future/module.rl 的
// `pub import protocol::Future;` 保持；各实现处写 `impl X: Future` 时经 typecheck
// `resolve_trait_key` 的 `::Future` 后缀兜底解析到本协议。

// S1a/W1：Future trait——`poll` 推进状态机，返回 `Ready(值)` 或 `Pending`。
// 关联类型 `type Output` 声明输出类型（U2 ✅）；`&mut self` 聚合指针传递。
protocol Future {
    type Output;
    fn poll(&mut self, cx: &mut future::poll::Context) -> future::poll::Poll<Self::Output>;
}
