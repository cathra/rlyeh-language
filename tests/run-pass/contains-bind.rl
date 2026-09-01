fn contains_bind(xs: &[i64], v: i64) -> bool {
    let n = xs.len();
    let mut found = false;
    let mut i = 0;
    loop {
        if i >= n { break; }
        let e = xs[i];
        if e == v { found = true; break; }
        i = i + 1;
    }
    found
}
fn main() {
    let a = [10, 20, 30, 40];
    if contains_bind(&a, 40) {
        println("hit");
    } else {
        println("miss");
    }
}
