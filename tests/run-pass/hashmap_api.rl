// T1c HashMap 目标 API 补齐：iter（MVP 退化键缓冲，与 keys 同构）/ get_mut（值拷贝）
// 注：map! 字面量产 HashMap<Infer, Infer>，for 迭代 / 泛型方法调用需显式类型注解
fn main() {
    // 1. iter：键缓冲（可 for 迭代求和）
    let m: HashMap<i64, i64> = map![1 => 10, 2 => 20];
    let ks = m.iter();
    let mut sum = 0;
    for k in ks {
        sum = sum + k;
    }
    println(sum); // 3

    // 2. iter 与 keys 同构（同一键集合，长度一致）
    let m2: HashMap<i64, i64> = map![5 => 50, 6 => 60, 7 => 70];
    println(m2.iter().len()); // 3
    println(m2.keys().len()); // 3

    // 3. get_mut：`Option<&mut V>` 引用语义——命中返回对原槽的可变引用
    //    （println 自动剥层打印解引用值）；缺失键返回 None。
    let mut m3: HashMap<i64, i64> = map![1 => 100, 2 => 200];
    match m3.get_mut(2) {
        Option::Some(v) => println(v), // 200
        Option::None => println(-1),
    }
    match m3.get_mut(9) {
        Option::Some(v) => println(v),
        Option::None => println(-1), // -1（缺失键）
    }

    // 4. get_mut 写回真实槽：`*r = x` 后 get 读回新值；插回/删除后引用失效（宽松）
    // 注：避免与第 3 段 match 臂同名变量（MVP variables 按名全局索引、
    // 无作用域隔离——同名遮蔽类型互相覆盖，已知限制）
    let mut m4: HashMap<i64, i64> = map![3 => 300];
    let mut vv = m4.get_mut(3);
    match vv {
        Option::Some(r) => *r = 333, // 写回真实槽
        Option::None => println(-1),
    }
    match m4.get(3) {
        Option::Some(x) => println(x), // 333
        Option::None => println(-1),
    }
    m4.insert(3, 999);
    println(m4.iter().len()); // 1
}
