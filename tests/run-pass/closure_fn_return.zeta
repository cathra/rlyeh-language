// H5 补全：返回闭包的函数 + 闭包值作 fn 实参。
// - `fn make() -> fn(..) { |x| .. }`：返回类型为 fn 且尾表达式为无捕获闭包时，
//   按 H2 无捕获闭包签名检查（`check_closure_expected`），生成函数指针返回；
// - `let f = |x: i64| ..; f` 作返回值：无捕获闭包值降级为 fn 指针；
// - `let f = |x| ..; f` 作返回值：未固化闭包按返回签名固化参数类型；
// - 闭包值（已注解/未注解，均无捕获）作 fn 形参实参，降级/固化为函数指针。
fn apply(f: fn(i64) -> i64, x: i64) -> i64 { f(x) }

// 尾表达式为无捕获闭包（H2 签名检查）
fn make() -> fn(i64) -> i64 {
    |x| x + 1
}

// 无捕获闭包值变量作返回值（降级为 fn 指针）
fn make2() -> fn(i64) -> i64 {
    let f = |x: i64| x * 2;
    f
}

// 未固化延迟闭包作返回值（按返回签名固化参数类型）
fn make3() -> fn(i64) -> i64 {
    let f = |x| x * 3;
    f
}

fn main() {
    let m = make();
    println(m(41));                 // 42
    println(apply(m, 41));          // 42

    let m2 = make2();
    println(m2(21));                // 42

    let m3 = make3();
    println(apply(m3, 14));         // 42

    // 已注解闭包值作 fn 实参（无捕获 → 降级为函数指针）
    let c = |x: i64| x + 1;
    println(apply(c, 41));          // 42

    // 未注解闭包值作 fn 实参（按 fn 形参签名固化）
    let d = |x| x - 1;
    println(apply(d, 43));          // 42
}
