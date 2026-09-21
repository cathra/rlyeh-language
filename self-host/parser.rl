// M-M2（SH-P2-7）Rlyeh 版 parser 切片1（M-M2a）：表达式 → 规范 S-表达式 AST 文本。
//
// 与 `rlyeh run <f> --emit ast-canonical` 的 Rust oracle（driver::emit_ast_canonical_expr）
// 逐字节对齐，由 `crates/rlyeh-driver/tests/self_host_parser.rs` 差分对拍。
//
// 设计要点：
//   - 自包含：直接对源码做最小内联词法（数字/标识符/运算符/括号/空白），不依赖 lexer 产物。
//   - 无递归：用 shunting-yard 生成 RPN，再迭代建树（规避 Rlyeh 的共享 Vec 传递/递归语义问题）。
//   - 括号透明（不产生节点），与 oracle 的 AST（无 Paren 节点）一致。
//
// 切片1 范围：整数/标识符/一元负号/括号/二元 + - * / % 与比较(< <= > >= == !=)/
// 逻辑(&& ||)/位(& | ^ << >>)；其余字符静默跳过（切片2+ 扩展）。

fn is_digit(c: i64) -> i64 {
    if c >= 48 && c <= 57 { 1 } else { 0 }
}

fn is_ident_start(c: i64) -> i64 {
    if c == 95 { return 1; }                 // _
    if c >= 65 && c <= 90 { return 1; }      // A-Z
    if c >= 97 && c <= 122 { return 1; }     // a-z
    0
}

fn is_ident_cont(c: i64) -> i64 {
    if is_ident_start(c) == 1 { return 1; }
    if c >= 48 && c <= 57 { return 1; }      // 0-9
    0
}

// 运算符优先级（仅决定建树形状，不影响输出文本；需与 Rust oracle 的绑定顺序一致）：
// 一元负(100) > * / %(90) > + -(80) > << >>(70) > 比较(60) > & (40) > ^ (35) > | (30) > && (20) > || (10)
fn prec_of(op: String) -> i64 {
    if op == "u-" { return 100; }
    if op == "mul" { return 90; }
    if op == "div" { return 90; }
    if op == "mod" { return 90; }
    if op == "add" { return 80; }
    if op == "sub" { return 80; }
    if op == "shl" { return 70; }
    if op == "shr" { return 70; }
    if op == "lt" { return 60; }
    if op == "le" { return 60; }
    if op == "gt" { return 60; }
    if op == "ge" { return 60; }
    if op == "eq" { return 60; }
    if op == "ne" { return 60; }
    if op == "bitand" { return 40; }
    if op == "bitxor" { return 35; }
    if op == "bitor" { return 30; }
    if op == "and" { return 20; }
    if op == "or" { return 10; }
    0
}

