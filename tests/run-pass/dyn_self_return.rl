// G-M1：经 `dyn Protocol` 调用含 `Self` 签名的方法。
// devirtualize 时把签名中的 `Self` 替换为 dyn 绑定源的具体类型：
// - 形参 `&Self` 收敛为 `&P`；
// - 返回 `Self` 聚合收敛为 `P`（按值返回；经 vtable 取址故退化为
//   i8* 返回 ABI，函数体与声明一致——见 rlyeh-codegen llvm_ctor 4b-iv）。
protocol Combine {
    fn combine(&self, other: &Self) -> i64;
}
protocol Copyable {
    fn make(&self) -> Self;
}
struct P { x: i64 }
impl P: Combine {
    fn combine(&self, other: &Self) -> i64 { self.x + other.x }
}
impl P: Copyable {
    fn make(&self) -> Self { P { x: self.x } }
}
fn main() {
    let a = P { x: 10 };
    let b = P { x: 20 };

    // 形参含 `Self`：`&Self` → `&P`
    let d: dyn Combine = &a;
    let r = d.combine(&b);       // 30
    println(r);

    // 按值返回 `Self` 聚合：`Self` → `P`
    let p = P { x: 7 };
    let c: dyn Copyable = &p;
    let m = c.make();            // P { x: 7 }
    println(m.x);                // 7

    if r == 30 && m.x == 7 {
        println(1);              // 1
    } else {
        println(0);
    }
}
