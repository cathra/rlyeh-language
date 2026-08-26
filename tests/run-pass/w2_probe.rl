// W2 验收：控制流 await + 表达式边界 + 跨 await 非 i64 变量
async fn getv(x: i64) -> i64 {
    x * 2
}

// match 内 await（臂体块 + 通配符，非尾位置）
async fn match_await(x: i64) -> i64 {
    let mut r: i64 = 0;
    match x {
        0 => {
            r = 0;
        }
        _ => {
            r = getv(x).await;
        }
    }
    r
}

// 跨 await 的 String 变量 + let r 尾表达式
async fn str_cross() -> i64 {
    let s: String = String::from("abc");
    let a = getv(3).await;
    let r = s.len() + a;
    r
}

// 跨 await 的 bool 变量
async fn bool_cross() -> i64 {
    let flag: bool = true;
    let a = getv(3).await;
    if flag {
        let r = a + 1;
        r
    } else {
        0
    }
}

fn main() {
    let mut f1 = match_await(4);
    println(block_on(&mut f1));   // 8
    let mut f2 = match_await(0);
    println(block_on(&mut f2));   // 0
    let mut f3 = str_cross();
    println(block_on(&mut f3));   // 3+6=9
    let mut f4 = bool_cross();
    println(block_on(&mut f4));   // 7
}
