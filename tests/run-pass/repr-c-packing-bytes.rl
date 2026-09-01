// SH-P0-1 E2：repr(C) 字节级真布局验证。
// 通过裸指针按字节偏移读取，确认字段按 C 规则打包（a@0, b@4），
// 而非 8 字节槽模型（a@0, b@8）。`*(p+4)` 读到 0xEE（b 的最低字节）
// 即证明 b 落位在偏移 4；8 槽模型下 b 在偏移 8，偏移 4 为填充 0x00。
#[repr(C)]
struct Hdr { a: u8, b: u32 }

fn main() {
    let h = Hdr { a: 0xAA, b: 0xBBCCDDEE };
    unsafe {
        let p = &h as *const Hdr as *const u8;
        // 偏移 0 = a（0xAA = 170）
        println(*(p) as u32);          // 170
        // 偏移 4 = b 小端最低字节（0xEE = 238）。打包证据。
        println(*(p + 4) as u32);     // 238
    }
}
