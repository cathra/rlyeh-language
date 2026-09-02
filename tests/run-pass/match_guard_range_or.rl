// SH-P0-7：`match` 守卫 + 范围模式 + 或模式（0.2.0-P）。
//   P-M1 守卫   `pat if cond => ..`（守卫条件与模式条件 And 合并）
//   P-M2 范围   `lo..<hi` / `lo...hi` / `lo<..hi`，语义同 `x in lo..<hi`
//   P-M3 或模式 `A | B`（各备选须绑定同名同序的变量集）
//   P-M4 守卫 + 范围 / 或模式组合

enum Shape { Circle(i64), Square(i64) }

// P-M2 范围模式：三种开闭区间
fn classify(n: i64) -> i64 {
    match n {
        0 => 0,
        1...9 => 1,          // [1, 9]
        10..<20 => 2,        // [10, 20)
        20<..30 => 3,        // (20, 30]
        _ => -1,
    }
}

// P-M1 守卫：绑定变量在守卫内可见
fn guarded(o: Option<i64>) -> i64 {
    match o {
        Option::Some(v) if v > 10 => 1,
        Option::Some(v) if v > 5 => 2,
        Option::Some(_) => 3,
        Option::None => 4,
    }
}

// P-M3 或模式：字面量备选
fn or_lit(n: i64) -> i64 {
    match n {
        1 | 2 | 3 => 1,
        _ => 0,
    }
}

// P-M3 或模式：各备选绑定同名同序变量
fn or_bind(s: Shape) -> i64 {
    match s {
        Shape::Circle(r) | Shape::Square(r) => r,
        _ => -1,
    }
}

// P-M3 或模式 + 字符范围（解析器字符分类的典型形态）
fn char_class(c: char) -> i64 {
    match c {
        'a'...'z' | 'A'...'Z' => 1,
        '0'...'9' => 2,
        _ => 3,
    }
}

// P-M4 组合：或模式 + 守卫（守卫可引用外层变量）
fn combo(c: char, upper: bool) -> i64 {
    match c {
        'a'...'z' | 'A'...'Z' if upper => 1,
        'a'...'z' | 'A'...'Z' => 2,
        _ => 3,
    }
}

// P-M4 组合：范围模式 + 守卫引用被匹配变量本身
fn range_guard(n: i64) -> i64 {
    match n {
        0...100 if n > 50 => 1,
        0...100 => 2,
        _ => 3,
    }
}

fn main() {
    // 范围：边界与开闭
    println(classify(0));                 // 0
    println(classify(1));                 // 1
    println(classify(9));                 // 1
    println(classify(10));                // 2
    println(classify(19));                // 2
    println(classify(20));                // -1（[10,20) 与 (20,30] 均不含 20）
    println(classify(21));                // 3
    println(classify(30));                // 3
    println(classify(31));                // -1

    // 守卫
    println(guarded(Option::Some(11)));   // 1
    println(guarded(Option::Some(6)));    // 2
    println(guarded(Option::Some(1)));    // 3
    println(guarded(Option::None));       // 4

    // 或模式
    println(or_lit(2));                   // 1
    println(or_lit(4));                   // 0
    println(or_bind(Shape::Circle(7)));   // 7
    println(or_bind(Shape::Square(8)));   // 8
    println(char_class('q'));             // 1
    println(char_class('Q'));             // 1
    println(char_class('5'));             // 2
    println(char_class('+'));             // 3

    // 组合
    println(combo('a', true));            // 1
    println(combo('a', false));           // 2
    println(combo('+', true));            // 3
    println(range_guard(80));             // 1
    println(range_guard(20));             // 2
    println(range_guard(200));            // 3

    // `if let` desugar 为 match，故同样支持或模式
    let o = Option::Some(2);
    if let Option::Some(1) | Option::Some(2) = o {
        println(1);                       // 1
    } else {
        println(0);
    }
}
