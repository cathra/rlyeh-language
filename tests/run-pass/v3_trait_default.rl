// V3 trait 默认方法（2026-08-26）：
// 1. trait 方法带 body（默认实现），impl 显式实现时覆盖默认
// 2. impl 未实现时回退到 trait 默认实现
trait Greeter {
    fn greeting(&self) -> String { String::from("hello") }
    fn loud(&self) -> String { String::from("HELLO!") }
}

struct Foo {
    x: i64,
}

impl Greeter for Foo {
    fn greeting(&self) -> String { String::from("foo-hi") }
    fn loud(&self) -> String { String::from("FOO!") }
}

struct Bar {
    x: i64,
}

impl Greeter for Bar {
    // 仅实现 greeting，loud 走 trait 默认实现
    fn greeting(&self) -> String { String::from("bar-hi") }
}

struct Baz {
    x: i64,
}

impl Greeter for Baz {
    // 一个方法都不实现，全部回退默认
}

fn main() {
    // 1. Foo：两者都显式实现
    let f = Foo { x: 1 };
    println(f.greeting()); // foo-hi
    println(f.loud()); // FOO!

    // 2. Bar：greeting 显式、loud 回退默认
    let b = Bar { x: 2 };
    println(b.greeting()); // bar-hi
    println(b.loud()); // HELLO!（回退默认）

    // 3. Baz：全部回退默认
    let z = Baz { x: 3 };
    println(z.greeting()); // hello（回退默认）
    println(z.loud()); // HELLO!（回退默认）
}
