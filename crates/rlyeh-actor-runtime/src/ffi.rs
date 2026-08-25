//! C ABI 导出层：Zeta 语言侧经 `extern fn` 声明调用（链接静态库接入）。
//!
//! # 消息协议（MVP）
//!
//! - 消息 = [`ZetaMsg`]（`kind` 方法索引 + `a/b/c` 三个 i64 参数槽）；
//! - 消息处理回调 = Zeta 编译器为每个 actor 生成的
//!   `_zeta_actor_<name>_handle(state, kind, a, b, c) -> u64` 函数，
//!   经 `dlsym(RTLD_DEFAULT, 函数名)` 按符号名解析（Zeta 侧传函数名字符串，
//!   语言层无需函数指针表达能力）；
//! - 回调返回 `u64::MAX`（即 -1）表示处理出错 → [`ActorError::Panic`] →
//!   supervisor 按策略决策（重启 / 停止 / 升级）；
//! - 其余返回值作为 ask 回复（send 场景经 [`ActorContext::reply`] 静默忽略）；
//! - 状态内存由 Zeta 侧分配（`calloc`），运行时仅持有指针不负责释放
//!   （MVP 无析构，进程退出时由 OS 回收）。
//!
//! # 初始化
//!
//! 全局运行时**懒初始化**：首次 `spawn` / `send` / `ask` 时自动创建
//! （2 workers）；用户可先调 [`zeta_actor_init`] 指定 worker 数。

#![allow(unsafe_code)]

use std::any::Any;
use std::ffi::CStr;
use std::os::raw::{c_char, c_void};
use std::sync::{Mutex, OnceLock};

use crate::actor::{ActorContext, ActorState};
use crate::error::ActorError;
use crate::runtime::{ActorRef, Runtime, RuntimeBuilder};
use crate::supervisor::RestartStrategy;

/// C 消息结构（与 Zeta 侧消息槽布局一致：kind = 方法索引，a/b/c = 参数槽）。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZetaMsg {
    /// 方法索引（Zeta 编译器按 actor 方法声明顺序编号）。
    pub kind: u64,
    /// 参数槽 0。
    pub a: u64,
    /// 参数槽 1。
    pub b: u64,
    /// 参数槽 2。
    pub c: u64,
}

/// 消息处理回调签名（Zeta 编译器生成的 handle 函数）。
pub type ZetaHandler = unsafe extern "C" fn(u64, u64, u64, u64, u64) -> u64;

/// 状态工厂回调签名（supervisor 重启时重建状态对象）。
///
/// Zeta 侧 `__state_new` 实际返回 `i8*`（状态对象指针），故统一为指针返回，
/// 与 wasm（i32）及原生（8 字节）ABI 均匹配；值经 `as u64` 落入状态槽。
pub type ZetaFactory = unsafe extern "C" fn() -> *mut c_void;

/// 桥接 Actor 状态：Zeta 侧状态槽值（`__state_new` 返回的 u64，存 actor 字段
/// 槽位地址）+ 编译器生成的 handle 函数。
pub struct CallbackActor {
    /// Zeta 侧状态槽值（u64，由 `__state_new` 返回；原生目标下即状态内存地址）。
    ///
    /// `pub(crate)`：ask 快速路径需直接读状态槽并同步调用 handler。
    pub(crate) state: u64,
    /// 消息处理回调。
    pub(crate) handler: ZetaHandler,
}

// 状态内存由 Zeta 侧分配，运行时对同一 Actor 的消息处理串行化
// （每 Actor 单线程互斥），状态槽值在 Actor 间迁移由调度器保证安全。
// 生命周期与进程一致（MVP 不释放），因此可安全跨线程。
unsafe impl Send for CallbackActor {}
unsafe impl Sync for CallbackActor {}

