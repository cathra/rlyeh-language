struct Point { x: i64, y: i64 }

fn main() {
    let mut a: i64 = 1;
    // 第二个参数类型需与 *a 的元素类型一致（i64），传入 Point 应报错
    let _ = mem::replace(&mut a, Point { x: 1, y: 2 });
}
// expect: mem::replace 第二个参数类型需为 T
