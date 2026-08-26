# L4 平台加固（WASI net + Actor 交叉编译/WASM）

> **所属阶段**：阶段 L
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

WASI net 禁用文档化 + Actor 交叉编译 / WASM 支持。

## 背景

阶段 阶段 L 子任务，详见 阶段详情文档 [`stages/L.md`](../../stages/L.md)。

## 技术细节

L4a net：`net.rl` 网络函数加 `if __rlyeh_target_os() == 5 { return …; }` WASI 短路（码 5 = WASI）。L4b Actor WASM：driver 从 HIR 收集 actor 符号注入静态表 `actor_resolve_ir`（替代 dlsym）+ `sync.rs` WASI 单线程同步运行时（Entry + Mutex<Vec> + AtomicU64）+ ABI 修正（state: u64、factory: *mut c_void）。关键教训：clang 拒绝 strdup declare、ABI 不匹配 wasm trap、Vec::resize E0599。

## 验证

`wasm_target_test.rs` wasm_actor_runtime_ready/runs/supervised_runs + 全量 cargo test。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
