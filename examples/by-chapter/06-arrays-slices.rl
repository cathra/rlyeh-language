// 对应 docs/guide/06-arrays-slices.md —— 数组、Vec 与切片
// 运行：rlyeh run 06-arrays-slices.rl
fn max(xs: &[i64]) -> i64 {
    let mut m = xs[0];
    for x in xs {
        if x > m { m = x; }
    };
    m
}

fn main() {
    let arr = [1, 2, 3, 4, 5];
    let part = arr[1..<3];          // 值拷贝，返回新 Vec [2, 3]
    println(part[0]);               // 2

    println(max(&arr));             // 5：切片引用把长度绑进类型

    let v = vec![10, 20, 30];
    let doubled = v.map(|x| x * 2); // 适配器返回新 Vec
    println(doubled[0]);            // 20
}
