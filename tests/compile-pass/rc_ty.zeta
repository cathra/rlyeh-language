// K3 Rc<T> 类型注解：函数参数 / 返回值 / 变量声明 / 解引用（编译通过）
fn twice_rc(r: Rc<i64>) -> i64 {
    *r * 2
}

fn main() {
    let r: Rc<i64> = Rc::new(21);
    println(twice_rc(r));
    let a: Arc<i64> = Arc::new(5);
    println(*a);
}
