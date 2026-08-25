//! 析构管理：区域内分配的需要 drop 的对象统一注册，
//! 区域销毁时按注册逆序（LIFO）调用其析构函数。

use std::mem::needs_drop;
use std::ptr::{self, NonNull};

/// 析构条目：记录对象指针与其析构函数。
struct DestructorEntry {
    ptr: NonNull<u8>,
    drop_fn: unsafe fn(NonNull<u8>),
}

/// 通用 `drop_in_place` 适配：把裸指针还原为 `*mut T` 后原地析构。
///
/// # Safety
/// `ptr` 必须指向由 `Region::allocate::<T>` 写入的合法 `T`，且 `T` 未被转移。
fn drop_in_place<T>(ptr: NonNull<u8>) {
    // SAFETY: 调用方保证 ptr 指向有效的 T，且析构责任尚未转移。
    unsafe { ptr::drop_in_place(ptr.as_ptr() as *mut T) };
}

/// 析构注册表：按注册顺序逆序执行（后分配的先析构）。
#[derive(Default)]
pub(crate) struct DestructorRegistry {
    entries: Vec<DestructorEntry>,
}

impl DestructorRegistry {
    /// 新建空注册表。
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 注册对象析构。`T` 无需 drop 时不产生条目。
    pub(crate) fn register<T>(&mut self, ptr: NonNull<u8>) {
        if needs_drop::<T>() {
            self.entries.push(DestructorEntry {
                ptr,
                drop_fn: drop_in_place::<T>,
            });
        }
    }

    /// 逆序执行全部析构，返回执行的次数，并清空注册表。
    pub(crate) fn run_all(&mut self) -> usize {
        let n = self.entries.len();
        for e in self.entries.drain(..).rev() {
            // SAFETY: 条目由 register<T> 创建，ptr 指向有效 T。
            unsafe { (e.drop_fn)(e.ptr) };
        }
        n
    }

    /// 移除指定指针的析构条目（用于 `transfer` 后的所有权移交），
    /// 返回是否移除成功。
    pub(crate) fn remove(&mut self, ptr: NonNull<u8>) -> bool {
        if let Some(idx) = self.entries.iter().position(|e| e.ptr == ptr) {
            self.entries.swap_remove(idx);
            true
        } else {
            false
        }
    }
}
