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

// 表达式解析结果（Rlyeh 当前版本不支持元组字面量构造，用结构体承载「S-表达式 + 新位置」）。
struct PRes { s: String, ti: i64 }

// ============================================================================
// M-M2（SH-P2-7）切片2（M-M2b1）：程序/语句级解析 → 规范 S-表达式 AST 文本。
//
// 与 `rlyeh run <f> --emit ast-canonical` 的 Rust oracle（driver::emit_ast_canonical）
// 逐字节对齐，由 `crates/rlyeh-driver/tests/self_host_parser.rs` 差分对拍。
//
// 设计要点（延续 M-M2a 的"自包含、无递归"原则）：
//   - 自包含内联词法（`tokenize`）：数字/标识符/运算符/括号/块/分号，不依赖 lexer 产物。
//   - 表达式：复用 M-M2a 的运算符优先级与 shunting-yard 建树逻辑（改为对 token 流）。
//   - 语句/块：迭代式 buffer 栈（遇 `{` 入栈新 buffer，遇 `}` 出栈包成 (block ...)），
//     不引入递归，规避 Rlyeh 的共享 Vec 传递/递归语义风险。
//
// 规范格式（M-M2b1）：
//   (program STMT ...)                         顶层
//   (block STMT ... [FINAL-EXPR?])             块（FINAL-EXPR 仅块内末位裸表达式，裸渲染）
//   (let NAME INIT) / (let mut NAME INIT)       let（M-M2b1 忽略类型标注）
//   (semi EXPR)                                带 `;` 表达式语句 / 顶层裸表达式 / 顶层裸块
//   (return EXPR) / (return)                   return 表达式
// ============================================================================

// 将源码词法化为 token 流（每个 token 为字符串）：
//   操作数: "(int N)" / "(var NAME)"
//   运算符: "add"/"sub"/"mul"/"div"/"mod"/"bitand"/"bitor"/"bitxor"/"shl"/"shr"
//          /"lt"/"le"/"gt"/"ge"/"eq"/"ne"/"and"/"or"/"u-"
//   结构:   "(" / ")" / "{" / "}" / ";" / ":" / "="
//   关键字: "let" / "mut" / "return"
fn tokenize(src: String) -> Vec<String> {
    let mut toks: Vec<String> = Vec::new();
    let n = src.len;
    let mut i = 0;
    let mut prev_op = 0;   // 1 = 刚读到操作数；0 = 期待操作数（可遇一元负）
    while i < n {
        let c = src.get(i);
        // 空白
        if c == 32 || c == 9 || c == 13 || c == 10 { i = i + 1; continue; }
        // 整数
        if c >= 48 && c <= 57 {
            let start = i;
            while i < n && is_digit(src.get(i)) == 1 { i = i + 1; }
            toks.push("(int " + src.substring(start, i) + ")");
            prev_op = 1;
            continue;
        }
        // 标识符 / 关键字
        if is_ident_start(c) == 1 {
            let start = i;
            while i < n && is_ident_cont(src.get(i)) == 1 { i = i + 1; }
            let name = src.substring(start, i);
            if name == "let" { toks.push(String::from("let")); }
            else if name == "mut" { toks.push(String::from("mut")); }
            else if name == "return" { toks.push(String::from("return")); }
            else { toks.push("(var " + name + ")"); }
            prev_op = 1;
            continue;
        }
        // 括号 / 块 / 分号 / 冒号
        if c == 40 { toks.push(String::from("(")); prev_op = 0; i = i + 1; continue; }
        if c == 41 { toks.push(String::from(")")); prev_op = 1; i = i + 1; continue; }
        if c == 123 { toks.push(String::from("{")); prev_op = 0; i = i + 1; continue; }
        if c == 125 { toks.push(String::from("}")); prev_op = 0; i = i + 1; continue; }
        if c == 59 { toks.push(String::from(";")); prev_op = 0; i = i + 1; continue; }
        if c == 58 { toks.push(String::from(":")); prev_op = 0; i = i + 1; continue; }
        // = 与 ==
        if c == 61 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 61 { toks.push(String::from("eq")); i = i + 2; }
            else { toks.push(String::from("=")); i = i + 1; }
            prev_op = 0;
            continue;
        }
        // 运算符（同 M-M2a 优先级名）
        let mut name = String::new();
        let mut adv = 1;
        let mut is_op = 1;
        if c == 43 { name = String::from("add"); }                                  // +
        else if c == 42 { name = String::from("mul"); }                             // *
        else if c == 47 { name = String::from("div"); }                             // /
        else if c == 37 { name = String::from("mod"); }                             // %
        else if c == 45 {
            if prev_op == 1 { name = String::from("sub"); }
            else { name = String::from("u-"); }
        }
        else if c == 38 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 38 { name = String::from("and"); adv = 2; }                    // &&
            else { name = String::from("bitand"); }                                  // &
        }
        else if c == 94 { name = String::from("bitxor"); }                          // ^
        else if c == 124 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 124 { name = String::from("or"); adv = 2; }                    // ||
            else { name = String::from("bitor"); }                                   // |
        }
        else if c == 60 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 61 { name = String::from("le"); adv = 2; }                     // <=
            else if c2 == 60 { name = String::from("shl"); adv = 2; }               // <<
            else { name = String::from("lt"); }                                      // <
        }
        else if c == 62 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 61 { name = String::from("ge"); adv = 2; }                     // >=
            else if c2 == 62 { name = String::from("shr"); adv = 2; }               // >>
            else { name = String::from("gt"); }                                      // >
        }
        else if c == 33 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 61 { name = String::from("ne"); adv = 2; }                     // !=
            else { is_op = 0; adv = 1; }                                             // 单 ! 属后续切片
        }
        else { is_op = 0; adv = 1; }                                                // 未知字符跳过
        if is_op == 1 {
            toks.push(name);
            prev_op = 0;
        }
        i = i + adv;
    }
    toks
}

