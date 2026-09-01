// SH-P0-1 E2：repr(C) 含 sub-8 字节字段——真布局（C 打包）尚未实现，应报错。
// expect: repr(C)
// expect: 真布局
#[repr(C)]
struct Header {
    magic: u8,
    len: u32,
}

fn main() {
    let h = Header { magic: 1, len: 2 };
    println(h.len);
}
