// SH-P0-6：`if let` / `while let` 模式控制流。
// 纯语法糖——parser 层 desugar 为既有 `match` / `loop`（零新增 IR 节点）：
//   if let Pat = e { A } else { B }  ⟶  match e { Pat => { A }, _ => { B } }
//   while let Pat = e { A }          ⟶  loop { match e { Pat => { A }, _ => break } }
fn get(i: i64) -> Option<i64> {
    if i > 0 { Option::Some(i) } else { Option::None }
}

fn main() {
    // 1) 匹配成功：绑定 v = 5
    if let Option::Some(v) = get(5) {
        println(v);                  // 5
    } else {
        println(-1);
    }

    // 2) 匹配失败走 else（`Option::None` 落到 `_` 臂）
    if let Option::Some(v) = get(0) {
        println(v);
    } else {
        println(-1);                 // -1
    }

    // 3) 无 else：不匹配即跳过（兜底空块）
    if let Option::Some(v) = get(5) {
        println(v);                  // 5
    }

    // 4) else if 链
    let n = 0;
    if let Option::Some(v) = get(n) {
        println(v);
    } else if n == 0 {
        println(100);                // 100
    } else {
        println(-2);
    }

    // 5) 嵌套 if let
    if let Option::Some(a) = get(3) {
        if let Option::Some(b) = get(4) {
            println(a + b);          // 7
        }
    }

    // 6) 作表达式使用（match 即表达式）
    let o = Option::Some(9);
    let v = if let Option::Some(x) = o { x } else { 0 };
    println(v);                      // 9

    // 7) 字面量模式
    if let 7 = n + 7 {
        println(1);                  // 1
    } else {
        println(0);
    }

    // 8) while let：通道取到 None（关闭且空）为止
    let pair = channel::<i64>();
    let mut tx = pair.tx;
    let mut rx = pair.rx;
    tx.send(1);
    tx.send(2);
    tx.close();
    let mut sum = 0;
    while let Option::Some(got) = rx.recv() {
        sum = sum + got;
    }
    println(sum);                    // 3

    // 9) else if let 链：首臂不匹配时继续试下一个模式
    if let Option::Some(v) = get(0) {
        println(v);
    } else if let Option::Some(w) = get(6) {
        println(w);                  // 6
    } else {
        println(-3);
    }

    // 10) while let 体内的 continue / break（desugar 为 loop，语义同 Rust）
    let p2 = channel::<i64>();
    let mut tx2 = p2.tx;
    let mut rx2 = p2.rx;
    tx2.send(1);
    tx2.send(2);
    tx2.send(3);
    tx2.send(4);
    tx2.close();
    let mut acc = 0;
    while let Option::Some(x) = rx2.recv() {
        if x == 2 {
            continue;                // 跳过 2
        }
        if x == 4 {
            break;                   // 提前退出
        }
        acc = acc + x;
    }
    println(acc);                    // 1 + 3 = 4
}
