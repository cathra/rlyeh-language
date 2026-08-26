// V2-C 方案 A：&str（StrFat）打印链路修复
// as_str / as_str_range / trim 的 &str 子区间走 %.*s 长度限定打印
fn main() {
    let s = String::from("hello world");
    let v = s.as_str();
    println(v); // hello world（全串）
    let r = s.as_str_range(0, 5);
    println(r); // hello（子区间前 5 字节）
    let t = String::from("  hi  ");
    println(t.trim()); // hi（trim 剥离空白）
}
