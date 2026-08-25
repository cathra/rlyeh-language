// T3a Box::leak：泄漏堆对象，返回裸指针 *mut T（G3 语义，*p 读写可用），不再释放
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    // 1. 标量：leak 返回 *mut i64，*p 读取
    let b = Box::new(42);
    let p = Box::leak(b);
    println(*p); // 42

    // 2. 裸指针写入（同一堆对象，G3 读写语义）
    let b2 = Box::new(10);
    let q = Box::leak(b2);
    *q = 99;
    println(*q); // 99

    // 3. Box<String>：leak 后解引用拷贝（聚合指针拷贝），方法可用
    let bs = Box::new(String::from("leak"));
    let ps = Box::leak(bs);
    let s = *ps;
    println(s.len()); // 4

    // 4. 聚合：Box<Point> leak 后解引用拷贝，字段访问
    let bp = Box::new(Point { x: 1, y: 2 });
    let pp = Box::leak(bp);
    let pt = *pp;
    println(pt.x + pt.y); // 3

    // 5. 类型注解：*mut i64 显式绑定
    let b5 = Box::new(7);
    let r5: *mut i64 = Box::leak(b5);
    println(*r5); // 7
}
