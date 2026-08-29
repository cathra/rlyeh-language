// V1：Vec::iter_ref() 只读引用迭代器（next() -> Option<&T> 零拷贝，指向原缓冲真实槽）
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(10);
    v.push(20);
    v.push(30);
    // 1. 只读引用求和（*r 解引用读取）
    let mut s = 0;
    for r in v.iter_ref() {
        s = s + *r;
    }
    println(s); // 60
    // 2. 原地写回（*r = ... 修改原缓冲真实槽）
    for r in v.iter_ref() {
        *r = *r + 1;
    }
    let mut s2 = 0;
    for r in v.iter_ref() {
        s2 = s2 + *r;
    }
    println(s2); // 63
    // 3. 与 iter() 值拷贝对比（应一致，验证写回生效）
    let mut s3 = 0;
    for x in v.iter() {
        s3 = s3 + x;
    }
    println(s3); // 63
    // 4. 空 Vec 引用迭代（应不触发、求和为 0）
    let empty: Vec<i64> = Vec::new();
    let mut s4 = 0;
    for r in empty.iter_ref() {
        s4 = s4 + *r;
    }
    println(s4); // 0
}
