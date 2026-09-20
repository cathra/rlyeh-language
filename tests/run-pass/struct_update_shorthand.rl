// X（0.2.0-X）：结构体字段简写 `Foo { x: 1, y }` 与更新语法 `Foo { a, ..base }`。
// 注：字段简写须位于显式首字段之后（首字段 `Ident :` 用于与 `if cond { }` 消歧）。
struct Point { x: i64, y: i64 }

fn main() {
    // 字段简写：y 即变量 y（位于显式首字段 x: 10 之后）
    let y = 20;
    let a = Point { x: 10, y };
    println(a.x);                     // 10
    println(a.y);                     // 20

    // 更新语法：从 base 拷贝其余字段后覆盖显式字段
    let b = Point { x: 1, y: 2 };
    let c = Point { x: 3, ..b };
    println(c.x);                     // 3
    println(c.y);                     // 2

    // 仅覆盖一个字段，其余全拷贝
    let d = Point { y: 99, ..c };
    println(d.x);                     // 3
    println(d.y);                     // 99
}
