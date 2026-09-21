// `try { .. }` 错误聚合块（EH-6 M1，2026-09-21）。
//
// desugar：`try { <stmts>; <tail> }` → `loop { <stmts>; break <Kind>::<OkVariant>(<tail>) }`，
// 块内 `?` 改为残留式 `break`（失败值就地跳出该块，而非整个函数）。块的类型 =
// 外层函数的错误位类型（`Result<T, E_fn>` / `Option<T>`）。本测试覆盖成功 / 首个
// 错误 / 第二个 `?` 失败 三条路径，以及 `Option` 形态与「try 块结果可当作普通
// `Result` / `Option` 直接使用」。
fn do_step(n: i64) -> Result<i64, String> {
    if n < 0 {
        Result::Err(String::from("neg"))
    } else {
        Result::Ok(n * 2)
    }
}

fn do_step2(n: i64) -> Result<i64, String> {
    if n > 10 {
        Result::Err(String::from("big"))
    } else {
        Result::Ok(n + 1)
    }
}

// Result 形态：块内两次 `?`，任一处失败 → 块整体返回 Err（聚合首个错误）。
fn risky(op: i64) -> Result<i64, String> {
    let r = try {
        let x = do_step(op)?;
        let y = do_step2(x)?;
        x + y
    };
    r
}

// Option 形态：`?` 作用于 `Option`，失败 → 块整体返回 `None`。
fn first_some(v: i64) -> Option<i64> {
    let r = try {
        let x = if v > 0 { Some(v) } else { None }?;
        let y = if x < 100 { Some(x * 2) } else { None }?;
        x + y
    };
    r
}

fn main() {
    // 1. 全成功：do_step(3)=6，do_step2(6)=7，尾值 = 6+7 = 13
    let a = risky(3);
    match a {
        Result::Ok(v) => println(v),        // 13
        Result::Err(e) => println(e),
    }

    // 2. 首个 `?` 失败：do_step(-1) = Err("neg")
    let b = risky(-1);
    match b {
        Result::Ok(v) => println(v),
        Result::Err(e) => println(e),      // neg
    }

    // 3. 第二个 `?` 失败：do_step(6)=12，do_step2(12) = Err("big")
    let c = risky(6);
    match c {
        Result::Ok(v) => println(v),
        Result::Err(e) => println(e),      // big
    }

    // 4. Option 成功：v=5 → x=5, y=10, 尾值=15
    let d = first_some(5);
    match d {
        Option::Some(v) => println(v),     // 15
        Option::None => println(-1),
    }

    // 5. Option 失败：v=0 → `if v>0` 为 false → None → 块整体 None
    let e = first_some(0);
    match e {
        Option::Some(v) => println(v),
        Option::None => println(0),        // 0
    }

    // 6. try 块结果当作普通 Result 直接返回给调用方（不立即 match）
    let f = risky(2);                      // do_step(2)=4, do_step2(4)=5, 尾=9
    match f {
        Result::Ok(v) => println(v),       // 9
        Result::Err(_) => println(-1),
    }
}
