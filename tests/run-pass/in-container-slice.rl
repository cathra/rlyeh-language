fn member(xs: &[i64], v: i64) -> bool {
    v in xs
}
fn main() {
    let a: [i64; 4] = [10, 20, 30, 40];
    if member(&a, 40) {
        println("hit-slice");
    } else {
        println("miss-slice");
    }
    if member(&a, 1) {
        println("never");
    } else {
        println("miss-slice-2");
    }
}
