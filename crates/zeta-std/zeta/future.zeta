// ===== future 模块（S1/S2，2026-08）：异步运行时基础 =====
// S1a：`Poll` 枚举 + `Future` trait；S1b：`block_on` 手动轮询；
// S2c：`timeout` 带超时轮询。
// 实现说明（与 std-lib.md §10 规划 API 的 MVP 退化对照）：
// - 关联类型 `type Output`（trait 内 `type` 成员）、`Pin<&mut Self>`、`Context`
//   与泛型约束 `F: Future` 均为规划语法，MVP 不可用——`Future::poll` 的
//   Output 固定为 `i64`（std-lib.md 标注"不可行则退化"）。
// - `block_on` 采用泛型函数形态：`block_on<T>(f: &mut T)`，实例化时按具体
//   类型解析 `poll`（无 `F: Future` 约束检查，宽松语义）。
// - 手动轮询：`loop { match f.poll() { Ready(v) => return v, Pending => .. } }`。
// - S1c（async/await 状态机 desugar）与 Future 版 `join_all`（S2）后续实现。

// S1a：轮询结果（泛型枚举，与 Option 同构）。
enum Poll<T> {
    Ready(T),
    Pending,
}

// S1a：Future trait——`poll` 推进状态机，返回 `Ready(值)` 或 `Pending`。
// MVP 退化：Output 固定 i64（关联类型不支持）；`&mut self` 聚合指针传递。
trait Future {
    fn poll(&mut self) -> Poll<i64>;
}

// S1b：阻塞轮询 `f` 直到 `poll` 返回 `Ready`，返回其携带值。
// 调用方把状态机实现为 `Future` 类型后 `block_on(&mut fut)`；泛型实参
// 由实参类型推断（MVP 实例化时重查 body，`f.poll()` 解析到具体 impl）。
fn block_on<T>(f: &mut T) -> i64 {
    loop {
        match f.poll() {
            Poll::Ready(v) => return v,
            Poll::Pending => {}
        }
    }
}

// S2c：带超时阻塞轮询——`duration` 内未 `Ready` 返回 `Err`（超时）。
// MVP 退化（对齐 std-lib §10 规划 API 参数顺序，duration 在前）：
// - 返回 `Result<i64, i64>`——`TimeoutError` 类型规划中，超时统一返回
//   `Err(-1)`，成功返回 `Ok(值)`；
// - 泛型 `f: &mut T` 与 `block_on` 一致（实例化时按具体类型解析 poll，
//   无 `F: Future` 约束，宽松语义）；
// - 超时判定经墙钟 `__zeta_clock_monotonic`（S2b ✅ clock_gettime
//   MONOTONIC，与 `time::Instant` 相同的退化逻辑：返回 -1 退回 `clock()`
//   CPU 时钟——busy-wait 下仍有效）。注：MVP 静态方法调用不支持模块路径
//   前缀（`time::Instant::now` 不可用），故直接复用 extern；
// - 忙等轮询（与 block_on 一致，无让步；事件驱动等待规划随 R1 Poller）。
fn timeout<T>(duration: time::Duration, f: &mut T) -> Result<i64, i64> {
    let limit = duration.micros();
    let t0 = __zeta_clock_monotonic();
    let start = if t0 >= 0 { t0 } else { clock() };
    loop {
        match f.poll() {
            Poll::Ready(v) => return Result::Ok(v),
            Poll::Pending => {}
        }
        let t1 = __zeta_clock_monotonic();
        let cur = if t1 >= 0 { t1 } else { clock() };
        if cur - start >= limit {
            return Result::Err(-1);
        }
    }
}
