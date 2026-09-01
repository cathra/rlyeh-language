fn contains(a: [i64; 4], v: i64) -> bool {
    v in a
}
fn main() {
    let a = [10, 20, 30, 40];
    if contains(a, 30) {
        println("hit-fn");
    } else {
        println("miss-fn");
    }
    if contains(a, 99) {
        println("never");
    } else {
        println("miss-fn-2");
    }
}
