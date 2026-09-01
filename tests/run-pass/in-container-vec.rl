fn main() {
    let v: Vec<i64> = vec!(5, 6, 7);
    if 6 in v {
        println("hit-vec");
    } else {
        println("miss-vec");
    }
    if 100 not in v {
        println("miss100");
    } else {
        println("hit100");
    }
}