// 对 token 流从位置 ti 起解析一个表达式，返回 PRes{ s: S-表达式, ti: 新位置 }。
// 表达式在以下 token 处停止：';' '}' '=' ':' '{' 'let' 及 EOF（交由语句层处理）。
fn parse_expr(tokens: Vec<String>, ti0: i64) -> PRes {
    let mut ti = ti0;
    let n = tokens.len;
    let mut opstack: Vec<String> = Vec::new();
    let mut rpn: Vec<String> = Vec::new();
    let mut prev_op = 0;
    let mut ret_flag = 0;
    // return 前缀
    if ti < n && tokens.get(ti) == "return" {
        ret_flag = 1;
        ti = ti + 1;
    }
    while ti < n {
        let tok = tokens.get(ti);
        // 停止条件
        if tok == ";" { break; }
        if tok == "}" { break; }
        if tok == "=" { break; }
        if tok == ":" { break; }
        if tok == "{" { break; }
        if tok == "let" { break; }
        // 操作数：(int ...)/(var ...) 起于 '(' 但非 "("
        let first = tok.get(0);
        if first == 40 && tok != "(" {
            rpn.push(tok);
            prev_op = 1;
            ti = ti + 1;
            continue;
        }
        if tok == "(" {
            opstack.push(String::from("("));
            prev_op = 0;
            ti = ti + 1;
            continue;
        }
        if tok == ")" {
            while opstack.len() > 0 {
                let t = opstack.pop();
                let v = match t { Option::Some(x) => x, Option::None => String::new() };
                if v == "" { break; }
                if v == "(" { break; }
                rpn.push(v);
            }
            ti = ti + 1;
            prev_op = 1;
            continue;
        }
        // 运算符（含 u-）
        let pr = prec_of(tok);
        while opstack.len() > 0 {
            let t = opstack.pop();
            let v = match t { Option::Some(x) => x, Option::None => String::new() };
            if v == "" { break; }
            if v == "(" { opstack.push(v); break; }
            let tp = prec_of(v);
            if tp >= pr { rpn.push(v); }
            else { opstack.push(v); break; }
        }
        opstack.push(tok);
        prev_op = 0;
        ti = ti + 1;
    }
    // 清空运算符栈
    while opstack.len() > 0 {
        let t = opstack.pop();
        let v = match t { Option::Some(x) => x, Option::None => String::new() };
        if v == "" { break; }
        if v == "(" { /* 丢弃未匹配左括号 */ }
        else { rpn.push(v); }
    }
    // 由 RPN 迭代建树
    let mut ast: Vec<String> = Vec::new();
    let mut k = 0;
    while k < rpn.len() {
        let tk = rpn.get(k);
        let f = tk.get(0);
        if f == 40 {
            ast.push(tk);
        } else if tk == "u-" {
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
                        Option::Some(av) => { ast.push("(bin " + tk + " " + av + " " + bv + ")"); }
                        Option::None => { ast.push("(bin " + tk + " ? ?)"); }
                    }
                }
                Option::None => { ast.push("(bin " + tk + " ? ?)"); }
            }
        }
        k = k + 1;
    }
    if ast.len() == 0 {
        if ret_flag == 1 { return PRes { s: String::from("(return)"), ti: ti }; }
        return PRes { s: String::from("?"), ti: ti };
    }
    let res = ast.get(ast.len() - 1);
    if ret_flag == 1 { return PRes { s: "(return " + res + ")", ti: ti }; }
    PRes { s: res, ti: ti }
}

