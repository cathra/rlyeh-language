// SH-P0-1 验证：Rlyeh 手动 bump 分配器（unsafe + 裸指针 + 指针算术）。
// 对拍 rlyeh-region-alloc 的 bump 路径：单游标在连续内存上前推，返回互不重叠的区间。
struct Bump {
    ptr: *mut u8,
    end: *mut u8,
}

impl Bump {
    fn alloc(&mut self, size: i64) -> *mut u8 {
        let result = self.ptr;
        self.ptr = self.ptr + size;
        result
    }
}

fn main() {
    let mut arena = [
        0u8, 0u8, 0u8, 0u8,
        0u8, 0u8, 0u8, 0u8,
        0u8, 0u8, 0u8, 0u8,
    ];
    let base: *mut u8 = &arena[0];
    let mut b = Bump { ptr: base, end: base + 12 };
    unsafe {
        let p0 = b.alloc(4);
        let p1 = b.alloc(4);
        let p2 = b.alloc(4);
        *p0 = 100;
        *p1 = 200;
        *p2 = 300;
        println(*p0);
        println(*p1);
        println(*p2);
    }
}
