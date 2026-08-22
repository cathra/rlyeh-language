// K2 Box<T> 类型注解：函数参数 / 返回值 / 变量声明 / 解引用（编译通过）
fn add_box(b: Box<i64>) -> i64 {
    *b + 1
}

fn main() {
    let b: Box<i64> = Box::new(41);
    let r = add_box(b);
    println(r);
}
