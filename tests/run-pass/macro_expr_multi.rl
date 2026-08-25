// `$x:expr` 多 token 表达式匹配 + `r#"..."#` 带哈希原始字符串。
// 多 token 表达式按原样展开（优先级由调用方负责，与 Rust 语义一致）；
// 展开 token 序列为源级文本，如 `twice!(3 + 4)` → `3 + 4 * 2`。
macro_rules! twice { ($x:expr) => { $x * 2 }; }
macro_rules! larger { ($a:expr, $b:expr) => { if $a > $b { $a } else { $b } }; }

fn add10(x: i64) -> i64 { x + 10 }

fn main() {
    // 二元中缀表达式原样展开：3 + 4 * 2 = 11
    println(twice!(3 + 4));                // 11

    // 显式括号获得预期优先级
    println(twice!((3 + 4)));              // 14

    // 多参数 + 比较链
    let a = 5;
    let b = 9;
    println(larger!(a + 1, b * 2));        // 18

    // 一元负
    println(twice!(-7));                   // -14

    // 嵌套宏调用作实参
    println(twice!(larger!(1, 2)));        // 4

    // 函数调用表达式作实参
    println(twice!(add10(1)));             // 22

    // 方法调用作实参（s.len() = 5）
    let s = String::from("hello");
    println(twice!(s.len()));              // 10

    // r# 带哈希原始字符串：无转义（`\n` 保留字面）
    println!(r#"raw \n no-escape"#);       // raw \n no-escape

    // r## 多哈希定界：内容可含双引号
    let q = String::from(r##"say "hi""##);
    println(q.len());                      // 8

    // r# 字符串绑定（String::from 升级）
    let rs = String::from(r#"path\to\file"#);
    println(rs.len());                     // 12

    // 数组索引表达式作实参
    let arr = [2, 4, 6];
    println(twice!(arr[1]));               // 8
}
