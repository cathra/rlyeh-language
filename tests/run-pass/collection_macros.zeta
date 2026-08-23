// I3 集合宏：arr! / vec! / map!（parse 期 desugar 为数组字面量 / 块表达式，
// 经 Vec::with_capacity + push、HashMap::with_capacity + insert 构造，零新增 IR）

macro_rules! double {
    ($x:expr) => { $x * 2 };
}

fn sum_vec(v: Vec<i64>) -> i64 {
    let mut s = 0;
    for x in v {
        s += x;
    }
    s
}

fn main() {
    // 1. arr! → 数组字面量（长度编译期已知，索引访问）
    let a = arr![1, 2, 3];
    println(a[0] + a[1] + a[2]);        // 6

    // 2. vec! 带元素 → Vec<i64>
    let v = vec![10, 20, 30];
    println(sum_vec(v));                // 60

    // 3. vec! 空 → Vec::new()
    let e = vec![];
    println(e.len());                   // 0

    // 4. vec! 元素为任意表达式（含嵌套宏调用展开）
    let w = vec![double!(1), double!(2), 5];
    println(sum_vec(w));                // 11

    // 5. map! 键值对 → HashMap<i64, i64>
    let m = map![1 => 10, 2 => 20];
    println(m.len());                   // 2
    match m.get(2) {
        Option::Some(x) => println(x),  // 20
        Option::None => println(0),
    }

    // 6. map! 空 → HashMap::new()
    let me = map![];
    println(me.len());                  // 0

    // 7. vec! 结果可继续可变操作
    let mut mv = vec![1, 2];
    mv.push(3);
    println(sum_vec(mv));               // 6

    // 8. map! 键冲突覆盖旧值
    let m2 = map![1 => 10, 1 => 20];
    match m2.get(1) {
        Option::Some(x) => println(x),  // 20
        Option::None => println(0),
    }

    // 9. 元素可为绑定变量与算术表达式（非字面量）
    let base = 5;
    let vb = vec![base, base + 1, base * 2];
    println(sum_vec(vb));               // 21
}
