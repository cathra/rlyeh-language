// T1a Vec 目标 API 补齐：iter（MVP 退化元素拷贝缓冲）/ get_mut（值拷贝）/ sort_by（比较器闭包）
// 注：vec! 字面量产 Vec<Infer>，for 迭代 / 泛型方法调用需显式类型注解（MVP 延迟推断机制）
fn main() {
    // 1. iter：元素拷贝缓冲，可 for 直接迭代
    let v: Vec<i64> = vec![5, 1, 4, 2, 3];
    let mut sum = 0;
    for x in v.iter() {
        sum = sum + x;
    }
    println(sum); // 15

    // 2. iter 返回独立缓冲：快照语义，原 Vec 修改不影响已取 iter
    let mut v2: Vec<i64> = vec![10, 20];
    let it = v2.iter();
    v2.push(30);
    println(it.len()); // 2（拷贝缓冲）

    // 3. get_mut：值拷贝（与 get 一致，MVP 无 &mut 引用返回）
    let mut v3: Vec<i64> = vec![7, 8, 9];
    println(v3.get_mut(1)); // 8

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
}
