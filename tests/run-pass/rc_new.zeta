// K3 引用计数装箱：Rc::new / 解引用 / 自动剥层 / clone 计数 / 弱引用 / try_unwrap / Arc 同构
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    // 1. Rc::new + 解引用（标量 load / 聚合指针拷贝）
    let r = Rc::new(42);
    println(*r); // 42
    let v: i64 = *r;
    println(v); // 42

    // 2. 强引用计数：clone 共享同一 RcInner
    println(r.strong_count()); // 1
    let r2 = r.clone();
    println(r2.strong_count()); // 2
    println(r.strong_count()); // 2

    // 3. 方法 / 索引剥层（Rc<String>）
    let rs = Rc::new(String::from("hello"));
    println(rs.len()); // 5
    println(rs[0]); // 104 ('h' 字节)

    // 4. 字段剥层（Rc<Point>）+ 聚合解引用
    let rp = Rc::new(Point { x: 10, y: 20 });
    println(rp.x + rp.y); // 30
    let p = *rp;
    println(p.x); // 10

    // 5. 弱引用：downgrade / weak_count / upgrade
    let w = r.downgrade();
    println(r.weak_count()); // 1
    match w.upgrade() {
        Option::Some(rc) => println(*rc), // 42
        Option::None => println(0),
    }

    // 6. try_unwrap：强计数 > 1 → Err；== 1 → Ok
    // （r / r2 共享强计数 2，此处再 clone 保持 > 1 走 Err 分支）
    match r.clone().try_unwrap() {
        Result::Ok(x) => println(x),
        Result::Err(rc) => println(*rc), // 42
    }

    // 7. Arc 与 Rc 同构（计数槽原子性规划中）
    let a = Arc::new(7);
    println(*a); // 7
    println(a.strong_count()); // 1
    let a2 = a.clone();
    println(a.strong_count()); // 2

    // 8. Rc<Vec<i64>>：方法剥层（&mut self push 写入堆上 Vec 对象）
    let mut rv: Rc<Vec<i64>> = Rc::new(Vec::with_capacity(2));
    rv.push(5);
    rv.push(6);
    println(rv.len()); // 2
    println(rv[0] + rv[1]); // 11
}
