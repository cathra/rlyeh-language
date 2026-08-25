// 宏系统：用户声明式宏展开 + 内置格式化宏，编译必须成功
macro_rules! twice {
    ($x:expr) => { $x + $x };
}
macro_rules! apply3 {
    ($f:expr, $x:expr) => { $f($f($x)) };
}
macro_rules! choose {
    ($cond:expr, $a:expr, $b:expr) => {
        if $cond { $a } else { $b }
    };
}

fn id(x: i64) -> i64 { x }

fn main() {
    let a = twice!(5);
    let b = apply3!(id, 1);
    if a > b {
        println!("a bigger");
    } else {
        println!("b bigger");
    }
    println!("{} {} {:?}", a, b, true);
    let s = format!("value = {}", a + b);
    println!("{}", s);
    let c = dbg!(a);
    println!("{}", c);
    let d = choose!(true, 100, 200);
    println!("{}", d);
}
