// V1 Vec 目标 API：iter/iter_mut（瘦指针迭代器，零拷贝）/ get_mut（值拷贝）/ sort_by（比较器闭包）
// 注：vec! 字面量产 Vec<Infer>，for 迭代 / 泛型方法调用需显式类型注解（MVP 延迟推断机制）
// V1（2026-08）：iter() 返回 Iter<T>（*const T + 剩余长度，next() 值拷贝读取）；
// iter_mut() 返回 IterMut<T>（*mut T + cur 写回目标），write(x) 经 DerefSet 写回原元素。
fn main() {
    // 1. iter 接入 for：元素值拷贝迭代
    let v: Vec<i64> = vec![5, 1, 4, 2, 3];
    let mut sum = 0;
    for x in v.iter() {
        sum = sum + x;
    }
    println(sum); // 15

    // 2. iter 瘦指针手动迭代：next 值拷贝 + 剩余长度（非缓冲长度快照）
    let v2: Vec<i64> = vec![10, 20];
    let mut it = v2.iter();
    println(it.len()); // 2（剩余元素数）
    match it.next() {
        Option::Some(x) => println(x), // 10
        Option::None => println(-1),
    }
    match it.next() {
        Option::Some(x) => println(x), // 20
        Option::None => println(-1),
    }
    println(it.is_empty()); // true

    // 3. get_mut：`Option<&mut T>` 引用语义——命中返回对原槽的可变引用，
    //    经 `match { Some(r) => *r = x }` 写回真实槽（非拷贝）；越界返回 None。
    let mut v3: Vec<i64> = vec![7, 8, 9];
    match v3.get_mut(1) {
        Option::Some(r) => *r = 88,
        Option::None => println(-1),
    }
    println(v3[1]); // 88（真实写回）
    match v3.get_mut(9) {
        Option::Some(r) => *r = 0,
        Option::None => println(-1), // -1（越界）
    }

    // 4. sort_by：比较器闭包（降序，|a, b| b - a 返回三态 i64）
    let mut v4: Vec<i64> = vec![3, 1, 4, 1, 5];
    v4.sort_by(|a, b| b - a);
    for x in v4.iter() {
        println(x); // 5 4 3 1 1
    }

    // 5. sort_by 升序（|a, b| a - b），与 sort 结果一致
    let mut v5: Vec<i64> = vec![3, 1, 4, 1, 5];
    v5.sort_by(|a, b| a - b);
    for x in v5.iter() {
        println(x); // 1 1 3 4 5
    }

    // 6. iter_mut + write：DerefSet 真实写回原元素（非拷贝）
    let mut v6: Vec<i64> = vec![1, 2, 3];
    let mut itm = v6.iter_mut();
    match itm.next() {
        Option::Some(x) => itm.write(x * 100), // v6[0] = 100
        Option::None => itm.write(0),
    }
    println(v6[0]); // 100（真实写回）
    println(v6[1]); // 2（未动）
    match itm.next() {
        Option::Some(x) => itm.write(x + 5), // v6[1] = 7
        Option::None => itm.write(0),
    }
    println(v6[1]); // 7

    // 7. 适配器链：Iter<T> 接收者 + H2 闭包（J3 适配器检测 next() 方法）
    let v7: Vec<i64> = vec![1, 2, 3];
    let r = v7.iter().map(|x| x * 10).collect();
    println(r.len()); // 3
    println(r[0]); // 10
    println(r[2]); // 30
}
