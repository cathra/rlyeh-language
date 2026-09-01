// SH-P0-1 E2：#[repr(C)] 属性解析与结构可用。
// 当前内存布局与默认一致（8 字节对齐字段即 C 兼容）；
// sub-8 字节字段的 C 打包布局待 MIR/LIR/codegen 字段尺寸下传专项落地。
#[repr(C)]
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let p = Point { x: 3, y: 4 };
    println(p.x);
    println(p.y);
}
