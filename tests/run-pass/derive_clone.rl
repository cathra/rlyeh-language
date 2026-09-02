// SH-P1-2（0.2.0-C）：#[derive(Clone)] 自动合成 `impl Clone for Point`。
#[derive(Clone)]
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let p = Point { x: 1, y: 2 };
    let q = p.clone();   // 逐字段拷贝
    println(q.x);
    println(q.y);

    // 含 String 字段的结构体：String 走 .clone()
    let s = Named { id: 7, label: String::from("hi") };
    let t = s.clone();
    println(t.id);
    println(t.label);
}

#[derive(Clone)]
struct Named {
    id: i64,
    label: String,
}
