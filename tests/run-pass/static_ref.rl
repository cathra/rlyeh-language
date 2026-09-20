// M3（0.2.0-V）：对 `static` 全局取址 `&GLOBAL` 产出 `&'static T`，可解引用读写。
static X: i64 = 42;
static mut C: i64 = 7;

fn main() {
    let p = &X;
    println(*p);                       // 42

    let q: &'static i64 = &X;          // 显式 'static 注解
    println(*q);                       // 42

    unsafe {
        let r = &mut C;                // &'static mut T（unsafe 内）
        *r = 10;
        println(*r);                   // 10
        println(C);                    // 10
    }
}
