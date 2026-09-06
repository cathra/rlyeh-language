// `mut` 修饰符仅可用于 `let` 解构绑定；match / if let / while let 位置绑定的变量不可变。
enum Opt { None, Some(i64) }

fn main() {
    let o = Opt::Some(1);
    match o {
        Opt::Some(mut v) => { v = 2; println(v); }
        Opt::None => { println(0); }
    }
}
