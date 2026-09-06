// expect: does not live long enough
// lang-defects #9：按值参数返回 &param.field，引用指向已出栈的参数槽，悬垂。
struct Foo { v: i64 }
fn get(p: Foo) -> &i64 { &p.v }
fn main() {
    let f = Foo { v: 1 };
    println(*get(f));
}
