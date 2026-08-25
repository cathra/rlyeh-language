//! Zeta 追踪 GC 运行时（K4 `Gc<T>` 保守标记-清除）。
//!
//! 对象布局约定（与编译器内建 `Gc<T>` 一致）：
//! - 对象 = `slots` 个 8 字节槽的连续堆块（malloc），`T` 值区自堆首槽起
//!   （与 `Box<T>` 完全同构，解引用 / 剥层零差异）
//! - 标记 / 存活状态保存在运行时块表（不占对象内存）
//! - 值区内任意槽位值若等于某已注册对象基址，视为引用（保守扫描，安全）
//!
//! 生命周期协议（`gc_region` 块 desugar 调用序列）：
//! 1. `zeta_gc_region_begin()`：`epoch += 1`
//! 2. `zeta_gc_alloc(n)`：分配 `n` 槽对象（malloc + 注册块表 + 记录当前 epoch）
//! 3. `zeta_gc_escape(ptr)`：登记块返回值对象为逃逸 root（`ptr` 为空则空操作）
//! 4. `zeta_gc_collect()`：从逃逸 root 标记 → 清除本块（`epoch` 匹配）未标记对象
//!    → 本块存活对象提升为逃逸 root → `epoch -= 1`
//!
//! 关键语义（epoch 分层）：
//! - 块外分配的 `Gc` 对象 `epoch` 恒小于任何块 → 永不回收（MVP 泄漏语义）
//! - 块内对象：本块结束时仅回收 `epoch` 匹配且未被标记的（未逃逸、无存活引用）
//! - 跨块存活的引用链经"存活提升"连续保护（内层逃逸对象在后续块 collect 中是 root）
//!
//! MVP 限制（见 memory-model.md §5 MVP 注记）：非增量（stop-the-world）、全局互斥
//! （非线程局部）、递归标记（深引用图可能爆栈）、值区内非指针槽误判为引用是
//! 保守行为（安全但可能漏回收）；`gc_region` 结束后块内对象失效（逃逸限制，
//! 无悬空指针防护）；跨块逃逸对象及其引用图泄漏至程序结束。
//!
//! 线程模型：MVP 编译产物为单线程（无 actor 调度并发），全局状态经
//! `SyncUnsafeCell` 直接可变借用，全程无锁。多线程 GC（全局锁 / 线程局部堆）
//! 规划见 memory-model.md §5。
//!
//! 分配器说明（重要）：本运行时全部动态内存管理**只使用 `libc::malloc` /
//! `libc::free`**（对象块 + 元数据节点链表），**不使用 Rust 堆分配
//! （`Vec` / `Box`）与 `realloc`**。原因（macOS 实测）：在链接了 Rust
//! staticlib 的 C 主程序环境中，经 `RawVec`（`Vec::with_capacity` / `push`
//! 扩容）与 `libc::realloc` 的分配在先前 `libc::malloc` 之后调用会触发
//! libsystem_malloc 的 `mfm_alloc` 内 `_os_unfair_lock_unowned_abort` 崩溃
//! （SIGKILL / EXC_BREAKPOINT，复现实验详见代码注释）；而直接
//! `libc::malloc` / `libc::free` 与 `std::alloc::alloc` 均正常。元数据用
//! 链表组织（每对象一个 header 节点），避免任何"二次分配/扩容"形态。

use std::cell::UnsafeCell;

/// GC 对象元数据节点（链表）。
///
/// 与对象数据块（`base` 指向 `slots` 个 8 字节槽）分离，`marked` / `epoch`
/// 等状态不占用对象内存。
#[derive(Clone, Copy)]
struct GcHeader {
    base: *mut i64,
    slots: usize,
    epoch: usize,
    marked: bool,
    next: *mut GcHeader,
}

/// 逃逸 root 节点（链表）。
#[derive(Clone, Copy)]
struct RootNode {
    base: *mut i64,
    next: *mut RootNode,
}

/// 全局 GC 状态。
struct GcState {
    /// 所有已分配对象（跨 collect 保留存活对象）——header 链表
    list: *mut GcHeader,
    /// 逃逸 root（collect 时标记起点；collect 后替换为本块存活对象）——链表
    roots: *mut RootNode,
    /// 当前块层级（`gc_region` 嵌套深度）
    epoch: usize,
}

impl Default for GcState {
    fn default() -> Self {
        GcState {
            list: std::ptr::null_mut(),
            roots: std::ptr::null_mut(),
            epoch: 0,
        }
    }
}

/// 全局状态容器：`UnsafeCell` 提供内部可变性，`Sync` 由调用约定保证
/// （单线程串行调用，见模块头注释）。
struct GlobalState(UnsafeCell<Option<GcState>>);

// SAFETY: 所有 `zeta_gc_*` C ABI 入口在 MVP 中由编译器生成的单线程代码串行调用，
// 且每个入口内对 `STATE` 的借用互不重叠（`&mut` 同一时刻唯一）。
unsafe impl Sync for GlobalState {}

static STATE: GlobalState = GlobalState(UnsafeCell::new(None));

fn with_state<T>(f: impl FnOnce(&mut GcState) -> T) -> T {
    // SAFETY: 单线程串行调用约定（见模块头），`&mut *` 唯一借用安全。
    let cell = unsafe { &mut *STATE.0.get() };
    f(cell.get_or_insert_with(GcState::default))
}

