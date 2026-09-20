struct Point { x: i64, y: i64 }

fn main() {
    // 标量替换并返回旧值
    let mut a = 1;
    let old = mem::replace(&mut a, 99);
    println(old);  // 1
    println(a);    // 99

    // 结构体替换
    let mut p = Point { x: 10, y: 20 };
    let oldp = mem::replace(&mut p, Point { x: 30, y: 40 });
    println(oldp.x); // 10
    println(oldp.y); // 20
    println(p.x);    // 30
    println(p.y);    // 40

    // 数组替换
    let mut v = [1, 2, 3];
    let oldv = mem::replace(&mut v, [7, 8, 9]);
    println(oldv[0]); // 1
    println(v[2]);    // 9

    // 作为语句（旧值丢弃）：仅验证写入生效
    let mut q = 5;
    mem::replace(&mut q, 42);
    println(q);    // 42
}
