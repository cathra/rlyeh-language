// 跨模块以私有 actor 作类型标注：`--visibility=error --no-std` 应报 PrivateItem(m::Act)
// flag: --visibility=error --no-std
module m {
    actor Act { v: i64 = 0 }
}
fn f(a: m::Act) -> i64 { 0 }
// expect: m::Act
