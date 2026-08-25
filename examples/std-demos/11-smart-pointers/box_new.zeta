// K2 堆分配装箱：Box::new / 解引用 / 自动剥层字段与方法访问 / 嵌套 / 指针共享
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    // 1. 标量 i64：Box::new + 解引用
    let b = Box::new(42);
    println(*b); // 42
    let v: i64 = *b; // 类型注解解引用
    println(v); // 42

    // 2. 浮点 / 布尔
    let bf = Box::new(3.14);
    println(*bf); // 3.140000
    let bt = Box::new(true);
    println(*bt); // true

    // 3. Box<T> 显式类型注解
    let bi: Box<i64> = Box::new(7);
    println(*bi); // 7

    // 4. 嵌套 Box：Box<Box<i64>>
    let bb = Box::new(Box::new(10));
    println(**bb); // 10

    // 5. Box<String>：方法 / 索引自动剥层
    let bs = Box::new(String::from("hello"));
    println(bs.len()); // 5
    println(bs[0]); // 104 ('h' 字节)

    // 6. Box<Point>：字段自动剥层 + 聚合解引用（指针拷贝）
    let bp = Box::new(Point { x: 10, y: 20 });
    println(bp.x + bp.y); // 30
    let p = *bp;
    println(p.x); // 10
    println(p.y); // 20

    // 7. Box<Vec<i64>>：Vec 方法经 Box 调用（&mut self）
    // （`Vec::with_capacity` 返回 `Vec<Infer>`，经 Box 注解统一元素类型）
    let mut bv: Box<Vec<i64>> = Box::new(Vec::with_capacity(2));
    bv.push(5);
    bv.push(6);
    println(bv.len()); // 2
    println(bv[0] + bv[1]); // 11

    // 8. Box 赋值 = 指针共享（浅拷贝，MVP 语义）
    let b2 = b;
    println(*b2); // 42
    println(*b); // 42（同一堆对象）
}
