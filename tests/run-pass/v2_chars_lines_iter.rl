// V2：Chars/Lines 实现 Iterator trait——for 循环接入迭代器框架
//（目标签名 chars() -> Chars / lines() -> Lines 的基础；现有 chars()/lines()
// 返回 Vec 保留兼容，迭代器版为 chars_iter()/lines_iter()）。

fn main() -> i64 {
    // 1. chars_iter() 经 for 遍历码点
    let s = String::from("Ab!");
    let mut sum = 0;
    let mut count = 0;
    for c in s.chars_iter() {
        sum = sum + (c as i64); // 65 + 98 + 33 = 196（c 为 char，需 as i64）
        count = count + 1;
    }

    // 2. lines_iter() 经 for 遍历行
    let t = String::from("a\nbb\nccc");
    let mut line_sum = 0;
    let mut line_count = 0;
    for l in t.lines_iter() {
        line_sum = line_sum + l.len(); // 1+2+3 = 6
        line_count = line_count + 1;
    }

    sum + line_sum + count + line_count
}
