// 跨模块引用 pub struct/enum：`--visibility=error --no-std` 应编译通过
// flag: --visibility=error --no-std
module m {
    pub struct S { x: i64 }
    pub enum E { A, B }
}
fn use_s() -> i64 {
    let _ = m::S { x: 1 };
    0
}
fn use_e() -> i64 {
    let _ = m::E::A;
    0
}
