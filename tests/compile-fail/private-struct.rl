// 跨模块构造私有 struct 字面量：`--visibility=error --no-std` 应报 PrivateItem(m::S)
// flag: --visibility=error --no-std
module m {
    struct S { x: i64 }
}
fn f() -> i64 {
    let _ = m::S { x: 1 };
    0
}
// expect: m::S
// expect: not accessible
