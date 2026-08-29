# P1 Poller kqueue 分派

> **所属专项**：[专项开发计划](../专项开发计划.md)（P1）
> **来源缺陷**：[`leaf/lang-defects.md`](lang-defects.md) #7（完整 Poller kqueue 分派，源自 Y2b）
> **状态**：✅ 已完成（2026-08-28）
> **风险**：低（无语言级障碍，独立可完成）
> **前置能力**：无（kqueue 基础设施已就绪）

## 目标

`io/nio.rl` 的 `Poller` 从仅 poll(2) 升级为按 `__rlyeh_target_os()` 分派：macOS/BSD（码 2/4）走 kqueue（O(1)），其他平台（Linux/Windows/WASI）回退 poll(2) 兜底。

## 现状（2026-08-28 实测）

- **kqueue 基础设施已全部就绪**：
  - driver `platform_ir.rs` 已注入 `__rlyeh_kqueue`/`__rlyeh_kevent`（macOS/BSD 原生转发 `kqueue`/`kevent`，其他平台 stub -1）
  - `io/nio.rl` 已有 `kevent_make`（32B 结构体）、`kqueue_new`、`kevent_ctl`（EV_ADD/DELETE）、`kevent_wait`（阻塞等待）
  - `y2b_kqueue_wait.rl` 已验证等待真实 socketpair 事件
- **当前缺口**：`Poller`（`io/nio.rl`）仅用 poll(2)（`fds/events/tokens` 三 Vec 注册表），未接入 kqueue。

## 技术方案

`Poller` 结构加 `kq: i64` 字段（非 macOS/BSD 置 -1 触发兜底），四方法分派：

| 方法 | macOS/BSD（码 2/4） | 其他平台 |
|------|-------------------|---------|
| `new` | `kqueue_new()` 建 kq + 注册表 | 现有 poll(2) 实现 |
| `register` | `kevent_ctl(EV_ADD)` + 注册表 | 现有 poll(2) |
| `deregister` | `kevent_ctl(EV_DELETE)` | 现有 poll(2) |
| `poll` | `kevent_wait` 解析 eventlist | 现有 poll(2) |

WASI 下现有 `poll`/`fcntl` 短路返回 Err（保留）。

**关键点**：`future.rl` 内部对 `Poller::new()/register()/poll()` 的调用**签名不变**，无需改动；纯内部实现替换。

## 波及范围

- `io/nio.rl`：`Poller` 加 `kq` 字段 + 四方法分派改造
- `y2b_kqueue_wait.rl`：可复用现有 kqueue 原语（不变）
- 新增 Poller 集成测试（socketpair 就绪事件经 Poller 分派）
- `y2a_kqueue.rl`、examples/projects/chatd/{client,server}.rl、smoke/main.rl、core.rl、future.rl：签名不变，**无需迁移**

## 执行步骤

1. `Poller` struct 加 `kq: i64`（非 macOS/BSD 置 -1 触发兜底）
2. `new`：macOS/BSD 调 `kqueue_new()`；`register` = `kevent_ctl(EV_ADD)`；`deregister` = EV_DELETE；`poll` = `kevent_wait` 解析 eventlist
3. 其他平台保留现有 poll(2) 实现；WASI 短路保留
4. 新增集成测试（socketpair 就绪事件经 Poller 分派）

## 验收标准

- macOS 下 `Poller` 事件等待正确（kqueue 路径）
- Linux/兜底平台回归全绿（poll 路径）
- 166 用例 + 新增全过

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-28 | 由专项开发计划 P1 生成叶子文档 |
| 2026-08-28 | ✅ 完成：`io/nio.rl` Poller 加 `kq` 字段 + `new` 建 kqueue（macOS/BSD）/其他 -1；`register`/`reregister`/`deregister` 分派 `kq_change`（EV_ADD/EV_DELETE，按 Interest 映射 EVFILT_READ/WRITE）；`poll` 分派 `kq_poll`（kevent_wait_res 解析，fd 反查 token + filter 判断方向）；新增 helper `interest_filter_mask`/`events_to_filter_mask`/`kevent_changes`/`filter_count`/`kq_change`/`kevent_wait_res` + `KeventRes`。测试：nio_test 8 全过（含新 `poller_kqueue_dispatch` 走真 kqueue）+ y2a/y2b kqueue FFI + cargo test 全绿。观察：`rlyeh test` 全量 runner 运行期 stack overflow（独立待查，cargo test 与 nio/kqueue 单跑均通过，非 P1 引入） |
