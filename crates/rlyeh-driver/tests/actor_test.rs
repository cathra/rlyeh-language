//! 阶段 C：Actor 语言级接线集成测试。
//!
//! 覆盖：
//! - `Actor::new()` → `zeta_actor_spawn`（编译器生成 `__state_new` 初始化状态）
//! - 方法调用 `.await` → `zeta_actor_ask`（同步往返）
//! - `send actor.method(args)` → `zeta_actor_send`（异步 fire-and-forget）
//! - 多实例独立状态 / 多字段多方法 / 复合赋值
//! - Supervisor 恢复（语言级生成的 `__handle`/`__state_new` 符号 + extern 监督 spawn）

use std::path::PathBuf;

use zeta_driver::run_source_file;

/// 独立临时目录，避免并行测试互相覆盖。
fn temp_dir() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("zeta-actor-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.zeta），返回程序输出。
fn run_zeta(src: &str) -> String {
    let dir = temp_dir();
    let file = dir.join("main.zeta");
    std::fs::write(&file, src).expect("写入 main.zeta 失败");
    let out = run_source_file(&file).expect("actor 测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 1. Counter 自增：ask 往返 + 复合赋值 + 字段读取 + reset
#[test]
fn counter_increment_ask() {
    let src = r#"
actor Counter {
    value: i64 = 0,

    pub fn increment(amount: i64) -> i64 {
        self.value += amount;
        self.value
    }

    pub fn get() -> i64 {
        self.value
    }

    pub fn reset() -> i64 {
        self.value = 0;
        self.value
    }
}

fn main() {
    let c = Counter::new();
    let r1 = c.increment(10).await;
    let r2 = c.increment(5).await;
    let r3 = c.get().await;
    let r4 = c.reset().await;
    println(r1);
    println(r2);
    println(r3);
    println(r4);
    println(c.increment(7).await);
}
"#;
    assert_eq!(run_zeta(src), "10\n15\n15\n0\n7\n");
}

/// 2. send 异步：fire-and-forget 后 get 可见
#[test]
fn counter_send_async() {
    let src = r#"
actor Counter {
    value: i64 = 0,

    pub fn increment(amount: i64) -> i64 {
        self.value += amount;
        self.value
    }

    pub fn get() -> i64 {
        self.value
    }
}

fn main() {
    let c = Counter::new();
    send c.increment(100);
    let r = c.get().await;
    println(r);
    send c.increment(1);
    send c.increment(2);
    println(c.get().await);
}
"#;
    assert_eq!(run_zeta(src), "100\n103\n");
}

/// 3. ping-pong：echo 往返 + 方法链
#[test]
fn ping_pong_echo() {
    let src = r#"
actor Echo {
    last: i64 = 0,

    pub fn echo(x: i64) -> i64 {
        self.last = x;
        self.last
    }

    pub fn peek() -> i64 {
        self.last
    }
}

fn main() {
    let e = Echo::new();
    let p1 = e.echo(42).await;
    let p2 = e.echo(p1 * 2).await;
    println(p1);
    println(p2);
    println(e.peek().await);
}
"#;
    assert_eq!(run_zeta(src), "42\n84\n84\n");
}

/// 4. 多实例独立状态
#[test]
fn multi_actor_independent() {
    let src = r#"
actor Counter {
    value: i64 = 0,

    pub fn increment(amount: i64) -> i64 {
        self.value += amount;
        self.value
    }

    pub fn get() -> i64 {
        self.value
    }
}

fn main() {
    let a = Counter::new();
    let b = Counter::new();
    let ra1 = a.increment(3).await;
    let rb1 = b.increment(9).await;
    let ra2 = a.increment(1).await;
    let rb2 = b.get().await;
    println(ra1);
    println(rb1);
    println(ra2);
    println(rb2);
    println(a.get().await);
}
"#;
    assert_eq!(run_zeta(src), "3\n9\n4\n9\n4\n");
}

/// 5. 多字段多方法：字段种类（i64/bool）+ 多参数（≤3）+ 复杂运算
#[test]
fn multi_field_multi_method() {
    let src = r#"
actor Calc {
    sum: i64 = 0,
    count: i64 = 0,
    ready: bool = false,

    pub fn add(a: i64, b: i64, c: i64) -> i64 {
        self.sum += a + b + c;
        self.count += 1;
        self.sum
    }

    // 注意：不能返回 -1（runtime 视其为崩溃信号 → Panic）
    pub fn state() -> i64 {
        if self.ready {
            self.sum * 100 + self.count
        } else {
            -2
        }
    }

    pub fn mark() -> i64 {
        self.ready = true;
        1
    }
}

fn main() {
    let c = Calc::new();
    let s1 = c.add(1, 2, 3).await;
    let s2 = c.add(10, 20, 30).await;
    let st = c.state().await;
    let mk = c.mark().await;
    let st2 = c.state().await;
    println(s1);
    println(s2);
    println(st);
    println(mk);
    println(st2);
}
"#;
    assert_eq!(run_zeta(src), "6\n66\n-2\n1\n6602\n");
}

/// 6）Supervisor 恢复：语言级 `Boom::new_supervised(0)`（编译器自动生成
/// `zeta_actor_spawn_supervised` extern 声明），崩溃（返回 -1）后重启，
/// 由 `__state_new` 重建初始状态（不再依赖手工 extern 声明）。
#[test]
fn supervisor_restart() {
    let src = r#"
actor Boom {
    state: i64 = 7,

    // kind 0：state + a
    pub fn step(a: i64) -> i64 {
        self.state += a;
        self.state
    }

    // kind 1：崩溃信号（返回 -1 → runtime 视为 Panic）
    pub fn crash() -> i64 {
        -1
    }
}

fn main() {
    let boom = Boom::new_supervised(0);
    let r1 = boom.step(5).await;
    println(r1);
    // 触发崩溃 → supervisor 重启 → __state_new 重建 state=7
    let crash = boom.crash().await;
    println(crash);
    let r2 = boom.step(3).await;
    println(r2);
}
"#;
    assert_eq!(run_zeta(src), "12\n0\n10\n");
}

/// 7）无监督崩溃：`new()`（无 supervisor）下方法返回 -1，
/// runtime 判定 Panic 且无 supervisor，actor 停止，
/// 后续 ask 失败返回 0（非阻塞，避免 ask 干等满超时）。
#[test]
fn crash_without_supervisor_stops_actor() {
    let src = r#"
actor Wobbly {
    n: i64 = 0,

    pub fn add(a: i64) -> i64 {
        self.n += a;
        self.n
    }

    pub fn boom() -> i64 {
        -1
    }
}

fn main() {
    let w = Wobbly::new();
    println(w.add(7).await);
    // 崩溃：panic 前 reply 0 让 ask 立即返回
    println(w.boom().await);
    // actor 已停止，ask 失败返回 0
    println(w.add(1).await);
}
"#;
    assert_eq!(run_zeta(src), "7\n0\n0\n");
}

/// 8）send FIFO 顺序：连续 `send` 的消息按入队顺序处理，
/// 之后立即 ask 能观察到全部累积效果（每 actor 互斥处理）。
#[test]
fn send_fifo_order() {
    let src = r#"
actor Buffer {
    total: i64 = 0,

    pub fn add(a: i64) -> i64 {
        self.total += a;
        self.total
    }

    pub fn total() -> i64 {
        self.total
    }
}

fn main() {
    let b = Buffer::new();
    send b.add(1);
    send b.add(2);
    send b.add(3);
    println(b.total().await);
    send b.add(10);
    send b.add(20);
    println(b.total().await);
}
"#;
    assert_eq!(run_zeta(src), "6\n36\n");
}

/// 9）resolve 支持：`use` 导入模块内 actor 后，短名构造 / 方法调用 / send
/// 均经 `resolve_full_name`（`actors` 表 + use_aliases）正确解析到完整符号名
/// `service::Counter`（`TypeContext.actors` + `lookup_actor`）。
#[test]
fn use_imported_actor_resolve() {
    let src = r#"
mod service {
    actor Counter {
        value: i64 = 0,

        pub fn add(n: i64) -> i64 {
            self.value += n;
            self.value
        }

        pub fn get() -> i64 {
            self.value
        }
    }
}

use service::Counter;

fn main() {
    let c = Counter::new();
    let r1 = c.add(10).await;
    println(r1);
    send c.add(5);
    let r2 = c.get().await;
    println(r2);
}
"#;
    assert_eq!(run_zeta(src), "10\n15\n");
}
