// T-5：region 边界借用良构性（合法用例，必须编译运行成功）。
// 守护：合法 region 借用不被 borrowck 误拒。
// 注：各场景使用互不相同的变量名，规避 borrowck 既有「uses 按名索引、不区分同名遮蔽」
// 的潜在缺陷（见 RFC §9.8）；该缺陷与 T-3 无关，留待 borrowck 专项外修复。
// - 引用在 region 内创建并在 region 内使用（不逃逸）→ 合法；
// - region 结束后原变量可重新写入（借用随 region 释放，NLL 语义）；
// - 引用指向 region 外的局部（其生命周期覆盖 region）→ 合法。
fn main() {
    // 1. region 内借用、区内使用、块尾值为 i64（非引用，不逃逸）
    let v1 = region 'r {
        let x1 = 10;
        let r1 = &x1;
        *r1 + 1
    };
    println(v1);             // 11

    // 2. region 内借用结束后，原变量可重新写入
    let mut z2 = 1;
    let w2 = region 'r {
        let rz2 = &z2;
        *rz2
    };
    z2 = 9;                  // region 结束后可写原变量
    println(w2);             // 1
    println(z2);             // 9

    // 3. 引用指向 region 外的局部（生命周期覆盖 region），区内解引用合法
    let a3 = 5;
    let r3 = &a3;
    let inside3 = region 'r {
        *r3 + 1
    };
    println(inside3);        // 6
}
