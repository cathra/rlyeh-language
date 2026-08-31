# 时间（time 模块）

时间类型，位于 `time` 子模块（`import time::Duration;` 已在根模块重导出）。底层时间戳为微秒（`i64`，约 ±292 年）。

> **C 程序员对照**：Rlyeh 的 `Duration`/`Instant`/`SystemTime` ≈ C 的 `struct timespec` + `clock_gettime`（`CLOCK_MONOTONIC`/`CLOCK_REALTIME`），或 C++ 的 `std::chrono`。关键不同：`Duration` 内部就是一个**整数微秒计数**（类似 Rust `Duration`），单位换算用构造器（`from_secs`/`from_millis`…）和读回方法（`secs()`/`millis()`…）完成，**不会像 C 那样让你纠结 `tv_sec`/`tv_nsec` 字段**。注意：**纳秒精度会截断到微秒**（底层是微秒整数）。

## Duration

时间间隔。构造时按单位缩放为微秒存储；读回时按单位还原（整数截断）。

### 构造

```rlyeh
let d1 = Duration::from_secs(1);           // 1_000_000 us
let d2 = Duration::from_millis(90000);     // 90_000_000 us
let d3 = Duration::from_micros(123);       // 123 us
let d4 = Duration::from_nanos(123456);     // 123 us（纳秒截断到微秒）
```

- `Duration::from_secs(s: i64) -> Duration`
- `Duration::from_millis(ms: i64) -> Duration`
- `Duration::from_micros(us: i64) -> Duration`
- `Duration::from_nanos(ns: i64) -> Duration`（纳秒截断到微秒精度）

### 读回（成员/方法）

| 读取 | 类型 | 说明 |
|------|------|------|
| `secs` / `d.secs()` | `i64` | 秒（微秒 / 1e6，整数） |
| `millis` / `d.millis()` | `i64` | 毫秒 |
| `micros` / `d.micros()` | `i64` | 微秒 |
| `nanos` / `d.nanos()` | `i64` | 纳秒（微秒 × 1000，长时长溢出时 saturate） |

```rlyeh
let d = Duration::from_secs(90);
println(d.secs());          // 90
println(d.millis());        // 90000
println(d.micros());        // 90000000
println(d.nanos());         // 90000000000
```

> 跨单位换算：`secs*micros_per_sec + (micros%micros_per_sec)` 组合重建 64 位 `Duration`，读回时按除数/取模拆分；纳秒读回由微秒 ×1000 还原（长时长 saturate 防溢出）。`from_nanos(123456)` → `micros=123` → `nanos=123000`（纳秒向下截断）。

## Instant

单调时钟（常用于计时，不受系统时间回拨影响）。

### 构造

```rlyeh
let t = Instant::now();     // 当前单调时间
```

- `Instant::now() -> Instant`

### 方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `elapsed` | `() -> Duration` | 距 `now()` 的时长 |
| `duration_since` | `(other: Instant) -> Duration` | 与另一 Instant 的差 |

```rlyeh
let start = Instant::now();
// ... 工作 ...
let el = start.elapsed();
println(el.micros());
```

## SystemTime

系统时钟（墙上时间）。

### 构造

```rlyeh
let now = SystemTime::now();
```

- `SystemTime::now() -> SystemTime`

### 方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `duration_since` | `(other: SystemTime) -> Duration` | 与另一 SystemTime 的差 |
| `elapsed` | `() -> Duration` | 距 `now()` 的时长 |

## 完整示例

```rlyeh
fn main() {
    let start = Instant::now();
    Thread::sleep(10);                       // 休眠 10ms（见 sync 模块）
    let el = start.elapsed();
    println(el.millis());

    let d = Duration::from_secs(2);
    println(d.secs());
    println(d.nanos());
}
```

---

[← 返回标准库详述索引](./index.md)
