// EH-3（0.2.0-AA，2026-09-21）：Option / Result 非闭包组合子补全。
// 覆盖：Option::ok_or / Result::ok / Result::err / Result::expect_err。
//
// 已知约束：同一函数内同名绑定不得出现不同类型——typecheck 变量环境按名全局
// 索引、无作用域隔离（见 docs/std-lib.md §12 已知限制），LIR 亦按名记录局部
// 类型并直接报 `变量 x 类型冲突`。故本用例各分支使用唯一变量名。
//
// 暂缓项：接收函数值的组合子（map / and_then / or_else / unwrap_or_else /
// ok_or_else）受「方法级泛型仅出现在 fn(..) 形参中时无法推断」限制；嵌套泛型
// 组合子（flatten / transpose / copied / cloned）受 `impl<T> Option<Option<T>>`
// （嵌套泛型 self 类型）不被解析支持的限制。详见 eh-3 叶子。

fn main() {
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

    // 4. Result::ok / err（Ok 分支）：ok → Some，err → None
    let good: Result<i64, String> = Result::Ok(5);
    println(good.ok().is_some());           // 1
    println(good.err().is_none());          // 1

    // 5. Result::ok / err（Err 分支）：ok → None，err → Some(e)
    let bad: Result<i64, String> = Result::Err(String::from("boom"));
    println(bad.ok().is_none());            // 1
    match bad.err() {
        Option::Some(eb2) => println(eb2),  // boom
        Option::None => println(0),
    }

    // 6. Result::expect_err：Err → e
    let r: Result<i64, i64> = Result::Err(9);
    println(r.expect_err(String::from("should be err")));   // 9

    // 7. ok_or 用于 option → result 提升
    println(len_or_err(Option::Some(String::from("rlyeh"))));  // 5
    println(len_or_err(Option::None));                         // -1
}

fn len_or_err(s: Option<String>) -> i64 {
    match s.ok_or(String::from("absent")) {
        Result::Ok(vs) => vs.len(),
        Result::Err(_) => -1,
    }
}
