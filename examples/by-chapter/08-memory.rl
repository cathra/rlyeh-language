// 对应 docs/guide/08-memory.md —— 内存管理：分层所有权模型
// 运行：rlyeh run 08-memory.rl
// 重点：L0 所有权 + 借用（默认零开销）、L2 Box/Rc 引用计数。

fn main() {
    // ---- L0：所有权 + 借用（无需手动 free） ----
    let x = 10;
    let r = &x;            // 不可变借用（不获取所有权）
    println(*r);           // 10

    let mut y = 20;
    let m = &mut y;        // 可变借用
    *m = 21;
    println(y);            // 21
    // x / y 离开作用域时自动释放

    // ---- L2：Box 独占堆分配 ----
    let b = Box::new(42);
    println(*b);           // 42：* 解引用剥层（离开作用域自动 free）

    // ---- L2：Rc 引用计数（多处共享，非线程） ----
    let shared = Rc::new(7);
    let a = shared.clone();   // 引用计数 +1
    let c = shared.clone();   // 引用计数 +1
    println(*a + *c);         // 14：三个句柄指向同一份 7
}
