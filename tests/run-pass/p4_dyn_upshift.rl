// P4（2026-08-28）：`&dyn Protocol` 上转型——`let d: &dyn Error = r;`（r: &MyError）
// 把具体类型引用上转为胖指针引用 `&dyn Protocol`，虚调用经 vtable/去虚拟化分派。
// 覆盖：&dyn 上转型 + 虚调用 + 去虚拟化。

protocol Error {
    fn message(&self) -> String;
}

struct MyError { code: i64 }

impl MyError: Error {
    fn message(&self) -> String {
        String::from("my error ")
    }
}

fn make() -> MyError {
    MyError { code: 7 }
}

fn main() {
    let e = make();
    let r = &e;
    // &dyn Error 上转型（引用 → 胖指针引用）
    let d: &dyn Error = r;
    // 虚调用：分派到 MyError::message
    println(d.message());
    // H4 值上转型回归
    let d2: dyn Error = &e;
    println(d2.message());
}
