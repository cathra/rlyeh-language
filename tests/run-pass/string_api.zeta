// T1b String 目标 API 补齐：chars（字节级字符列表）/ lines（行切分）/
// to_uppercase / to_lowercase（to_upper / to_lower 的 API 别名）
fn main() {
    // 1. chars：字节级字符列表（MVP 字符 = 字节）
    let s = String::from("Ab!");
    let cs = s.chars();
    println(cs.len()); // 3
    println(cs[0]); // 65 (A)
    println(cs[2]); // 33 (!)

    // 2. lines：按 \n 切分
    let t = String::from("a\nbb\nccc");
    let ls = t.lines();
    println(ls.len()); // 3
    println(ls[0].len()); // 1 (a)
    println(ls[1].len()); // 2 (bb)
    println(ls[2].len()); // 3 (ccc)

    // 3. lines 尾随换行产生尾空行段（split 语义）
    let t2 = String::from("x\ny\n");
    let ls2 = t2.lines();
    println(ls2.len()); // 3（x、y、空行）
    println(ls2[2].len()); // 0

    // 4. to_uppercase / to_lowercase 别名
    println(String::from("abc123").to_uppercase()); // ABC123
    println(String::from("DeF").to_lowercase()); // def

    // 5. 别名与原名一致
    let up = String::from("Hello");
    println(up.to_uppercase() == up.to_upper()); // true
    println(up.to_lowercase() == up.to_lower()); // true
}
