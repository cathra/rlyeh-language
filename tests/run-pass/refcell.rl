// SH-P2-3 (0.2.0-I) 验证：安全内部可变性封装（I1 的 RefCell 等价）。
// 经 unsafe 裸指针 + alloc_array 堆缓冲承载 T；safe 面提供 get(&self)/set(&self)，
// 使共享所有者可在无 &mut 接收者的情况下 mutate（绕过借用检查）。与现有 Rc<T> 组合
// 即得 Rc<RefCell<T>> 共享可变状态（Rust 惯用法），替代 Rc/Arc/RefCell 内部可变性模式。

struct RefCell<T> {
    ptr: *mut T,
}

impl<T> RefCell<T> {
    fn new(x: T) -> RefCell<T> {
        let buf: [T; 0] = alloc_array(1);
        buf[0] = x;
        let p: *mut T = &buf[0];
        RefCell { ptr: p }
    }
    // 共享引用下读出（Copy 类型按值返回）。
    fn get(&self) -> T {
        unsafe { *self.ptr }
    }
    // 内部可变性核心：&self 即可写回，无需 &mut 接收者。
    fn set(&self, x: T) {
        unsafe { *self.ptr = x; }
    }
}

fn main() {
    // 1. 基本内部可变性：&self 写入
    let cell = RefCell::new(1);
    println(cell.get());            // 1
    cell.set(42);
    println(cell.get());            // 42

    // 2. 共享别名下各自 mutate（同一底层 ptr，两份句柄）
    let a = RefCell::new(10);
    let b = RefCell { ptr: a.ptr };
    b.set(99);
    println(a.get());               // 99（a、b 共享同一底层）
    a.set(7);
    println(b.get());               // 7
}