/// 分配 `slots` 个 8 字节槽的 GC 对象，注册块表，返回基址（C ABI 返回指针）。
///
/// 分配序列：对象数据块（malloc）→ 元数据节点（malloc）→ 挂链表。全部为
/// 首次 `malloc`，不涉及扩容 / `realloc`（见模块头"分配器说明"）。
#[no_mangle]
pub extern "C" fn zeta_gc_alloc(slots: i64) -> *mut i64 {
    let slots = slots.max(1) as usize;
    let bytes = slots * 8;
    let base = unsafe { libc::malloc(bytes) as *mut i64 };
    assert!(!base.is_null(), "zeta-gc: malloc 失败 ({} bytes)", bytes);
    // 清零（避免未初始化槽被误判为引用）。注意 `base` 为 `*mut i64`，
    // 须先转 `*mut u8` 按字节清零（`i64` 的 `write_bytes` 按元素计数会
    // 溢出写入 `bytes * 8` 字节，破坏 malloc 元数据导致后续分配崩溃）。
    unsafe { (base as *mut u8).write_bytes(0u8, bytes) };
    let hdr = unsafe { libc::malloc(std::mem::size_of::<GcHeader>()) as *mut GcHeader };
    assert!(!hdr.is_null(), "zeta-gc: 元数据节点 malloc 失败");
    unsafe {
        (*hdr).base = base;
        (*hdr).slots = slots;
        (*hdr).marked = false;
    }
    with_state(|s| {
        unsafe {
            (*hdr).epoch = s.epoch;
            (*hdr).next = s.list; // 头插
        }
        s.list = hdr;
    });
    base
}

/// `gc_region` 块入口：块层级 +1（块内新分配带本块 epoch 标签）。
#[no_mangle]
pub extern "C" fn zeta_gc_region_begin() {
    with_state(|s| s.epoch += 1);
}

/// 登记块返回值对象为逃逸 root（`ptr` 为空指针时为空操作）。
#[no_mangle]
pub extern "C" fn zeta_gc_escape(ptr: *mut i64) {
    if ptr.is_null() {
        return;
    }
    let node = unsafe { libc::malloc(std::mem::size_of::<RootNode>()) as *mut RootNode };
    assert!(!node.is_null(), "zeta-gc: root 节点 malloc 失败");
    with_state(|s| {
        unsafe {
            (*node).base = ptr;
            (*node).next = s.roots; // 头插
        }
        s.roots = node;
    });
}

/// `gc_region` 块结束：标记-清除。
///
/// 标记起点 = 逃逸 root；清除本块（`epoch` 匹配）未标记对象；外层对象
/// （`epoch` 更小）永不回收；本块存活对象提升为逃逸 root（链式保护），
/// 随后块层级 -1。
#[no_mangle]
pub extern "C" fn zeta_gc_collect() {
    with_state(|s| unsafe {
        // 1. 标记（逃逸 root 出发，递归扫描值区指针槽）
        let mut r = s.roots;
        while !r.is_null() {
            mark(s, (*r).base);
            r = (*r).next;
        }

        // 2. 清除 + 存活提升（头插重建链表；顺序无关，查找为线性）
        let mut keep: *mut GcHeader = std::ptr::null_mut();
        let mut new_roots: *mut RootNode = std::ptr::null_mut();
        let mut h = s.list;
        while !h.is_null() {
            let next = (*h).next;
            let alive = (*h).marked || (*h).epoch < s.epoch;
            if alive {
                // 所有存活对象提升为逃逸 root（含 epoch 更小的外层对象）：
                // 嵌套块 collect 会释放全部旧 root 节点，若仅提升本块（epoch 匹配）
                // 已标记对象，外层对象（epoch < s.epoch 存活、但在本块内被外层
                // 变量引用、未从 root 可达）将失去 root 保护——外层块 collect 时
                // 因未标记且 epoch 匹配被误回收（如嵌套块中 `let o2 = o;` 引用
                // 的外层 `o`）。epoch 更小的对象本就不会被本块回收，提升它们为
                // root 只增保护不增回收（MVP 泄漏语义，见 memory-model.md §5）。
                if (*h).marked || (*h).epoch < s.epoch {
                    let node = libc::malloc(std::mem::size_of::<RootNode>()) as *mut RootNode;
                    assert!(!node.is_null(), "zeta-gc: root 节点 malloc 失败");
                    (*node).base = (*h).base;
                    (*node).next = new_roots;
                    new_roots = node;
                }
                (*h).marked = false;
                (*h).next = keep;
                keep = h;
            } else {
                libc::free((*h).base as *mut libc::c_void);
                libc::free(h as *mut libc::c_void);
            }
            h = next;
        }

        // 3. 释放旧 root 节点
        let mut old = s.roots;
        while !old.is_null() {
            let next = (*old).next;
            libc::free(old as *mut libc::c_void);
            old = next;
        }

        s.list = keep;
        s.roots = new_roots;
        s.epoch = s.epoch.saturating_sub(1);
    });
}

/// 从 `base` 出发递归标记（DFS，状态存元数据链表，扫值区全部槽）。
fn mark(s: &mut GcState, base: *mut i64) {
    // 元数据定位（MVP：线性查找）
    let mut h = s.list;
    while !h.is_null() {
        unsafe {
            if (*h).base == base {
                if (*h).marked {
                    return; // 已标记（防循环引用）
                }
                (*h).marked = true;
                let slots = (*h).slots;
                // 扫描值区全部槽：值为已注册对象基址则递归标记（保守扫描）
                for i in 0..slots {
                    let v = *base.add(i);
                    if v != 0 {
                        let ptr = v as *mut i64;
                        let mut j = s.list;
                        while !j.is_null() {
                            if (*j).base == ptr {
                                mark(s, ptr);
                                break;
                            }
                            j = (*j).next;
                        }
                    }
                }
                return;
            }
            h = (*h).next;
        }
    }
    // 非 GC 对象地址（标量槽误判）→ 忽略
}
