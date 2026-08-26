// V1 编译器能力（2026-08）：`&obj.field` / `&arr[i]` 真实取址（GEP 地址）——
// `&` / `&mut` 作用于字段 / 索引目标时生成真实槽地址，写回经 DerefWrite
// 直达原字段 / 原元素（替代 U5 的"求值到临时槽再取址"拷贝语义）。
// 配套：MIR AddrOfField / PtrAdd 指令 + codegen FieldAddr / PtrAdd 发射。
fn main() {
    // &arr[i]：索引取址（读，GEP base + index*8）
    let arr = [10, 20, 30];
    let p = &arr[1];
    println(*p); // 20

    // &mut arr[i]：写回原元素（DerefWrite 到 GEP 地址）
    let mut arr2 = [1, 2, 3];
    let q = &mut arr2[2];
    *q = 99;
    println(arr2[2]); // 99

    // &mut obj.field：写回原字段（AddrOfField GEP 到字段槽）
    let mut pt = Point { x: 5, y: 6 };
    let r = &mut pt.y;
    *r = 7;
    println(pt.y); // 7

    // 指针算术：ptr + n（裸指针偏移，元素步长 8；&T → *const T 互视赋值）
    let arr3 = [4, 5, 6];
    let base: *const i64 = &arr3[0];
    let second = base + 1;
    println(*second); // 5

    // 索引变量取址：&arr[i] 的 i 为变量（PtrAdd 经运行时索引）
    let arr4 = [100, 200, 300];
    let mut i = 2;
    let pv = &arr4[i];
    println(*pv); // 300
    i = 0;
    let pv0 = &arr4[i];
    println(*pv0); // 100

    // 字段链 + 索引：&v.data[0]（迭代器瘦指针构造路径：聚合字段 Ptr 透传 + PtrAdd）
    let mut v = Vec::new();
    v.push(11);
    v.push(22);
    let e0 = &v.data[0];
    println(*e0); // 11

    // &mut v.data[i]：可变索引写回（瘦指针 IterMut 构造路径）
    let mut v2 = Vec::new();
    v2.push(1);
    v2.push(2);
    let e1 = &mut v2.data[1];
    *e1 = 55;
    println(v2[1]); // 55
}

struct Point { x: i64, y: i64 }
