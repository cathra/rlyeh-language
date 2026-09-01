fn main() {
    let a = [10, 20, 30];
    let mut s = 0;
    for x in &a {
        s = s + x;
    }
    println(s);
}
