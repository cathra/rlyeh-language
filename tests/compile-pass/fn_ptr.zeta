// H1 类型检查：函数类型注解 + 函数一等值（编译通过，不运行）
fn identity(x: i64) -> i64 {
    x
}

fn double(x: i64) -> i64 {
    x * 2
}

fn choose(a: bool, f: fn(i64) -> i64, g: fn(i64) -> i64) -> fn(i64) -> i64 {
    if a {
        f
    } else {
        g
    }
}

fn apply2(f: fn(i64) -> i64, x: i64) -> i64 {
    f(x)
}

fn main() {
    let f: fn(i64) -> i64 = identity;
    let h = choose(true, identity, double);
    h(1);
    f(2);
    apply2(identity, 3);
}
