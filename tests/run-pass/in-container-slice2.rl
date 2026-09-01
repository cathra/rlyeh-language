fn main() {
    let a = [10, 20, 30, 40];
    let s: &[i64] = &a;
    if 40 in s {
        println("hit");
    } else {
        println("miss");
    }
    if 1 in s {
        println("never");
    } else {
        println("miss2");
    }
}
