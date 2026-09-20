// 跨模块 spawn pub actor，--visibility=error 应编译通过（私有 actor spawn 才会被拒）
// flag: --visibility=error --no-std
module m {
    pub actor PubActor { v: i64 = 0 }
}
fn f() -> i64 {
    let _ = m::PubActor::new();
    0
}
