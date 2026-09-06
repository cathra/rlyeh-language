// lang-defects #9 反向保证：&self（按引用接收）返回 &self.field 指向调用方内存，合法。
struct Foo { v: i64 }
impl Foo {
    fn get(&self) -> &i64 { &self.v }
}
fn main() {
    let f = Foo { v: 42 };
    let r = f.get();
    println(*r);
}
