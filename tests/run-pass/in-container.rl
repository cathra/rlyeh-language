// M2：`in` 右侧运行时容器（数组 / Vec / 切片）成员判断 run-pass 验证
fn member(xs: &[i64], v: i64) -> bool {
    v in xs
}

fn main() {
    // 1) 数组字面量容器
    let x = 2;
    if x in [1, 2, 3] {
        println("hit-arr-lit");
    }
    if 9 in [1, 2, 3] {
        println("never");
    }

    // 2) 数组变量容器 + not in
    let a: [i64; 4] = [10, 20, 30, 40];
    if 30 in a {
        println("hit-arr-var");
    }
    if 99 not in a {
        println("miss-arr-var");
    }

    // 3) Vec 容器
    let v: Vec<i64> = vec!(5, 6, 7);
    if 6 in v {
        println("hit-vec");
    }
    if 100 not in v {
        println("miss-vec");
    }

    // 4) 切片容器（数组经 unsize coercion 到 &[i64]）
    if member(&a, 40) {
        println("hit-slice");
    }
    if member(&a, 1) {
        println("never");
    } else {
        println("miss-slice");
    }
}
