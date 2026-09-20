fn update(a: i64, b: i64) -> i64 {
    let and = a && b;
    let or = a || b;
    let eq = a == b;
    let ne = a != b;
    let le = a <= b;
    let ge = a >= b;
    let shl = a << 2;
    let shr = a >> 3;
    let add = a += 1;
    if a == 0 { return -1; }
    let r = a..b;
    let s = a..<b;
    let t = a...b;
    match a { 0 => 1, _ => 2 }
}
