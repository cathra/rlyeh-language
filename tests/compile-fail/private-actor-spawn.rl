// 跨模块 spawn 私有 actor，--visibility=error 应报 PrivateItem(m::PrivateActor)
// flag: --visibility=error --no-std
module m {
    actor PrivateActor { v: i64 = 0 }
}
fn f() -> i64 {
    let _ = m::PrivateActor::new();
    0
}
// expect: m::PrivateActor
// expect: not accessible
