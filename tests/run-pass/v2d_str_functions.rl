// V2-B：trim_start / trim_end 单侧剥离 + 全空白空视图
fn main() {
    let s = String::from("  hello  ");
    println(s.trim_start());
    println(s.trim_end());
    let w = String::from(" \t ");
    println(w.trim().len());
    println(w.trim_start().len());
    println(w.trim_end().len());
    // V2-D：String::from(&str) 深拷贝
    let src = String::from("world");
    let view = src.as_str();
    let copy = String::from(view);
    println(copy);
    println(copy.len());
    println(copy == src);
}
