// Y4b-1：* 解引用支持自定义 Deref<T> trait（typecheck UnaryOp::Deref 分派）
// `MutexGuard<T>` 实现 Deref<T>，`*guard` 生成 `guard.deref()` 返回 T。
// 此前 `*g` 仅支持内建类型（&T/裸指针/Box/Rc/Arc/Gc）——报 unsupported syntax。

protocol Deref<T> {
    fn deref(&self) -> T;
}

struct MutexGuard<T> { value: T }

impl<T> MutexGuard<T>: Deref<T> {
    fn deref(&self) -> T {
        let g: T = self.value;
        g
    }
}

fn main() {
    // 1. *g 解引用 i64
    let g1 = MutexGuard { value: 42 };
    println(*g1); // 42
    // 2. *g 解引用 String
    let g2 = MutexGuard { value: String::from("hi") };
    println(*g2); // hi
    // 3. 显式 deref() 调用仍可用（等义）
    let g3 = MutexGuard { value: 7 };
    println(g3.deref()); // 7
}
