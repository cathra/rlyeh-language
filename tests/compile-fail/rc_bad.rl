// 期望编译失败：Rc::new 参数个数错误
// expect: Rc::new
fn main() {
    let r = Rc::new();
    println(*r);
}
