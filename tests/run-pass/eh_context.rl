// EH-7（0.2.0-AA，2026-09-21）：错误上下文 / 背链附加——RFC error-handling §4.7 验收。
//
// 覆盖：
//  1. `Result<T, DynError>::context(msg)`：Err 分支附加语义上下文；
//  2. 逐层 `context` **累积**为 `outer: inner: root` 可读背链
//     （`DynError::context` 把当前 DynError 装箱 → `&*b` → 上转 `dyn Error` → 重新包装；
//     固有 `DynError::message` 与 `impl DynError: Error::message` 同语义，
//     后者为 vtable 分派目标，经 `Box<dyn Error>` 视图读取时同样保留上下文）；
//  3. `?` 传播保持已附加的上下文；
//  4. `with_context(f)` 惰性求值（`fn() -> String`，须无捕获——H2 限制）；
//  5. `Ok` 直通（不附加、不影响值）；
//  6. 附加上下文后 `source()` 仍直达根因（此处 IoError::source → None）。
//
// 位置注记：`impl<T> Result<T, DynError>` 定义在 std 根单元（`rlyeh/module.rl`）——
// `collect_impl` 以「模块前缀 + impl 类型名」构造 self 类型，写在子模块会得到
// `io::error::Result`，与根单元的 `Result` 不匹配。

fn root_fail() -> Result<i64, DynError> {
    Result::Err(IoError::from_kind(IoErrorKind::NotFound).into_dyn())
}

// `?` 传播：返回类型同为 `Result<_, DynError>`，已附加的上下文保留。
fn read_cfg() -> Result<i64, DynError> {
    let v = root_fail().context(String::from("read config"))?;
    Result::Ok(v)
}

// 两层上下文（模拟调用栈：startup → read config → root）。
fn startup() -> Result<i64, DynError> {
    let v = read_cfg().context(String::from("startup"))?;
    Result::Ok(v)
}

fn main() {
    // 1 + 2. 单层 / 两层上下文
    match root_fail().context(String::from("read config")) {
        Result::Ok(_) => println(-1),
        Result::Err(e1) => println(e1.message()),      // read config: entity not found
    }
    match startup() {
        Result::Ok(_) => println(-1),
        Result::Err(e2) => {
            println(e2.message());                     // startup: read config: entity not found
            println(e2.source().is_none());            // 1（source 仍直达根因）
        }
    }
    // 3. `?` 传播保持上下文
    println(read_cfg().is_err());                      // 1
    match read_cfg() {
        Result::Ok(_) => println(-1),
        Result::Err(e3) => println(e3.message()),      // read config: entity not found
    }
    // 4. `with_context` 惰性求值
    match root_fail().with_context(|| String::from("lazy ctx")) {
        Result::Ok(_) => println(-1),
        Result::Err(e4) => println(e4.message()),      // lazy ctx: entity not found
    }
    // 5. `Ok` 直通
    let okv: Result<i64, DynError> = Result::Ok(5);
    println(okv.context(String::from("unused")).unwrap());   // 5
}