// 解析源码，返回规范 S-表达式 AST 文本（如 `(bin add (int 1) (int 2))`）。
fn parse(src: String) -> String {
    let n = src.len;
    let mut i = 0;
    let mut opstack: Vec<String> = Vec::new();
    let mut rpn: Vec<String> = Vec::new();
    let mut prev_op = 0;   // 1 = 刚读到操作数（期待运算符）；0 = 期待操作数（可遇一元负）
    while i < n {
        let c = src.get(i);
        // 空白
        if c == 32 || c == 9 || c == 13 || c == 10 { i = i + 1; continue; }
        // 整数
        if c >= 48 && c <= 57 {
            let start = i;
            while i < n && is_digit(src.get(i)) == 1 { i = i + 1; }
            let num = src.substring(start, i);
            rpn.push("(int " + num + ")");
            prev_op = 1;
            continue;
        }
        // 标识符
        if is_ident_start(c) == 1 {
            let start = i;
            while i < n && is_ident_cont(src.get(i)) == 1 { i = i + 1; }
            let name = src.substring(start, i);
            rpn.push("(var " + name + ")");
            prev_op = 1;
            continue;
        }
        // 左括号
        if c == 40 {
            opstack.push(String::from("("));
            i = i + 1;
            prev_op = 0;
            continue;
        }
        // 右括号：弹到 (
        if c == 41 {
            while opstack.len() > 0 {
                let t = opstack.pop();
                let v = match t { Option::Some(x) => x, Option::None => String::new() };
                if v == "" { break; }
                if v == "(" { break; }
                rpn.push(v);
            }
            i = i + 1;
            prev_op = 1;
            continue;
        }
        // 运算符解析：name / pr / 消费的字符数 adv / 是否识别 is_op
        let mut name = String::new();
        let mut pr = 0;
        let mut adv = 1;
        let mut is_op = 1;
        if c == 43 { name = String::from("add"); pr = 80; }                                  // +
        else if c == 42 { name = String::from("mul"); pr = 90; }                              // *
        else if c == 47 { name = String::from("div"); pr = 90; }                              // /
        else if c == 37 { name = String::from("mod"); pr = 90; }                              // %
        else if c == 45 {
            if prev_op == 1 { name = String::from("sub"); pr = 80; }                          // -（二元）
            else { name = String::from("u-"); pr = 100; }                                     // -（一元）
        }
        else if c == 38 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 38 { name = String::from("and"); pr = 20; adv = 2; }                      // &&
            else { name = String::from("bitand"); pr = 40; }                                    // &
        }
        else if c == 94 { name = String::from("bitxor"); pr = 35; }                          // ^
        else if c == 124 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 124 { name = String::from("or"); pr = 10; adv = 2; }                     // ||
            else { name = String::from("bitor"); pr = 30; }                                   // |
        }
        else if c == 60 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 61 { name = String::from("le"); pr = 60; adv = 2; }                      // <=
            else if c2 == 60 { name = String::from("shl"); pr = 70; adv = 2; }                // <<
            else { name = String::from("lt"); pr = 60; }                                      // <
        }
        else if c == 62 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 61 { name = String::from("ge"); pr = 60; adv = 2; }                      // >=
            else if c2 == 62 { name = String::from("shr"); pr = 70; adv = 2; }                // >>
            else { name = String::from("gt"); pr = 60; }                                      // >
        }
        else if c == 61 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 61 { name = String::from("eq"); pr = 60; adv = 2; }                      // ==
            else { is_op = 0; adv = 1; }                                                      // 单 = 属语句，跳过
        }
        else if c == 33 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 61 { name = String::from("ne"); pr = 60; adv = 2; }                      // !=
            else { is_op = 0; adv = 1; }                                                      // 单 ! 属切片2
        }
        else { is_op = 0; adv = 1; }                                                          // 未知字符：跳过

        if is_op == 1 {
            while opstack.len() > 0 {
                let t = opstack.pop();
                let v = match t { Option::Some(x) => x, Option::None => String::new() };
                if v == "" { break; }
                if v == "(" { opstack.push(v); break; }
                let tp = prec_of(v);
                if tp >= pr { rpn.push(v); }
                else { opstack.push(v); break; }
            }
            opstack.push(name);
            prev_op = 0;
        }
        i = i + adv;
    }
    // 清空运算符栈
    while opstack.len() > 0 {
        let t = opstack.pop();
        let v = match t { Option::Some(x) => x, Option::None => String::new() };
        if v == "" { break; }
        if v == "(" { /* 丢弃未匹配左括号 */ }
        else { rpn.push(v); }
    }
    // 由 RPN(rpn) 迭代建树
    let mut ast: Vec<String> = Vec::new();
    let mut k = 0;
    while k < rpn.len() {
        let tok = rpn.get(k);
        let first = tok.get(0);
        if first == 40 {
            // 操作数：(int ...)/(var ...)
            ast.push(tok);
        } else if tok == "u-" {
            let a = ast.pop();
            match a {
                Option::Some(av) => { ast.push("(neg " + av + ")"); }
                Option::None => { ast.push("(neg ?)"); }
            }
        } else {
            let b = ast.pop();
            let a = ast.pop();
            match b {
                Option::Some(bv) => {
                    match a {
                        Option::Some(av) => { ast.push("(bin " + tok + " " + av + " " + bv + ")"); }
                        Option::None => { ast.push("(bin " + tok + " ? ?)"); }
                    }
                }
                Option::None => { ast.push("(bin " + tok + " ? ?)"); }
            }
        }
        k = k + 1;
    }
    ast.get(ast.len() - 1)
}
