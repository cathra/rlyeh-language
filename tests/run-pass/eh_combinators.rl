// EH-3（0.2.0-AA，2026-09-21）：Option / Result 组合子补全。
//
// 第一批（非闭包）：Option::ok_or / Result::ok / err / expect_err。
// 第二批（接收函数值）：Option::map / and_then、Result::map / map_err / and_then
//   ——形参为 `fn(..) -> ..` 函数值，方法泛型由实参 fn 类型反推（typecheck
//   `check_method_call` 候选循环此前把含泛型的 fn 形参一律跳过，2026-09-21 修复）。
//
// 已知约束：
// 1. 闭包字面量实参（`|x| ..`）不能反推方法泛型（闭包体类型无法脱离上下文
//    定型），请传具名函数 / `fn` 值；
// 2. 同一函数内同名绑定不得跨类型复用——typecheck 变量环境按名全局索引、无作用域
//    隔离（docs/std-lib.md §12 已知限制），LIR 亦按名记录类型并直接报冲突；
// 3. 嵌套泛型组合子（flatten / transpose / copied / cloned）受
//    `impl<T> Option<Option<T>>`（嵌套泛型 self 类型）不被解析支持的限制，暂缓。

fn inc(x: i64) -> i64 { x + 1 }
fn dbl(x: i64) -> i64 { x + x }
fn even_half(x: i64) -> Option<i64> {
    if x % 2 == 0 { Option::Some(x / 2) } else { Option::None }
}
fn err_if_zero(x: i64) -> Result<i64, String> {
    if x == 0 { Result::Err(String::from("zero")) } else { Result::Ok(x) }
}

fn main() {
    // ---------- 第一批：非闭包组合子 ----------
    // 1. Option::ok_or：Some → Ok
    let a: Option<i64> = Option::Some(7);
    match a.ok_or(0) {
        Result::Ok(va) => println(va),      // 7
        Result::Err(_) => println(-1),
    }
    // 2. Option::ok_or：None → Err（E=i64 由实参推断）
    let b: Option<i64> = Option::None;
    match b.ok_or(42) {
        Result::Ok(vb) => println(vb),
        Result::Err(eb) => println(eb),     // 42
    }
    // 3. Option::ok_or：E=String（聚合载荷错误）
    let c: Option<String> = Option::None;
    match c.ok_or(String::from("missing")) {
        Result::Ok(vc) => println(vc),
        Result::Err(ec) => println(ec),     // missing
    }
    // 4. Result::ok / err（Ok 分支）
    let good: Result<i64, String> = Result::Ok(5);
    println(good.ok().is_some());           // 1
    println(good.err().is_none());          // 1
    // 5. Result::ok / err（Err 分支）
    let bad: Result<i64, String> = Result::Err(String::from("boom"));
    println(bad.ok().is_none());            // 1
    match bad.err() {
        Option::Some(eb2) => println(eb2),  // boom
        Option::None => println(0),
    }
    // 6. Result::expect_err
    let r: Result<i64, i64> = Result::Err(9);
    println(r.expect_err(String::from("should be err")));   // 9

    // ---------- 第二批：接收函数值的组合子 ----------
    // 7. Option::map（Some / None）
    let m1: Option<i64> = Option::Some(5);
    match m1.map(inc) {
        Option::Some(vm) => println(vm),    // 6
        Option::None => println(-1),
    }
    let m2: Option<i64> = Option::None;
    println(m2.map(inc).is_none());         // 1
    // 8. Option::and_then（Some 命中 / 未命中）
    let m3: Option<i64> = Option::Some(10);
    match m3.and_then(even_half) {
        Option::Some(vm3) => println(vm3),  // 5
        Option::None => println(-1),
    }
    let m4: Option<i64> = Option::Some(9);
    println(m4.and_then(even_half).is_none());  // 1
    // 9. Result::map
    let m5: Result<i64, String> = Result::Ok(3);
    match m5.map(dbl) {
        Result::Ok(vm5) => println(vm5),    // 6
        Result::Err(_) => println(-1),
    }
    // 10. Result::map_err（E=i64 → F=String）
    let m6: Result<i64, i64> = Result::Err(7);
    match m6.map_err(int_to_string) {
        Result::Ok(_) => println(-1),
        Result::Err(em6) => println(em6),   // 7
    }
    // 11. Result::and_then（Ok → Err 转换 / Ok 直通）
    let m7: Result<i64, String> = Result::Ok(0);
    match m7.and_then(err_if_zero) {
        Result::Ok(vm7) => println(vm7),
        Result::Err(em7) => println(em7),   // zero
    }
    let m8: Result<i64, String> = Result::Ok(4);
    match m8.and_then(err_if_zero) {
        Result::Ok(vm8) => println(vm8),    // 4
        Result::Err(em8) => println(em8),
    }

    // ---------- 组合使用 ----------
    // 12. map + and_then 链：21 → 22 → Some(11)
    let chain: Option<i64> = Option::Some(21);
    match chain.map(inc).and_then(even_half) {
        Option::Some(vch) => println(vch),  // 11
        Option::None => println(-1),
    }
    // 13. ok_or 用于 option → result 提升
    println(len_or_err(Option::Some(String::from("rlyeh"))));  // 5
    println(len_or_err(Option::None));                         // -1
}

fn len_or_err(s: Option<String>) -> i64 {
    match s.ok_or(String::from("absent")) {
        Result::Ok(vs) => vs.len(),
        Result::Err(_) => -1,
    }
}
