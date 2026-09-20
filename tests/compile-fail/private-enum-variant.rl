// 跨模块访问私有 enum 变体：`--visibility=error --no-std` 应报 PrivateItem(m::E)
// flag: --visibility=error --no-std
module m {
    enum E { A, B }
}
fn f() -> i64 {
    let _ = m::E::A;
    0
}
// expect: m::E
