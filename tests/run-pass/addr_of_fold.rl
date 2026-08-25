// U5 `&*p` 引用折叠：解引用再取引用直接透传指针（写回原地址生效）
fn main() {
    // `&*b`：Box 内值取引用（b: Box<i64> → 指针值即堆地址）
    let mut b = Box::new(42);
    let p = &*b;
    println(*p);               // 42
    let q = &mut *b;           // 可变引用：写回 Box 堆对象
    *q = 99;
    println(*b);               // 99（写回原地址——折叠语义正确）

    // 引用变量：`&*r` ≡ r 的指针值（Deref 目标取引用）
    let mut x = 7;
    let r = &mut x;
    let s = &*r;
    println(*s);               // 7
    *r = 8;
    println(*s);               // 8（s 与 r 指向同一地址）

    // 不可变 `&expr` 表达式取址（求值到临时槽，读语义正确）
    let e = &(3 * 4);
    println(*e);               // 12

    // Box::leak（T3a）：泄漏堆对象返回裸指针，读写可用
    let l = Box::leak(b);
    println(*l);               // 99
    *l = 100;
    println(*l);               // 100
}
