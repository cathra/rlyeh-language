// SH-P0-1 E2：repr(C) sub-8 字节字段真布局（C 打包）。
// 验证读取提升（u8/u16/u32→zext、i32→sext）与写入降窄（i64 值→字段宽度 trunc）。
// 字段 C 偏移：a@0 b@2 c@4 d@8 e@12（自然对齐），与 8 字节槽模型（a@0 b@8 c@16 d@24 e@32）不同，
// 但字段访问在两种布局下均读到正确值，故本用例主要验证「窄值读写经转换不丢信息」。
#[repr(C)]
struct Hdr {
    a: u8,
    b: u16,
    c: i32,
    d: u32,
    e: i64,
}

fn main() {
    let mut h = Hdr { a: 1, b: 2, c: 3, d: 4, e: 5 };
    h.a = 250;
    h.b = 60000;
    h.c = -654321;
    h.d = 4000000000;
    h.e = -9876543210987;
    println(h.a);    // 250
    println(h.b);    // 60000
    println(h.c);    // -654321
    println(h.d);    // 4000000000
    println(h.e);    // -9876543210987
}
