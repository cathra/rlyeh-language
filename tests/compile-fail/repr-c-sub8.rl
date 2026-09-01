// SH-P0-1 E2：repr(C) 含非 repr(C) 嵌套结构体——应拒绝（嵌套聚合内联要求内层同样 C 布局）
// expect: repr(C)
// expect: 嵌套结构体
struct Inner { x: i64, y: i64 }

#[repr(C)]
struct Outer {
    a: i64,
    b: Inner,   // Inner 未标注 #[repr(C)] → 应报错
    c: i64,
}

fn main() {}
