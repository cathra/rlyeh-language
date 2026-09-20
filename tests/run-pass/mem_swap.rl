struct Point { x: i64, y: i64 }

fn main() {
    // 标量交换
    let mut a = 1;
    let mut b = 2;
    mem::swap(&mut a, &mut b);
    println(a);   // 期望 2
    println(b);   // 期望 1

    // 结构体交换（普通 8 字节槽布局）
    let mut p = Point { x: 10, y: 20 };
    let mut q = Point { x: 30, y: 40 };
    mem::swap(&mut p, &mut q);
    println(p.x); // 30
    println(p.y); // 40
    println(q.x); // 10
    println(q.y); // 20

    // 数组交换
    let mut v = [1, 2, 3];
    let mut w = [4, 5, 6];
    mem::swap(&mut v, &mut w);
    println(v[0]); // 4
    println(w[2]); // 3
}
