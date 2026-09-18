// B-3 / B-4：生命周期省略与 region 参数化。
// - 省略 'a（B-3）：`struct Excerpt { part: &i64 }` 与
//   `fn longest(x: &i64, y: &i64) -> &i64` 默认可用
//   （默认 caller region，等价于既有 drop 语义）。
// - B-4 新语法：`struct Wrapper 'a { inner: &'a i64 }` region 参数化被接受。

struct Excerpt { part: &i64 }

struct Wrapper 'a {
    inner: &'a i64,
}

fn longest(x: &i64, y: &i64) -> &i64 {
    if *x > *y { x } else { y }
}

fn main() -> i64 {
    let a = 10;
    let b = 20;
    let e = Excerpt { part: &a };
    let w = Wrapper { inner: &b };
    let r = *longest(&a, &b);
    println(*e.part);   // 10
    println(*w.inner);  // 20
    println(r);         // 20
    r
}
