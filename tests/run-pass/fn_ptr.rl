// H1 函数一等值：函数指针传递与间接调用，输出与 fn_ptr.out 精确对比
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn apply(f: fn(i64, i64) -> i64, x: i64, y: i64) -> i64 {
    f(x, y)
}

fn main() {
    let f = add;
    println!("direct via fp: {}", f(3, 4));
    let r = apply(add, 10, 20);
    println!("as arg: {}", r);
    let g = f;
    println!("re-bound: {}", g(7, 8));
    let typed: fn(i64, i64) -> i64 = add;
    println!("typed: {}", typed(1, 2));
    let r2 = f(5, 6);
    println!("store again: {}", r2);
}