// 解析整段程序源码，返回规范 S-表达式程序文本（见本文件顶部"规范格式"）。
fn parse_program(src: String) -> String {
    let tokens = tokenize(src);
    let n = tokens.len;
    let mut ti = 0;
    let mut bufstack: Vec<String> = Vec::new();
    bufstack.push("");   // 程序体 buffer
    while ti < n {
        let tok = tokens.get(ti);
        // 空语句
        if tok == ";" { ti = ti + 1; continue; }
        // 块结束
        if tok == "}" {
            if bufstack.len() > 1 {
                ti = ti + 1;
                let body_opt = bufstack.pop();
                let body = match body_opt { Option::Some(x) => x, Option::None => String::new() };
                let block = "(block" + body + ")";
                let nxt = if ti < n { tokens.get(ti) } else { String::from("") };
                let mut append = block;
                if nxt == ";" {
                    ti = ti + 1;
                    append = "(semi " + block + ")";
                } else if nxt == "}" {
                    append = block;   // 紧跟 '}' → 父块块尾表达式，裸
                } else {
                    append = "(semi " + block + ")";  // 后接更多语句 → Semi
                }
                let p_opt = bufstack.pop();
                let mut p = match p_opt { Option::Some(x) => x, Option::None => String::new() };
                p = p + " " + append;
                bufstack.push(p);
            } else {
                ti = ti + 1;   // 多余 '}' 直接跳过
            }
            continue;
        }
        // 块开始
        if tok == "{" {
            ti = ti + 1;
            bufstack.push("");
            continue;
        }
        // let 语句
        if tok == "let" {
            ti = ti + 1;
            let mut is_mut = 0;
            if ti < n && tokens.get(ti) == "mut" { is_mut = 1; ti = ti + 1; }
            let raw = tokens.get(ti); ti = ti + 1;
            let name = raw.substring(5, raw.len - 1);
            // 跳过类型标注（M-M2b1 忽略）：从 ':' 跳到 '='
            if ti < n && tokens.get(ti) == ":" {
                loop {
                    if ti >= n { break; }
                    if tokens.get(ti) == "=" { break; }
                    ti = ti + 1;
                }
            }
            // 越过 '='
            if ti < n && tokens.get(ti) == "=" { ti = ti + 1; }
            let r = parse_expr(tokens, ti);
            let init = r.s;
            ti = r.ti;
            if ti < n && tokens.get(ti) == ";" { ti = ti + 1; }
            let node = if is_mut == 1 {
                "(let mut " + name + " " + init + ")"
            } else {
                "(let " + name + " " + init + ")"
            };
            let p_opt = bufstack.pop();
            let mut p = match p_opt { Option::Some(x) => x, Option::None => String::new() };
            p = p + " " + node;
            bufstack.push(p);
            continue;
        }
        // 其余：表达式语句（含 return；块表达式已由 '{' 分支处理）
        let r = parse_expr(tokens, ti);
        let e = r.s;
        ti = r.ti;
        let nxt = if ti < n { tokens.get(ti) } else { String::from("") };
        let mut s = e;
        if nxt == ";" {
            ti = ti + 1;
            s = "(semi " + e + ")";
        } else if nxt == "}" {
            s = e;   // 紧跟 '}' → 块尾表达式，裸
        } else {
            s = "(semi " + e + ")";   // 顶层裸表达式 / 后接更多语句 → Semi
        }
        let p_opt = bufstack.pop();
        let mut p = match p_opt { Option::Some(x) => x, Option::None => String::new() };
        p = p + " " + s;
        bufstack.push(p);
    }
    let body_opt = bufstack.pop();
    let body = match body_opt { Option::Some(x) => x, Option::None => String::new() };
    "(program" + body + ")"
}
