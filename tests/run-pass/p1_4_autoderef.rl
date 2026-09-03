// SH-P1-4 M2：自动解引用强制——字段 / 方法 / 索引访问失败时，对接收者递归插入
// `x.deref()`（deref 返回内部值，对齐 std 智能指针 MutexGuard/RwLockGuard 的
// 内建 deref 分发：deref(&self) -> T），限深度。零新增 IR 节点。
//
// 验证：自定义守卫 `Guard<T>` 持有 `T`，`guard.method()` / `guard.field` 经自动
// 解引用透明透传至 `T`，无需手写 `(*guard).method()`。

struct Inner2 {
    z: i64,
}
impl Inner2 {
    fn double(&self) -> i64 { self.z * 2 }
}

struct Inner {
    x: i64,
    y: i64,
    inner2: Inner2,
}
impl Inner {
    fn sum(&self) -> i64 { self.x + self.y }
}
// Inner 的 deref 透传至 Inner2（嵌套强制链）
impl Inner {
    fn deref(&self) -> Inner2 { self.inner2 }
}

struct Guard<T> {
    value: T,
}
// 守卫 deref：返回被持有值（对齐 MutexGuard::deref）
impl<T> Guard<T> {
    fn deref(&self) -> T { self.value }
}

fn main() {
    let g = Guard { value: Inner { x: 3, y: 4, inner2: Inner2 { z: 5 } } };
    // 方法经自动解引用透传至 Inner::sum
    println(g.sum());    // 7
    // 字段经自动解引用透传至 Inner.x
    println(g.x);        // 3
    // 嵌套强制链：Guard 无 double -> Inner（仍无）-> Inner2.double
    println(g.double()); // 10
}
