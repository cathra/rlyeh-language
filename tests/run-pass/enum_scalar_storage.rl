// U3 核心项（2026-08-30）：标量枚举的存储与布局交互验证——
// 结构体字段 / 数组元素 / 跨函数返回 / 表达式 match / 整数上下文 /
// 与裸整数双向比较 / 作为联合成员，均按单标量（值即 tag）一致处理。

enum Color { Red, Green, Blue }
struct Pair { a: Color, b: Color }

fn make(c: Color) -> Color { c }

fn main() {
    let p = Pair { a: Color::Red, b: Color::Green };
    match p.a {
        Color::Red => println(1),
        Color::Green => println(2),
        Color::Blue => println(3),
    }
    let arr = [Color::Red, Color::Green, Color::Blue];
    let x = arr[1];
    match x {
        Color::Red => println(10),
        Color::Green => println(20),
        Color::Blue => println(30),
    }
    let m = make(Color::Blue);
    match m {
        Color::Blue => println(100),
        _ => println(0),
    }
    let arr2 = [Color::Red, Color::Blue];
    match arr2[0] {
        Color::Red => println(1000),
        Color::Blue => println(2000),
        _ => println(0),
    }
    let n: i64 = Color::Green;
    println(n);
    // 与裸整数双向比较（对称）
    let c = Color::Green;
    if c == 1 { println(11); }
    if 1 == c { println(12); }
    // 作为联合成员
    let u: i64 | Color = Color::Green;
    match u {
        i64 => println(100),
        Color => println(200),
    }
}
