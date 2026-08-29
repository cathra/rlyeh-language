// T1b String 目标 API 补齐：chars（UTF-8 码点迭代器）/ lines（行迭代器）/
// to_uppercase / to_lowercase（to_upper / to_lower 的 API 别名）
fn main() {
    // 1. chars：UTF-8 码点迭代器（MVP 字符 = 字节，ASCII 一致）；
    // V2（2026-08-29）：`chars()` 现返回 `Chars` 迭代器（`next() -> Option<char>`）。
    let s = String::from("Ab!");
    let mut count = 0;
    let mut sum = 0;
    for c in s.chars() {
        count = count + 1;
        sum = sum + (c as i64); // 65 + 98 + 33 = 196
    }
    println(count); // 3
    println(sum);   // 196

    // 2. lines：按 \n 切分（惰性迭代器）；`lines()` 现返回 `Lines`。
    let t = String::from("a\nbb\nccc");
    let mut line_count = 0;
    let mut line_len_sum = 0;
    for l in t.lines() {
        line_count = line_count + 1;
        line_len_sum = line_len_sum + l.len(); // 1 + 2 + 3 = 6
    }
    println(line_count); // 3
    println(line_len_sum); // 6

    // 3. lines 尾随换行产生尾空行段（split 语义）
    let t2 = String::from("x\ny\n");
    let mut line_count2 = 0;
    let mut line_len_sum2 = 0;
    for l in t2.lines() {
        line_count2 = line_count2 + 1;
        line_len_sum2 = line_len_sum2 + l.len(); // 1 + 1 + 0 = 2
    }
    println(line_count2); // 3（x、y、空行）
    println(line_len_sum2); // 2

    // 4. to_uppercase / to_lowercase 别名
    println(String::from("abc123").to_uppercase()); // ABC123
    println(String::from("DeF").to_lowercase()); // def

    // 5. 别名与原名一致
    let up = String::from("Hello");
    println(up.to_uppercase() == up.to_upper()); // true
    println(up.to_lowercase() == up.to_lower()); // true
}
