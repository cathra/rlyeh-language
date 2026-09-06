// expect: does not live long enough
// lang-defects #9：方法按值接收 self，返回 &self.field 指向已销毁的 self 槽，悬垂。
struct Foo { v: i64 }
impl Foo {
    fn get(self) -> &i64 { &self.v }
}
fn main() {
    let f = Foo { v: 42 };
    println(*f.get());
}