impl ActorState for CallbackActor {
    fn handle_message(
        &mut self,
        msg: Box<dyn Any + Send>,
        ctx: &mut ActorContext,
    ) -> Result<bool, ActorError> {
        let m = msg
            .downcast::<ZetaMsg>()
            .map_err(|_| ActorError::WrongReplyType)?;
        // 回调：state 与消息槽均按 u64 传递（Zeta 侧 i64 槽语义）。
        let ret = unsafe { (self.handler)(self.state, m.kind, m.a, m.b, m.c) };
        if ret == u64::MAX {
            // -1 为崩溃信号：handle 返回 -1 表示处理出错，视为 panic。
            // 先 reply 0 让 ask 立即失败返回（否则 ask 要干等满超时），
            // 再返回 Err 触发 supervisor 重启（无 supervisor 则 actor 停止）。
            // 注意 0 需显式标注 u64：ask_blocking 按 u64 downcast 回复，
            // 裸字面量会被推断为 i32 导致 WrongReplyType。
            ctx.reply(0u64);
            return Err(ActorError::Panic {
                reason: "zeta handle 返回 -1（处理出错）".into(),
            });
        }
        // 非 ask 消息 reply 被 ActorContext 静默忽略，安全。
        ctx.reply(ret);
        Ok(false)
    }
}

/// 状态工厂（supervisor 用）：调 Zeta factory 回调重建状态对象。
#[derive(Clone, Copy)]
pub struct CallbackFactory {
    /// 消息处理回调（重启后的新状态实例仍用同一 handle）。
    handler: ZetaHandler,
    /// 状态重建回调。
    factory: ZetaFactory,
}

impl CallbackFactory {
    /// 创建新的桥接 Actor 状态实例。
    pub fn make(&self) -> Box<dyn ActorState> {
        let state = unsafe { (self.factory)() } as u64;
        Box::new(CallbackActor {
            state,
            handler: self.handler,
        })
    }
}

/// 全局运行时（懒初始化）。
static GLOBAL: OnceLock<Mutex<Option<Runtime>>> = OnceLock::new();

fn global() -> &'static Mutex<Option<Runtime>> {
    GLOBAL.get_or_init(|| Mutex::new(None))
}

/// 获取运行时引用；未初始化时自动初始化（默认 2 workers）。
fn with_runtime<T>(f: impl FnOnce(&Runtime) -> T) -> T {
    let mut guard = global().lock().unwrap();
    if guard.is_none() {
        *guard = Some(RuntimeBuilder::new().with_workers(2).build());
    }
    f(guard.as_ref().unwrap())
}

/// 将 C 字符串转为 Rust 字符串（空指针 → 空串）。
unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    CStr::from_ptr(ptr).to_string_lossy().into_owned()
}

