// Y5：Box::leak 目标签名——返回 &mut T 引用（替代 *mut T 裸指针退化）
// U5 AddrOf 已就绪：leak 返回指向堆 T 的 `&'static mut T`（codegen 中引用与
// 裸指针同为地址值），`*leaked` 解引用读写堆对象。'static 宽松丢弃（G4）。
struct Point { x: i64, y: i64 }

fn main() {
    // 1. 标量：leak 返回 &mut i64，*p 读取
    let b = Box::new(42);
    let p = Box::leak(b);
    println(*p); // 42

    // 2. &mut 语义：*q 写入（同一堆对象）
    let b2 = Box::new(10);
    let q = Box::leak(b2);
    *q = 99;
    println(*q); // 99

    // 3. Box<Point> leak 后解引用拷贝，字段访问
    let bp = Box::new(Point { x: 1, y: 2 });
    let pp = Box::leak(bp);
    let pt = *pp;
    println(pt.x + pt.y); // 3

    // 4. 类型注解 &mut i64 显式绑定
    let b5 = Box::new(7);
    let r5: &mut i64 = Box::leak(b5);
    println(*r5); // 7
}
