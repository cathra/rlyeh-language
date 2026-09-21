// EH-3（0.2.0-AA，2026-09-21）：Option / Result 组合子补全。
//
// 第一批（非闭包）：Option::ok_or / Result::ok / err / expect_err。
// 第二批（接收函数值）：Option::map / and_then、Result::map / map_err / and_then
//   ——形参为 `fn(..) -> ..` 函数值，方法泛型由实参 fn 类型反推（typecheck
//   `check_method_call` 候选循环此前把含泛型的 fn 形参一律跳过，2026-09-21 修复）。
//
// 第三批（嵌套泛型）：Option::flatten / Result::flatten / Option::transpose /
//   Result::transpose——依赖 2026-09-21 的「parser 保留 impl 泛型实参 +
//   typecheck 按实参构建嵌套 self 类型」修复（此前 `impl<T> Option<Option<T>>`
//   连解析都失败）。
// 第四批（引用侧）：Option::copied / cloned、Result::copied / cloned
//   （`impl<T> Option<&T>` / `impl<T, E> Result<&T, E>`；`cloned` 需 `T: Clone`）。
// 第五批（惰性 / 闭包字面量）：Option::unwrap_or_else / or_else / ok_or_else /
//   filter、Result::unwrap_or_else / or_else——依赖 2026-09-21 两项修复：
//   ① parser 补 `|| expr` 零参闭包前缀分派（此前 `||` 在表达式位置报
//      `unexpected token: found OrOr`）；
//   ② typecheck 对「泛型仅出现在闭包返回位置」的 fn 形参按闭包体定型并回填
//      （`check_closure_expected` + `check_method_call` 实参循环）。
//   同时放宽了旧的「闭包字面量不能反推方法泛型」限制：形参泛型已由接收者 /
//   其它实参可推时（`Option::map(|x| ..)`、`Result::map_err(|e| ..)`），
//   闭包字面量可直接使用。
//
// 已知约束：
// 1. 闭包捕获外部变量仍不支持（H3 规划，报 TC025）；方法泛型若只出现在
//    **闭包形参**位置且接收者无法反推时，仍无法定型，请传具名函数 / `fn` 值；
// 2. 同一函数内同名绑定不得跨类型复用——typecheck 变量环境按名全局索引、无作用域
//    隔离（docs/std-lib.md §12 已知限制），LIR 亦按名记录类型并直接报冲突。

fn inc(x: i64) -> i64 { x + 1 }
fn dbl(x: i64) -> i64 { x + x }
fn even_half(x: i64) -> Option<i64> {
    if x % 2 == 0 { Option::Some(x / 2) } else { Option::None }
}
fn err_if_zero(x: i64) -> Result<i64, String> {
    if x == 0 { Result::Err(String::from("zero")) } else { Result::Ok(x) }
}

