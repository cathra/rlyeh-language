fn main() {
    // M1：元组值字面量 `(a, b, c)` + 按位置字段访问 `t.f0`
    let t = (10, 20, 30);
    println(t.f0);
    println(t.f1);
    println(t.f2);
    println(t.f0 + t.f1 + t.f2);

    // 二元组走按值聚合布局（≤2 标量槽）
    let p = (7, 8);
    println(p.f0 + p.f1);

    // 异构元组（含堆分配字段）：构造 + 字段访问
    let s = (1, "rlyeh");
    println(s.f0);
    println(s.f1);
}
