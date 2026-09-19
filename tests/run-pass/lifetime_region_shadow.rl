// T-5/§9.8 回归：同名遮蔽场景——修复前（uses 按名索引不区分遮蔽）会误报 DanglingReference。
// 两个 `r` 是不同绑定（遮蔽）：前者指向 region 局部、仅在区内使用；后者指向 region 外局部。
// 修复后按绑定实例裁剪 last_use，二者互不干扰，合法代码不被误拒。
fn main() {
    let v = region 'r {
        let x = 10;
        let r = &x;
        *r + 1
    };
    println(v);                    // 11

    let a = 5;
    let r = &a;
    let inside = region 'r { *r + 1 };
    println(inside);               // 6
}
