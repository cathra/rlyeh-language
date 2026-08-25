//! WASI 单线程同步 Actor 模式（L4b）。
//!
//! `wasm32-wasip1` 目标无 `dlsym`（编译期即缺符号）且无线程运行时支持，
//! 故 Actor 消息路径改用：
//!
//! - **符号解析**：driver 为 wasm 目标注入静态符号表 `zeta_actor_resolve`
//!   （编译期已知全部 handle / factory 函数名，字符串比较 → 函数地址），
//!   替代原生目标的 `dlsym(RTLD_DEFAULT, name)`；
//! - **同步执行**：spawn 仅登记（句柄 = 表索引 + 1），send / ask 直接同步
//!   调用 handle 回调——与 L1 `async` 的 MVP 同步语义一致：单线程顺序
//!   处理、消息即时执行，actor 状态由状态槽承载。
//!
//! 语义对齐（与线程池模式一致）：handle 返回 `u64::MAX`（-1）视为崩溃信号；
//! 受监督 actor 崩溃后经 factory 重建状态继续运行，无监督则标记停止；
//! ask 在崩溃时回复 0（调用方约定错误值）。dispatch 持表快照调用回调，
//! 避免嵌套消息（handler 内再向其他 actor 发消息）时 Mutex 重入死锁。

#![allow(unsafe_code)]

use std::os::raw::{c_char, c_void};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// 消息处理回调签名（与 [`super::ffi::ZetaHandler`] 一致，避免跨模块类型耦合）。
pub type ZetaHandler = unsafe extern "C" fn(u64, u64, u64, u64, u64) -> u64;

/// 状态工厂回调签名（与 [`super::ffi::ZetaFactory`] 一致；Zeta 侧 `__state_new`
/// 返回 `i8*` 状态对象指针）。
pub type ZetaFactory = unsafe extern "C" fn() -> *mut std::os::raw::c_void;

/// 表条目：回调 + 状态槽 + 监督配置 + 运行标志。
#[derive(Clone, Copy)]
struct Entry {
    handler: ZetaHandler,
    state: u64,
    factory: Option<ZetaFactory>,
    running: bool,
}

/// 同步 Actor 表（句柄 = 表索引 + 1，0 保留为无效句柄）。
static TABLE: Mutex<Vec<Option<Entry>>> = Mutex::new(Vec::new());

/// 下一个句柄（原子递增，单线程下仍保持唯一性）。
static NEXT: AtomicU64 = AtomicU64::new(1);

/// driver 注入的静态符号表：`name`（NUL 结尾 C 字符串）→ 函数地址（0 = 未找到）。
unsafe extern "C" {
    fn zeta_actor_resolve(name: *const c_char) -> u64;
}

/// 按符号名解析 handle / factory 函数（替代 dlsym）。
pub unsafe fn resolve_symbol(name: *const c_char) -> *mut c_void {
    zeta_actor_resolve(name) as *mut c_void
}

fn alloc_entry(entry: Entry) -> u64 {
    let mut table = TABLE.lock().unwrap();
    let handle = NEXT.fetch_add(1, Ordering::Relaxed);
    let idx = handle as usize;
    if idx >= table.len() {
        table.resize(idx + 1, None);
    }
    table[idx] = Some(entry);
    handle
}

/// 创建无监督 Actor：返回句柄（0 = 失败）。
pub unsafe fn spawn(handler: ZetaHandler, state: u64) -> u64 {
    alloc_entry(Entry {
        handler,
        state,
        factory: None,
        running: true,
    })
}

/// 创建受监督 Actor：崩溃时经 factory 重建状态（`strategy` 在单线程模式下
/// 无重启调度差异，忽略）。
pub unsafe fn spawn_supervised(handler: ZetaHandler, factory: ZetaFactory, _strategy: i64) -> u64 {
    let state = factory() as u64;
    alloc_entry(Entry {
        handler,
        state,
        factory: Some(factory),
        running: true,
    })
}

/// 同步派发一条消息：返回处理结果。
///
/// - `Some(v)`：正常回复值；
/// - `None`：actor 已停止 / 崩溃且无监督（ask 返回 0 的失败语义）。
///
/// 持表快照后释放锁再回调，嵌套消息（handler 内再发消息）不会死锁；
/// 崩溃重建在回调后重新加锁写回。
pub unsafe fn dispatch(actor: u64, kind: u64, a: u64, b: u64, c: u64) -> Option<u64> {
    let idx = actor as usize;
    let (handler, state, factory) = {
        let mut table = TABLE.lock().unwrap();
        let entry = match table.get_mut(idx).and_then(|e| e.as_mut()) {
            Some(e) => e,
            None => return None,
        };
        if !entry.running {
            return None;
        }
        (entry.handler, entry.state, entry.factory)
    };
    let ret = (handler)(state, kind, a, b, c);
    if ret == u64::MAX {
        // -1 = 崩溃信号：受监督 → factory 重建状态（ask 回复 0，与原生一致）；
        // 无监督 → 标记停止。
        let mut table = TABLE.lock().unwrap();
        let entry = match table.get_mut(idx).and_then(|e| e.as_mut()) {
            Some(e) => e,
            None => return None,
        };
        if let Some(factory) = factory {
            entry.state = factory() as u64;
            return Some(0);
        }
        entry.running = false;
        return None;
    }
    Some(ret)
}

/// 请求停止（同步模式下立即置停止标志）。
pub fn stop(actor: u64) {
    if let Some(e) = TABLE
        .lock()
        .unwrap()
        .get_mut(actor as usize)
        .and_then(|e| e.as_mut())
    {
        e.running = false;
    }
}

/// 查询生命周期状态编码（与原生一致）：0=Idle 1=Processing 2=Stopping
/// 3=Stopped 4=Crashed。
pub fn status(actor: u64) -> i32 {
    match TABLE
        .lock()
        .unwrap()
        .get(actor as usize)
        .and_then(|e| e.as_ref())
    {
        Some(e) if e.running => 0,
        _ => 3,
    }
}

/// 清空表（运行时关闭）。
pub fn shutdown() {
    TABLE.lock().unwrap().clear();
}