/// 按符号名解析 Zeta 生成的 handle / factory 函数。
///
/// 返回 `None` 表示符号未找到（Zeta 侧可能未生成该函数，或符号被 strip）。
unsafe fn resolve_symbol(name: &str) -> Option<*mut c_void> {
    #[cfg(not(target_os = "wasi"))]
    {
        let name_c = std::ffi::CString::new(name).ok()?;
        let ptr = libc::dlsym(libc::RTLD_DEFAULT, name_c.as_ptr());
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
    // WASI 无 dlsym：查 driver 注入的静态符号表 `zeta_actor_resolve`（L4b）。
    #[cfg(target_os = "wasi")]
    {
        let name_c = std::ffi::CString::new(name).ok()?;
        let ptr = crate::sync::resolve_symbol(name_c.as_ptr());
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
}

/// 将 ActorRef 装箱为不透明句柄（`Box<ActorRef>` 裸指针）。
fn actor_to_handle(actor: ActorRef) -> u64 {
    Box::into_raw(Box::new(actor)) as u64
}

/// 从句柄取 ActorRef 引用（句柄生命周期与进程一致，MVP 不释放）。
unsafe fn handle_to_ref(handle: u64) -> &'static ActorRef {
    &*(handle as *const ActorRef)
}

/// 错误信息打印到 stderr。
fn report_err(what: &str, e: &ActorError) {
    eprintln!("zeta-actor-runtime: {what}: {e}");
}

// ---------------------------------------------------------------------------
// C ABI 导出
// ---------------------------------------------------------------------------

/// 显式初始化运行时（可选；默认首次使用时自动初始化）。
///
/// `workers` 为工作线程数（自动夹在 1..=64）。返回 0 表示成功。
/// WASI 单线程同步模式下无 worker，参数忽略，恒返回 0。
#[no_mangle]
pub unsafe extern "C" fn zeta_actor_init(_workers: i64) -> i32 {
    #[cfg(not(target_os = "wasi"))]
    {
        let mut guard = global().lock().unwrap();
        if guard.is_none() {
            let n = _workers.clamp(1, 64) as usize;
            *guard = Some(RuntimeBuilder::new().with_workers(n).build());
        }
    }
    0
}

/// 创建无监督 Actor。
///
/// `handler` 为 Zeta 编译器生成的 handle 函数名（C 字符串），
/// `state` 为 Zeta 侧 `__state_new` 返回的状态槽值（u64；原生目标下即状态
/// 内存地址）。返回 ActorRef 不透明句柄（0 表示失败：符号未找到或运行时
/// 未初始化）。
#[no_mangle]
pub unsafe extern "C" fn zeta_actor_spawn(handler: *const c_char, state: u64) -> u64 {
    let name = cstr(handler);
    let Some(ptr) = (unsafe { resolve_symbol(&name) }) else {
        eprintln!("zeta-actor-runtime: 未找到 handle 符号: {name}");
        return 0;
    };
    let handler: ZetaHandler = std::mem::transmute(ptr);
    #[cfg(target_os = "wasi")]
    {
        // 单线程同步模式：登记 (handler, state) 并返回句柄（L4b）。
        crate::sync::spawn(handler, state)
    }
    #[cfg(not(target_os = "wasi"))]
    {
        let actor = with_runtime(|rt| {
            rt.spawn_boxed(Box::new(CallbackActor { state, handler }))
        });
        actor_to_handle(actor)
    }
}

/// 创建受监督的 Actor（崩溃时按策略重启，窗口配置用运行时默认值）。
///
/// `handler` / `factory` 为 Zeta 编译器生成的 handle / factory 函数名
/// （C 字符串）。`strategy`：0 = OneForOne，1 = AllForOne，2 = RestartForOne。
/// 返回 ActorRef 不透明句柄（0 表示失败）。
#[no_mangle]
pub unsafe extern "C" fn zeta_actor_spawn_supervised(
    handler: *const c_char,
    factory: *const c_char,
    strategy: i64,
) -> u64 {
    let handler_name = cstr(handler);
    let factory_name = cstr(factory);
    let Some(hp) = (unsafe { resolve_symbol(&handler_name) }) else {
        eprintln!("zeta-actor-runtime: 未找到 handle 符号: {handler_name}");
        return 0;
    };
    let Some(fp) = (unsafe { resolve_symbol(&factory_name) }) else {
        eprintln!("zeta-actor-runtime: 未找到 factory 符号: {factory_name}");
        return 0;
    };
    let handler: ZetaHandler = std::mem::transmute(hp);
    let factory: ZetaFactory = std::mem::transmute(fp);
    #[cfg(target_os = "wasi")]
    {
        // 单线程同步模式：登记受监督条目（崩溃时经 factory 重建状态）。
        crate::sync::spawn_supervised(handler, factory, strategy)
    }
    #[cfg(not(target_os = "wasi"))]
    {
        let strat = match strategy {
            0 => RestartStrategy::OneForOne,
            1 => RestartStrategy::AllForOne,
            _ => RestartStrategy::RestartForOne,
        };
        let cf = CallbackFactory { handler, factory };
        let actor = with_runtime(|rt| rt.spawn_supervised(move || cf.make(), strat));
        actor_to_handle(actor)
    }
}

/// fire-and-forget 发送消息。返回 0 表示成功，非 0 表示失败
/// （actor 已停止 / 邮箱满 / 运行时关闭）。
#[no_mangle]
pub unsafe extern "C" fn zeta_actor_send(
    actor: u64,
    kind: u64,
    a: u64,
    b: u64,
    c: u64,
) -> i32 {
    #[cfg(target_os = "wasi")]
    {
        // 单线程同步模式：同步派发（send 语义 = 立即处理，忽略回复）。
        return match crate::sync::dispatch(actor, kind, a, b, c) {
            Some(_) => 0,
            None => {
                eprintln!("zeta-actor-runtime: send 失败: actor 已停止");
                1
            }
        };
    }
    #[cfg(not(target_os = "wasi"))]
    {
        let actor_ref = unsafe { handle_to_ref(actor) };
        let msg = Box::new(ZetaMsg { kind, a, b, c });
        match actor_ref.send(msg) {
            Ok(()) => 0,
            Err(e) => {
                report_err("send 失败", &e);
                1
            }
        }
    }
}

/// ask 请求（阻塞等待回复，超时 [`crate::ASK_TIMEOUT`]）。
///
/// 返回回复值；失败（actor 停止 / 超时等）返回 0 并打印错误到 stderr
/// （MVP：失败返回值与合法回复 0 无法区分，调用方自行约定）。
#[no_mangle]
pub unsafe extern "C" fn zeta_actor_ask(
    actor: u64,
    kind: u64,
    a: u64,
    b: u64,
    c: u64,
) -> u64 {
    #[cfg(target_os = "wasi")]
    {
        // 单线程同步模式：同步调用并直接返回回复（崩溃回复 0，与原生一致）。
        return match crate::sync::dispatch(actor, kind, a, b, c) {
            Some(reply) => reply,
            None => {
                eprintln!("zeta-actor-runtime: ask 失败: actor 已停止");
                0
            }
        };
    }
    #[cfg(not(target_os = "wasi"))]
    {
        let actor_ref = unsafe { handle_to_ref(actor) };
        let msg = Box::new(ZetaMsg { kind, a, b, c });
        match actor_ref.ask_blocking::<u64>(msg) {
            Ok(reply) => reply,
            Err(e) => {
                report_err("ask 失败", &e);
                0
            }
        }
    }
}

/// 请求 actor 停止（处理完当前消息后退出）。返回 0。
#[no_mangle]
pub unsafe extern "C" fn zeta_actor_stop(actor: u64) -> i64 {
    #[cfg(target_os = "wasi")]
    {
        crate::sync::stop(actor);
        0
    }
    #[cfg(not(target_os = "wasi"))]
    {
        let actor_ref = unsafe { handle_to_ref(actor) };
        actor_ref.stop();
        0
    }
}

/// 查询 actor 生命周期状态：0 = Idle，1 = Processing，2 = Stopping，
/// 3 = Stopped，4 = Crashed。
#[no_mangle]
pub unsafe extern "C" fn zeta_actor_status(actor: u64) -> i32 {
    #[cfg(target_os = "wasi")]
    {
        crate::sync::status(actor)
    }
    #[cfg(not(target_os = "wasi"))]
    {
        let actor_ref = unsafe { handle_to_ref(actor) };
        actor_ref.status() as u8 as i32
    }
}

/// 优雅关闭运行时（排空消息队列后停止 Worker 与全部 Actor）。返回 0。
#[no_mangle]
pub unsafe extern "C" fn zeta_actor_shutdown() -> i64 {
    #[cfg(target_os = "wasi")]
    {
        crate::sync::shutdown();
        0
    }
    #[cfg(not(target_os = "wasi"))]
    {
        let mut guard = global().lock().unwrap();
        if let Some(rt) = guard.take() {
            rt.shutdown();
        }
        0
    }
}
