// 宏系统运行测试：用户宏 + 内置格式化宏，输出与 macro.out 精确对比
macro_rules! double {
    ($x:expr) => { $x * 2 };
}
macro_rules! max2 {
    ($a:expr, $b:expr) => {
        if $a > $b { $a } else { $b }
    };
}

fn main() {
    println!("double(21) = {}", double!(21));
    println!("max(3, 7) = {}", max2!(3, 7));
    println!("sum {} + {} = {}", 1, 2, 3);
    let s = format!("fmt: {} and {}", "abc", 7);
    println!("{}", s);
    let d = dbg!(double!(10));
    println!("dbg returned {}", d);
    println!("pi={}", 3 + 4);
    println!("strarg={}", format!("{} + {}", "ab", "cd"));
}
