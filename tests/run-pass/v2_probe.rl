// V2：完整 trim_basic
fn main() {
    let s = String::from("  hello  ");
    println(s.trim());
    let t = String::from("\t padded \n");
    println(t.trim());
    let u = String::from("left");
    println(u.trim());
    println(u.trim() == u);
    let v = String::from("a b c");
    println(v.trim());
}
