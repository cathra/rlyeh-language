// EH-6（0.2.0-AA，2026-09-21）：集合错误累积——RFC error-handling §4.6 落地验收。
//
// 覆盖：
//  M2 `Vec<Result<T, E>>` 两种聚合语义（`core/module.rl` 的
//     `impl<T, E> Vec<Result<T, E>>`，依赖嵌套泛型 self 类型——EH-3 期间修复）：
//       - `try_collect()` → `Result<Vec<T>, E>`：**首错短路**（对齐 Rust
//         `collect::<Result<Vec<T>, E>>()`）；
//       - `collect_errors()` → `Vec<E>`：忽略 Ok，按原序累积**全部**错误；
//     含全 Ok / 首错 / 多错 / 空集合四种形态。
//  M3 与 `DynError`（EH-4）组合：`Vec<DynError>` 统一聚合异质错误，
//     经 `message()` 逐条读取。
//
// 未落地（见 `docs/tasks/leaf/eh-6-try-aggregation.md`）：M1 `try { .. }` 块——
// 需新增 `try` 关键字 + `?` 的**残差上下文**（当前 `?` 硬编码合成函数级
// `return`，见 `check_question`）+ 验证 `break <value>` 的 codegen 通路。

fn main() {
    // 1. try_collect：全 Ok
    let mut ok_all: Vec<Result<i64, String>> = Vec::new();
    ok_all.push(Result::Ok(1));
    ok_all.push(Result::Ok(2));
    ok_all.push(Result::Ok(3));
    println(ok_all.try_collect().unwrap().len());        // 3

    // 2. try_collect：首错短路
    let mut first_err: Vec<Result<i64, String>> = Vec::new();
    first_err.push(Result::Ok(1));
    first_err.push(Result::Err(String::from("bad")));
    first_err.push(Result::Ok(3));
    println(first_err.try_collect().is_err());           // 1
    match first_err.try_collect() {
        Result::Ok(_) => println(-1),
        Result::Err(e) => println(e),                    // bad
    }

    // 3. collect_errors：聚合全部错误（忽略 Ok）
    let mut multi: Vec<Result<i64, String>> = Vec::new();
    multi.push(Result::Err(String::from("e1")));
    multi.push(Result::Ok(7));
    multi.push(Result::Err(String::from("e2")));
    println(multi.collect_errors().len());               // 2
    for m in multi.collect_errors() {
        println(m);                                      // e1 / e2
    }

    // 4. 空集合
    let empty: Vec<Result<i64, String>> = Vec::new();
    println(empty.try_collect().unwrap().len());         // 0
    println(empty.collect_errors().len());               // 0

    // 5. M3：与 `DynError` 组合（异质错误统一聚合）
    let mut dyns: Vec<Result<i64, DynError>> = Vec::new();
    dyns.push(Result::Ok(1));
    dyns.push(Result::Err(IoError::from_kind(IoErrorKind::NotFound).into_dyn()));
    dyns.push(Result::Err(IoError::from_kind(IoErrorKind::Other).into_dyn()));
    println(dyns.try_collect().is_err());                // 1
    let agg = dyns.collect_errors();
    println(agg.len());                                  // 2
    for d in agg {
        println(d.message());                            // entity not found / io error
    }
}
