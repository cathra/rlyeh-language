// SH-P1-5（2026-09-04）：Copy 标记 protocol + #[derive(Copy)] + T: Copy 泛型约束。
//
// Rlyeh 默认聚合即按值拷贝（无 move 语义），故 `Copy` 在此主要作为标记与约束：
// ① `#[derive(Copy)]` 展开为 `impl Copy for T {}`；
// ② `T: Copy` 约束经 `type_implements_protocol` 命中派生 impl；
// ③ Copy 类型传值后原绑定仍可用（不移动）。

#[derive(Copy)]
struct Point {
    x: i64,
    y: i64,
}

// T: Copy 约束的泛型函数：接收并原样返回 Copy 类型（按值，不移动）
fn identity<T: Copy>(v: T) -> T {
    v
}

#[derive(Clone, Copy)]
struct Label {
    id: i64,
}

fn main() {
    let p = Point { x: 3, y: 4 };
    let q = identity(p);     // 传值（Copy）
    println(q.x);            // 3
    println(q.y);            // 4
    println(p.x);            // 3（p 仍可用：Copy 不移动）

    let l = Label { id: 7 };
    let l2 = identity(l);
    println(l2.id);          // 7
}