// 第四批（引用侧组合子）所用类型：`cloned` 需 `T: Clone`。
#[derive(Clone)]
struct Pair { a: i64 }

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

    // ---------- 第三批：嵌套泛型组合子 ----------
    // （依赖 parser 保留 impl 泛型实参 + typecheck 按实参构建嵌套 self 类型）
    // 14. Option::flatten：Some(Some) / Some(None) / None
    let f1: Option<Option<i64>> = Option::Some(Option::Some(5));
    match f1.flatten() {
        Option::Some(vf1) => println(vf1),  // 5
        Option::None => println(-1),
    }
    let f2: Option<Option<i64>> = Option::Some(Option::None);
    println(f2.flatten().is_none());        // 1
    let f3: Option<Option<i64>> = Option::None;
    println(f3.flatten().is_none());        // 1
    // 15. Result::flatten：Ok(Ok) / Ok(Err)
    let f4: Result<Result<i64, String>, String> = Result::Ok(Result::Ok(3));
    match f4.flatten() {
        Result::Ok(vf4) => println(vf4),    // 3
        Result::Err(_) => println(-1),
    }
    let f5: Result<Result<i64, String>, String> = Result::Ok(Result::Err(String::from("inner")));
    match f5.flatten() {
        Result::Ok(_) => println(-1),
        Result::Err(ef5) => println(ef5),   // inner
    }
    // 16. Option<Result>::transpose：Some(Ok) / None
    let t1: Option<Result<i64, String>> = Option::Some(Result::Ok(7));
    match t1.transpose() {
        Result::Ok(vt1) => match vt1 {
            Option::Some(vt1i) => println(vt1i),  // 7
            Option::None => println(-1),
        },
        Result::Err(_) => println(-1),
    }
    let t2: Option<Result<i64, String>> = Option::None;
    match t2.transpose() {
        Result::Ok(_) => println(88),       // Ok(None) → 88
        Result::Err(_) => println(-1),
    }
    // 17. Result<Option>::transpose：Ok(Some) / Err
    let t3: Result<Option<i64>, String> = Result::Ok(Option::Some(8));
    match t3.transpose() {
        Option::Some(rt3) => match rt3 {
            Result::Ok(vt3) => println(vt3),      // 8
            Result::Err(_) => println(-1),
        },
        Option::None => println(-1),
    }
    let t4: Result<Option<i64>, String> = Result::Err(String::from("boom"));
    match t4.transpose() {
        Option::Some(rt4) => match rt4 {
            Result::Ok(_) => println(-1),
            Result::Err(et4) => println(et4),     // boom
        },
        Option::None => println(-1),
    }

    // ---------- 第四批：引用侧组合子（`&T` → `T`）----------
    let src = 7;
    let rs = &src;
    // 18. Option::copied / none
    let c1: Option<&i64> = Option::Some(rs);
    match c1.copied() {
        Option::Some(vc1) => println(vc1),    // 7
        Option::None => println(-1),
    }
    let c2: Option<&i64> = Option::None;
    println(c2.copied().is_none());           // 1
    // 19. Option::cloned（T: Clone）
    let pr = Pair { a: 9 };
    let rp = &pr;
    let c3: Option<&Pair> = Option::Some(rp);
    println(c3.cloned().is_some());           // 1
    // 20. Result::copied
    let c4: Result<&i64, String> = Result::Ok(rs);
    match c4.copied() {
        Result::Ok(vc4) => println(vc4),      // 7
        Result::Err(_) => println(-1),
    }
    // 21. Result::cloned
    let c5: Result<&Pair, String> = Result::Ok(rp);
    println(c5.cloned().is_ok());             // 1

    // ---------- 第五批：惰性 / 闭包字面量组合子 ----------
    // 22. Option::unwrap_or_else（None → 惰性默认值；Some → 原值）
    let e1: Option<i64> = Option::None;
    println(e1.unwrap_or_else(|| 7));         // 7
    let e2: Option<i64> = Option::Some(3);
    println(e2.unwrap_or_else(|| 7));         // 3
    // 23. Option::or_else（None → 替代；Some → 原值）
    let e3: Option<i64> = Option::None;
    match e3.or_else(|| Option::Some(9)) {
        Option::Some(ve3) => println(ve3),    // 9
        Option::None => println(-1),
    }
    let e4: Option<i64> = Option::Some(4);
    match e4.or_else(|| Option::Some(9)) {
        Option::Some(ve4) => println(ve4),    // 4
        Option::None => println(-1),
    }
    // 24. Option::ok_or_else（E 仅在闭包返回位置 → 由闭包体回填）
    let e5: Option<i64> = Option::None;
    println(e5.ok_or_else(|| String::from("missing")).is_err());  // 1
    let e6: Option<i64> = Option::Some(5);
    println(e6.ok_or_else(|| String::from("missing")).unwrap());  // 5
    // 25. Result::unwrap_or_else（Err → 由错误值派生默认值）
    let e7: Result<i64, i64> = Result::Err(1);
    println(e7.unwrap_or_else(|err| err + 100));                  // 101
    let e8: Result<i64, i64> = Result::Ok(5);
    println(e8.unwrap_or_else(|err| err + 100));                  // 5
    // 26. Result::or_else（Err → 错误恢复；F 由闭包体回填）
    let e9: Result<i64, String> = Result::Err(String::from("boom"));
    match e9.or_else(|err| {
        let n = err.len();
        Result::Ok(n)
    }) {
        Result::Ok(ve9) => println(ve9),      // 4
        Result::Err(_) => println(-1),
    }
    let e10: Result<i64, String> = Result::Ok(6);
    match e10.or_else(|err| Result::Ok(err.len())) {
        Result::Ok(ve10) => println(ve10),    // 6
        Result::Err(_) => println(-1),
    }
    // 27. 闭包字面量直接用于形参泛型可由接收者反推的组合子（旧限制已放宽）
    let e11: Option<i64> = Option::Some(5);
    println(e11.map(|x| x + 1).unwrap());                         // 6
    let e12: Result<i64, String> = Result::Ok(3);
    println(e12.map(|x| x * 2).unwrap());                         // 6
    // 28. Option::filter（形参无方法级泛型：闭包字面量与 fn 值均可用）
    let e13: Option<i64> = Option::Some(5);
    println(e13.filter(|x| x > 1).is_some());                     // 1
    let e14: Option<i64> = Option::Some(0);
    println(e14.filter(|x| x > 1).is_none());                     // 1
    let e15: Option<i64> = Option::None;
    println(e15.filter(positive).is_none());                      // 1
    let e16: Option<i64> = Option::Some(9);
    match e16.filter(positive) {
        Option::Some(ve16) => println(ve16),  // 9
        Option::None => println(-1),
    }
}

fn positive(x: i64) -> bool { x > 1 }

fn len_or_err(s: Option<String>) -> i64 {
    match s.ok_or(String::from("absent")) {
        Result::Ok(vs) => vs.len(),
        Result::Err(_) => -1,
    }
}
