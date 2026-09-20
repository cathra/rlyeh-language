// T-7：引用经聚合字段传播且 referent 存活——不应误报（锁定无假阳性）。
// 覆盖 `let r = w.inner` 内嵌引用具名化与 `*w.inner` 直接解引用两条新路径。
struct Wrapper 'a { inner: &'a i64 }

fn main() -> i64 {
    let a = 10;
    let b = 20;
    let w = Wrapper { inner: &a };
    let r = w.inner;       // 内嵌引用具名化
    println(*w.inner);     // 10
    println(*r);           // 10
    let w2 = Wrapper { inner: &b };
    println(*w2.inner);    // 20
    a + b
}
