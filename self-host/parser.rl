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
    if op.substring(0, 1) == "r" { return 5; }   // 范围运算符标记 r..< / r... / r<..
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

// 函数项头解析结果（M-M6）：规范 header 串（NAME (params ...) [RET]）+ 新位置。
struct FHdr { h: String, ti: i64 }

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
// 规范格式（M-M2b1 + M-M2b2 + M-M2c）：
//   (program STMT ...)                         顶层
//   (block STMT ... [FINAL-EXPR?])             块（FINAL-EXPR 仅块内末位裸表达式，裸渲染）
//   (let PAT [TYPE] INIT) / (let mut PAT [TYPE] INIT)
//                                                  let（M-M2b2 起：PAT=标识符/_/(tuple-pat ...)，
//                                                  TYPE 可选，存在时渲染为 (type ...) 节点）
//   (semi EXPR)                                带 `;` 表达式语句 / 顶层裸表达式 / 顶层裸块
//   (return EXPR) / (return)                   return 表达式
//   (if COND (block ...) [ (block ...) ])      if/else（M-M2c；else 分支可选；else if 收束为嵌套 if）
//   (while COND (block ...))                   while 循环（M-M2c）
//   (loop (block ...))                         loop 循环（M-M2c）
//   控制流作语句：`(semi (if ...))`；作块尾表达式：裸 `(if ...)`（与 oracle 一致）
//   控制流作 let 初始化：`(let PAT [TYPE] (if ...))`（M-M2c 表达式双形态之一）
//
// 类型规范（M-M2b2，迭代式 @GEN@ 栈，见 parse_type_core）：
//   (type NAME ARG...)     路径类型，泛型实参递归渲染
//   (ref (type T))          &T
//   (ref mut (type T))      &mut T
//   (tuple-type T1 T2 ...)  (T1, T2) 元组类型
//   (array T N)             [T; N]（N 为长度表达式规范串）
//   (infer)                 _
// 模式规范（M-M2b2）：标识符裸名 / "_" / "(tuple-pat E1 E2 ...)"（扁平元组）。
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
        // 注释（M-M7）：行注释 // 与块注释 /* */（嵌套判定用嵌套 if，避免 && 非短路下 get 越界）
        if c == 47 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 47 {
                i = i + 2;
                while i < n {
                    if src.get(i) == 10 { break; }
                    i = i + 1;
                }
                continue;
            }
            if c2 == 42 {
                i = i + 2;
                while i < n {
                    let mut is_end = 0;
                    if src.get(i) == 42 {
                        if i + 1 < n {
                            if src.get(i + 1) == 47 { is_end = 1; }
                        }
                    }
                    if is_end == 1 { i = i + 2; break; }
                    i = i + 1;
                }
                continue;
            }
        }
        // 字符串字面量（M-M7a：不含转义，扫描至下一个 '"'）
        if c == 34 {
            let start = i + 1;
            let mut j = start;
            while j < n {
                if src.get(j) == 34 { break; }
                j = j + 1;
            }
            toks.push("(str " + src.substring(start, j) + ")");
            prev_op = 1;
            i = j + 1;
            continue;
        }
        // 整数
        if c >= 48 && c <= 57 {
            let start = i;
            while i < n && is_digit(src.get(i)) == 1 { i = i + 1; }
            toks.push("(int " + src.substring(start, i) + ")");
            prev_op = 1;
            continue;
        }
        // 标识符 / 关键字 / 布尔字面量
        if is_ident_start(c) == 1 {
            let start = i;
            while i < n && is_ident_cont(src.get(i)) == 1 { i = i + 1; }
            let name = src.substring(start, i);
            if name == "let" { toks.push(String::from("let")); }
            else if name == "mut" { toks.push(String::from("mut")); }
            else if name == "return" { toks.push(String::from("return")); }
            else if name == "if" { toks.push(String::from("if")); }
            else if name == "else" { toks.push(String::from("else")); }
            else if name == "while" { toks.push(String::from("while")); }
            else if name == "loop" { toks.push(String::from("loop")); }
            else if name == "for" { toks.push(String::from("for")); }
            else if name == "in" { toks.push(String::from("in")); }
            else if name == "match" { toks.push(String::from("match")); }
            else if name == "fn" { toks.push(String::from("fn")); }
            else if name == "pub" { toks.push(String::from("pub")); }
            else if name == "true" { toks.push(String::from("(bool true)")); }
            else if name == "false" { toks.push(String::from("(bool false)")); }
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
        // 逗号 / 方括号（M-M2b2：模式与类型标注使用；表达式解析中作停止符）
        if c == 44 { toks.push(String::from(",")); prev_op = 0; i = i + 1; continue; }
        if c == 91 { toks.push(String::from("[")); prev_op = 0; i = i + 1; continue; }
        if c == 93 { toks.push(String::from("]")); prev_op = 0; i = i + 1; continue; }
        // 点号 / 范围运算符（M-M3a 字段访问 a.b；M-M4 范围 ..< ... <..）
        if c == 46 {
            // 范围：..<（左闭右开）/ ...（闭区间）/ ..=（废弃，仍产 range 节点）/ ..（废弃）
            if i + 1 < n && src.get(i + 1) == 46 {
                let c2 = if i + 2 < n { src.get(i + 2) } else { 0 };
                if c2 == 60 { toks.push(String::from("..<")); i = i + 3; }       // ..<
                else if c2 == 61 { toks.push(String::from("..=")); i = i + 3; }  // ..= (废弃)
                else if c2 == 46 { toks.push(String::from("...")); i = i + 3; }  // ...
                else { toks.push(String::from("..")); i = i + 2; }              // .. (废弃)
                prev_op = 1;
                continue;
            }
            toks.push(String::from(".")); prev_op = 1; i = i + 1; continue;
        }
        // = / == / =>
        if c == 61 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 62 { toks.push(String::from("fatarrow")); i = i + 2; prev_op = 0; continue; }  // =>
            if c2 == 61 { toks.push(String::from("eq")); i = i + 2; }
            else { toks.push(String::from("=")); i = i + 1; }
            prev_op = 0;
            continue;
        }
        // 箭头 ->（M-M6：fn 返回类型分隔符）
        if c == 45 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 62 { toks.push(String::from("arrow")); prev_op = 0; i = i + 2; continue; }
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
            else if c2 == 46 && i + 2 < n && src.get(i + 2) == 46 {
                toks.push(String::from("<..")); prev_op = 0; i = i + 3; continue;   // <.. 左开右闭
            }
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

// ============================================================================
// M-M2e + M-M3：表达式后缀统一标记化（字段 .field / 索引 [ ] / 调用 ( ) /
//   元组字面量 ( a, b ) / 数组字面量 [ a, b ]）。
//   设计：shunting-yard 主循环把每个后缀转成 RPN 标记，末位建树阶段归约。
//   规避 Rlyeh 编译器对前向递归 / if-表达式返回值的已知限制——全程迭代，
//   索引/调用/元组/数组的子表达式直接在 RPN 内展开（靠 opstack 括号界定的
//   运算符冲刷），不需要递归调用 parse_expr。
// 与 oracle（render_expr_canonical）逐字节对齐：
//   a.b        -> (field (var a) b)
//   a.b.c      -> (field (field (var a) b) c)
//   (a + b).c  -> (field (bin add (var a) (var b)) c)
//   a[0]       -> (index (var a) (int 0))
//   a[i + 1]   -> (index (var a) (bin add (var i) (int 1)))
//   arr[i][j]  -> (index (index (var arr) (var i)) (var j))
//   foo()      -> (call (var foo))
//   foo(1, 2)  -> (call (var foo) (int 1) (int 2))
//   a.b(1)     -> (call (field (var a) b) (int 1))
//   add(1,2).scale(3) -> (call (field (call (var add) (int 1) (int 2)) scale) (int 3))
//   f(g(x))    -> (call (var f) (call (var g) (var x)))
//   (1, 2)     -> (tuple (int 1) (int 2))
//   (1 + 2, 3) -> (tuple (bin add (int 1) (int 2)) (int 3))
//   ((1, 2))   -> (tuple (int 1) (int 2))
//   [1, 2, 3]  -> (array-lit (int 1) (int 2) (int 3))
//   [(1, 2), 3] -> (array-lit (tuple (int 1) (int 2)) (int 3))
// RPN 标记约定（均以 '@' 起头，归约时 f==64）：
//   @field@NAME  : 字段访问后缀，弹栈顶节点包裹
//   @open-index@: 索引开始，建树时压 @OPEN@ 哨兵
//   @close-index@: 索引结束，弹至 @OPEN@ 取索引子表达式 + 接收者包裹
//   @open-call@ : 调用开始，建树时压 @OPEN@ 哨兵
//   @argsep@    : 调用实参分隔，建树时压 @ARGSEP@ 哨兵
//   @close-call@: 调用结束，弹至 @OPEN@ 拆 @ARGSEP@ 得实参 + 接收者包裹
//   @open-paren@: 分组/元组左圆括号，建树时压 @PARENSENT@ 哨兵（元组用其定位起点）
//   @tupsep@    : 元组元素分隔，建树时压 @TUPSEP@ 哨兵
//   @close-tup@ : 元组结束，弹至 @PARENSENT@ 拆 @TUPSEP@ 得元素包裹 (tuple ...)
//   @open-arr@  : 数组字面量开始，建树时压 @ARRSENT@ 哨兵
//   @arrsep@    : 数组元素分隔，建树时压 @ARRSEP@ 哨兵
//   @close-arr@ : 数组结束，弹至 @ARRSENT@ 拆 @ARRSEP@ 得元素包裹 (array-lit ...)
// opstack 括号：'(' 分组 / '(c' 调用 / '(t' 元组 / '[' 索引 / '[a' 数组字面量
//   （仅用于 ')'']' 冲刷运算符时定界）；',' 依栈顶括号类型决定分隔标记。
// ============================================================================

// 对 token 流从位置 ti 起解析一个表达式，返回 PRes{ s: S-表达式, ti: 新位置 }。
// 表达式在以下 token 处停止：';' '}' '=' ':' '{' 'let' 及 EOF；顶层 ','（非调用
//   元组/数组上下文）亦停止（交由语句层处理）。
fn parse_expr(tokens: Vec<String>, ti0: i64) -> PRes {
    let n = tokens.len;
    let mut ti = ti0;
    let mut opstack: Vec<String> = Vec::new();
    let mut rpn: Vec<String> = Vec::new();
    let mut prev_op = 0;     // 1 = 刚见到操作数 / 右括号 / 字段 / 索引闭 / 调用闭
    let mut call_depth = 0;  // 当前处于第几层调用括号（区分 ',' 是否实参分隔）
    let mut tuple_depth = 0; // 当前处于第几层元组字面量括号
    let mut arr_depth = 0;   // 当前处于第几层数组字面量括号
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
        // 逗号：调用/元组/数组上下文作元素分隔；否则停止（语句层处理）
        if tok == "," {
            // 冲刷运算符栈直到栈顶括号（公共逻辑）
            let mut flush = 1;
            while flush == 1 {
                if opstack.len() == 0 { flush = 0; break; }
                let t = opstack.pop();
                let v = match t { Option::Some(x) => x, Option::None => String::new() };
                if v == "" { flush = 0; break; }
                if v == "(" || v == "(c" || v == "(t" || v == "[" || v == "[a" {
                    opstack.push(v); flush = 0; break;
                }
                rpn.push(v);
            }
            // 依栈顶括号类型决定分隔标记（peek = pop+push）
            if opstack.len() == 0 {
                // 顶层裸逗号：停止（语句层处理元组/数组声明）
                break;
            }
            let mut top_o = opstack.pop();
            let top = match top_o { Option::Some(x) => x, Option::None => String::new() };
            if top == "(c" {
                opstack.push(top);
                rpn.push(String::from("@argsep@"));
            } else if top == "(t" {
                opstack.push(top);
                rpn.push(String::from("@tupsep@"));
            } else if top == "[a" {
                opstack.push(top);
                rpn.push(String::from("@arrsep@"));
            } else if top == "(" {
                // 分组转元组：首个逗号出现，将 '(' 升级为 '(t'
                opstack.push(String::from("(t"));
                tuple_depth = tuple_depth + 1;
                rpn.push(String::from("@tupsep@"));
            } else {
                // 顶层裸逗号：停止（语句层处理元组/数组声明）
                opstack.push(top);
                break;
            }
            prev_op = 0;
            ti = ti + 1;
            continue;
        }
        // 右圆括号：弹运算符直到匹配 ( 或 (c
        if tok == ")" {
            let mut un = 1;
            while un == 1 {
                if opstack.len() == 0 { un = 0; break; }
                let t = opstack.pop();
                let v = match t { Option::Some(x) => x, Option::None => String::new() };
                if v == "" { un = 0; break; }
                if v == "(" { prev_op = 1; un = 0; break; }       // 分组：直接丢弃
                if v == "(c" {
                    rpn.push(String::from("@close-call@"));
                    call_depth = call_depth - 1;
                    prev_op = 1;
                    un = 0;
                    break;
                }
                if v == "(t" {
                    rpn.push(String::from("@close-tup@"));
                    tuple_depth = tuple_depth - 1;
                    prev_op = 1;
                    un = 0;
                    break;
                }
                rpn.push(v);
            }
            ti = ti + 1;
            continue;
        }
        // 右方括号：弹运算符直到 [
        if tok == "]" {
            let mut un = 1;
            while un == 1 {
                if opstack.len() == 0 { un = 0; break; }
                let t = opstack.pop();
                let v = match t { Option::Some(x) => x, Option::None => String::new() };
                if v == "" { un = 0; break; }
                if v == "[" { rpn.push(String::from("@close-index@")); prev_op = 1; un = 0; break; }
                if v == "[a" {
                    rpn.push(String::from("@close-arr@"));
                    arr_depth = arr_depth - 1;
                    prev_op = 1;
                    un = 0;
                    break;
                }
                rpn.push(v);
            }
            ti = ti + 1;
            continue;
        }
        // 字段访问后缀 .name
        if tok == "." {
            ti = ti + 1;
            let mut nm = String::from("?");
            if ti < n { nm = tokens.get(ti); }
            ti = ti + 1;
            let mut name = nm;
            if nm.len > 5 && nm.substring(0, 5) == "(var " {
                name = nm.substring(5, nm.len - 1);
            }
            rpn.push("@field@" + name);
            prev_op = 1;
            continue;
        }
        // 左方括号：前接操作数 -> 索引后缀；否则数组字面量
        if tok == "[" {
            if prev_op == 1 {
                rpn.push(String::from("@open-index@"));
                opstack.push(String::from("["));
            } else {
                rpn.push(String::from("@open-arr@"));
                opstack.push(String::from("[a"));
                arr_depth = arr_depth + 1;
            }
            prev_op = 0;
            ti = ti + 1;
            continue;
        }
        // 左圆括号：前接操作数 -> 调用后缀；否则分组（亦可能是元组，待 ',' 转 (t）
        if tok == "(" {
            if prev_op == 1 {
                rpn.push(String::from("@open-call@"));
                opstack.push(String::from("(c"));
                call_depth = call_depth + 1;
            } else {
                rpn.push(String::from("@open-paren@"));
                opstack.push(String::from("("));
            }
            prev_op = 0;
            ti = ti + 1;
            continue;
        }
        // 范围运算符（M-M4：..< / ... / <..）：二元中缀，归约为 (range LOWER UPPER L U)
        if tok == "..<" || tok == "..." || tok == "<.." {
            // 冲刷运算符栈直到括号或更低优先级（范围优先级最低，仅高于赋值）
            let mut sh = 1;
            while sh == 1 {
                if opstack.len() == 0 { sh = 0; break; }
                let t = opstack.pop();
                let v = match t { Option::Some(x) => x, Option::None => String::new() };
                if v == "" { sh = 0; break; }
                if v == "(" || v == "(c" || v == "(t" || v == "[" || v == "[a" { opstack.push(v); sh = 0; break; }
                let tp = prec_of(v);
                if tp >= 6 { rpn.push(v); }
                else { opstack.push(v); sh = 0; break; }
            }
            opstack.push("r" + tok);   // 范围标记：r..< / r... / r<..
            prev_op = 0;
            ti = ti + 1;
            continue;
        }
        // 操作数：(int ...)/(var ...) 起于 '(' 但非 "("
        let first = tok.get(0);
        if first == 40 && tok != "(" {
            rpn.push(tok);
            prev_op = 1;
            ti = ti + 1;
            continue;
        }
        // 运算符（含 u-：prec_of("u-")==100，走 shunting-yard 落于运算元之后，
        //   归约阶段弹栈得 (neg ...)；不可提前压入 rpn，否则顺序错乱）
        let pr = prec_of(tok);
        let mut sh = 1;
        while sh == 1 {
            if opstack.len() == 0 { sh = 0; break; }
            let t = opstack.pop();
            let v = match t { Option::Some(x) => x, Option::None => String::new() };
            if v == "" { sh = 0; break; }
            if v == "(" || v == "(c" || v == "[" { opstack.push(v); sh = 0; break; }
            let tp = prec_of(v);
            if tp >= pr { rpn.push(v); }
            else { opstack.push(v); sh = 0; break; }
        }
        opstack.push(tok);
        prev_op = 0;
        ti = ti + 1;
    }
    // 清空运算符栈（遇括号终止）
    let mut dr = 1;
    while dr == 1 {
        if opstack.len() == 0 { dr = 0; break; }
        let t = opstack.pop();
        let v = match t { Option::Some(x) => x, Option::None => String::new() };
        if v == "" { dr = 0; break; }
        if v == "(" || v == "(c" || v == "(t" || v == "[" || v == "[a" { /* 丢弃未匹配括号 */ }
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
            // 弹出操作数（跳过分组/元组/调用/数组等哨兵，它们非真实节点）
            let mut a = String::from("?");
            let mut sk = 1;
            while sk == 1 {
                let ao = ast.pop();
                let av = match ao { Option::Some(x) => x, Option::None => String::new() };
                if av == "" { a = String::from("?"); sk = 0; break; }
                if av == "@PARENSENT@" || av == "@TUPSEP@" || av == "@ARGSEP@" || av == "@OPEN@" || av == "@ARRSENT@" || av == "@ARRSEP@" {
                    continue;
                }
                a = av; sk = 0; break;
            }
            ast.push("(neg " + a + ")");
        } else if f == 64 {
            // '@' 起头的后缀标记
            if tk.substring(0, 7) == "@field@" {
                let name = tk.substring(7, tk.len);
                let a = ast.pop();
                match a {
                    Option::Some(av) => { ast.push("(field " + av + " " + name + ")"); }
                    Option::None => { ast.push("(field ? " + name + ")"); }
                }
            } else if tk == "@open-index@" {
                ast.push(String::from("@OPEN@"));
            } else if tk == "@close-index@" {
                let idx_o = ast.pop();
                let idx = match idx_o { Option::Some(x) => x, Option::None => String::from("?") };
                if idx == "@OPEN@" {
                    ast.push(String::from("?"));   // 空索引（异常输入）
                } else {
                    let _open = ast.pop();   // 丢弃 @OPEN@
                    // 接收者可能位于嵌套分组哨兵 @PARENSENT@ 之下，跳过之
                    let mut rcv = String::from("?");
                    let mut sk = 1;
                    while sk == 1 {
                        let ro = ast.pop();
                        let rv = match ro { Option::Some(x) => x, Option::None => String::new() };
                        if rv == "" { sk = 0; break; }
                        if rv == "@PARENSENT@" { continue; }
                        rcv = rv; sk = 0; break;
                    }
                    ast.push("(index " + rcv + " " + idx + ")");
                }
            } else if tk == "@open-call@" {
                ast.push(String::from("@OPEN@"));
            } else if tk == "@argsep@" {
                ast.push(String::from("@ARGSEP@"));
            } else if tk == "@close-call@" {
                // 收集实参（逆序弹出，遇 @ARGSEP@ 分拆，遇 @OPEN@ 终止）
                let mut args: Vec<String> = Vec::new();
                let mut col = 1;
                while col == 1 {
                    if ast.len() == 0 { col = 0; break; }
                    let top_o = ast.pop();
                    let top = match top_o { Option::Some(x) => x, Option::None => String::new() };
                    if top == "@OPEN@" { col = 0; break; }
                    if top == "@ARGSEP@" { continue; }
                    if top == "@PARENSENT@" { continue; }   // 跳过分组哨兵（如 f((a+b))）
                    args.push(top);
                }
                // 反转还原实参顺序
                let mut ai = 0;
                let mut aj = args.len - 1;
                while ai < aj {
                    let tmp = args.get(ai);
                    args.set(ai, args.get(aj));
                    args.set(aj, tmp);
                    ai = ai + 1; aj = aj - 1;
                }
                let callee_o = ast.pop();
                let callee = match callee_o { Option::Some(x) => x, Option::None => String::from("?") };
                let mut s = "(call " + callee;
                let mut ai2 = 0;
                while ai2 < args.len {
                    s = s + " " + args.get(ai2);
                    ai2 = ai2 + 1;
                }
                s = s + ")";
                ast.push(s);
            } else if tk == "@open-paren@" {
                ast.push(String::from("@PARENSENT@"));
            } else if tk == "@tupsep@" {
                ast.push(String::from("@TUPSEP@"));
            } else if tk == "@close-tup@" {
                // 收集元组元素（逆序弹出，遇 @TUPSEP@ 分拆，遇 @PARENSENT@ 终止）
                let mut elems: Vec<String> = Vec::new();
                let mut col = 1;
                while col == 1 {
                    if ast.len() == 0 { col = 0; break; }
                    let top_o = ast.pop();
                    let top = match top_o { Option::Some(x) => x, Option::None => String::new() };
                    if top == "@PARENSENT@" { col = 0; break; }
                    if top == "@TUPSEP@" { continue; }
                    elems.push(top);
                }
                let mut ai = 0;
                let mut aj = elems.len - 1;
                while ai < aj {
                    let tmp = elems.get(ai);
                    elems.set(ai, elems.get(aj));
                    elems.set(aj, tmp);
                    ai = ai + 1; aj = aj - 1;
                }
                let mut s = String::from("(tuple");
                let mut ai2 = 0;
                while ai2 < elems.len {
                    s = s + " " + elems.get(ai2);
                    ai2 = ai2 + 1;
                }
                s = s + ")";
                ast.push(s);
            } else if tk == "@open-arr@" {
                ast.push(String::from("@ARRSENT@"));
            } else if tk == "@arrsep@" {
                ast.push(String::from("@ARRSEP@"));
            } else if tk == "@close-arr@" {
                let mut elems: Vec<String> = Vec::new();
                let mut col = 1;
                while col == 1 {
                    if ast.len() == 0 { col = 0; break; }
                    let top_o = ast.pop();
                    let top = match top_o { Option::Some(x) => x, Option::None => String::new() };
                    if top == "@ARRSENT@" { col = 0; break; }
                    if top == "@ARRSEP@" { continue; }
                    elems.push(top);
                }
                let mut ai = 0;
                let mut aj = elems.len - 1;
                while ai < aj {
                    let tmp = elems.get(ai);
                    elems.set(ai, elems.get(aj));
                    elems.set(aj, tmp);
                    ai = ai + 1; aj = aj - 1;
                }
                let mut s = String::from("(array-lit");
                let mut ai2 = 0;
                while ai2 < elems.len {
                    s = s + " " + elems.get(ai2);
                    ai2 = ai2 + 1;
                }
                s = s + ")";
                ast.push(s);
            } else {
                ast.push(tk);
            }
        } else {
            if tk.substring(0, 1) == "r" {
                // 范围运算符归约：(range LOWER UPPER lower_inclusive upper_inclusive)
                let bo = ast.pop();
                let bv = match bo { Option::Some(x) => x, Option::None => String::from("?") };
                let ao = ast.pop();
                let av = match ao { Option::Some(x) => x, Option::None => String::from("?") };
                let mut li = 1; let mut ui = 1;
                if tk == "r..<" { li = 1; ui = 0; }
                else if tk == "r..." { li = 1; ui = 1; }
                else if tk == "r<.." { li = 0; ui = 1; }
                else { li = 1; ui = 1; }
                ast.push("(range " + av + " " + bv + " " + int_to_string(li) + " " + int_to_string(ui) + ")");
            } else {
            // 弹出右操作数（跳过分组/元组/调用/数组等哨兵，它们非真实节点）
            let mut b = String::from("?");
            let mut sk = 1;
            while sk == 1 {
                let bo = ast.pop();
                let bv = match bo { Option::Some(x) => x, Option::None => String::new() };
                if bv == "" { b = String::from("?"); sk = 0; break; }
                if bv == "@PARENSENT@" || bv == "@TUPSEP@" || bv == "@ARGSEP@" || bv == "@OPEN@" || bv == "@ARRSENT@" || bv == "@ARRSEP@" {
                    continue;
                }
                b = bv; sk = 0; break;
            }
            // 弹出左操作数（跳过哨兵）
            let mut a = String::from("?");
            let mut sk2 = 1;
            while sk2 == 1 {
                let ao = ast.pop();
                let av = match ao { Option::Some(x) => x, Option::None => String::new() };
                if av == "" { a = String::from("?"); sk2 = 0; break; }
                if av == "@PARENSENT@" || av == "@TUPSEP@" || av == "@ARGSEP@" || av == "@OPEN@" || av == "@ARRSENT@" || av == "@ARRSEP@" {
                    continue;
                }
                a = av; sk2 = 0; break;
            }
            ast.push("(bin " + tk + " " + a + " " + b + ")");
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

// 从 token 串提取类型/模式名："(var Foo)" -> "Foo"，其余原样返回。
// 注意：Rlyeh 对"返回 String 的函数使用 early return"会生成错误 IR（ret i64），
// 故全部改用累加变量 + 尾部表达式，绝不使用 return / if-表达式作为返回值。
fn typename_of(t: String) -> String {
    let mut res = String::from("");
    if t.len > 5 && t.substring(0, 5) == "(var " {
        res = t.substring(5, t.len - 1);
    } else {
        res = t;
    }
    res
}

// 迭代解析类型核心（不含前置 ref，ref 由 parse_type_tokens 剥离）。所有原子压入 stk：
//   '<' 压 @GEN@，'>' 弹出 @GEN@ 之上全部实参 + @GEN@ 之下基类型，包成 (type BASE ARG...)；
//   '(' 压 @TUP@，')' 弹出直到 @TUP@ 包成 (tuple-type ...)；
//   '[' 压 @ARR@，';' 压 @ASEP@（其后为长度表达式，原子按字面渲染不包 (type ...)），
//        ']' 弹出直到 @ARR@，以 @ASEP@ 为界拆出 类型 T 与 长度 N，包成 (array T N)。
// 实参/元素逆序弹出后反转还原。必须在 parse_type_tokens 之前定义，避免前向调用推断为 i64。
fn parse_type_core(ts: Vec<String>, start: i64) -> String {
    let n = ts.len;
    let mut k = start;
    let mut stk: Vec<String> = Vec::new();
    let mut in_len = 0;   // 1 = 当前处于数组长度表达式部分（原子按字面渲染）
    while k < n {
        let t = ts.get(k);
        if t == "lt" {
            stk.push(String::from("@GEN@"));
            k = k + 1; continue;
        }
        if t == "gt" {
            // 泛型收束
            let mut args: Vec<String> = Vec::new();
            while stk.len() > 0 {
                let top_opt = stk.pop();
                let top = match top_opt { Option::Some(x) => x, Option::None => String::new() };
                if top == "@GEN@" { break; }
                args.push(top);
            }
            // 反转（弹出顺序为逆序）
            let mut ai = 0;
            let mut aj = args.len - 1;
            while ai < aj {
                let tmp = args.get(ai);
                args.set(ai, args.get(aj));
                args.set(aj, tmp);
                ai = ai + 1; aj = aj - 1;
            }
            let base_opt = stk.pop();
            let base = match base_opt { Option::Some(x) => x, Option::None => String::from("(type ?)") };
            let mut w = base;
            let mut xi = 0;
            while xi < args.len {
                w = w + " " + args.get(xi);
                xi = xi + 1;
            }
            w = w + ")";
            stk.push(w);
            k = k + 1; continue;
        }
        if t == "(" {
            stk.push(String::from("@TUP@"));
            k = k + 1; continue;
        }
        if t == ")" {
            // 元组收束：(T1, T2, ...) -> (tuple-type T1 T2 ...)
            let mut elems: Vec<String> = Vec::new();
            while stk.len() > 0 {
                let top_opt = stk.pop();
                let top = match top_opt { Option::Some(x) => x, Option::None => String::new() };
                if top == "@TUP@" { break; }
                elems.push(top);
            }
            let mut ai = 0;
            let mut aj = elems.len - 1;
            while ai < aj {
                let tmp = elems.get(ai);
                elems.set(ai, elems.get(aj));
                elems.set(aj, tmp);
                ai = ai + 1; aj = aj - 1;
            }
            let mut w = String::from("(tuple-type");
            let mut xi = 0;
            while xi < elems.len {
                w = w + " " + elems.get(xi);
                xi = xi + 1;
            }
            w = w + ")";
            stk.push(w);
            k = k + 1; continue;
        }
        if t == "[" {
            stk.push(String::from("@ARR@"));
            k = k + 1; continue;
        }
        if t == ";" {
            // 数组长度分隔符（仅出现在 [T; N] 内）
            stk.push(String::from("@ASEP@"));
            in_len = 1;
            k = k + 1; continue;
        }
        if t == "]" {
            // 数组收束：[T; N] -> (array T N)
            let mut parts: Vec<String> = Vec::new();
            while stk.len() > 0 {
                let top_opt = stk.pop();
                let top = match top_opt { Option::Some(x) => x, Option::None => String::new() };
                if top == "@ARR@" { break; }
                parts.push(top);
            }
            // parts 自顶向下：[N_last ... N_first, "@ASEP@", T_last ... T_first]
            let mut sep_idx = -1;
            let mut pi = 0;
            while pi < parts.len {
                if parts.get(pi) == "@ASEP@" { sep_idx = pi; }
                pi = pi + 1;
            }
            let mut nstr = String::from("?");
            let mut tstr = String::from("?");
            if sep_idx >= 0 {
                // 长度部分 = parts[0..sep_idx]（顶->sep），反转还原左到右
                let mut li = sep_idx - 1;
                while li >= 0 {
                    if nstr == "?" { nstr = parts.get(li); }
                    else { nstr = parts.get(li) + " " + nstr; }
                    li = li - 1;
                }
                // 类型部分 = parts[sep_idx+1..end]（sep->底，顺序为逆），反转还原左到右
                let mut ti = parts.len - 1;
                while ti > sep_idx {
                    if tstr == "?" { tstr = parts.get(ti); }
                    else { tstr = parts.get(ti) + " " + tstr; }
                    ti = ti - 1;
                }
            }
            let w = "(array " + tstr + " " + nstr + ")";
            stk.push(w);
            in_len = 0;
            k = k + 1; continue;
        }
        if t == "," { k = k + 1; continue; }
        // 原子：路径 / 推断 / 数组长度字面（in_len 时按字面）
        let mut atom = String::from("");
        if in_len == 1 {
            atom = t;   // 数组长度表达式（如 (int 4) / (var N)）按字面渲染
        } else if t == "(var _)" {
            atom = String::from("(infer)");
        } else {
            let nm = typename_of(t);
            atom = "(type " + nm;
            let mut is_gen_base = 0;
            if k + 1 < n {
                if ts.get(k + 1) == "lt" { is_gen_base = 1; }
            }
            if is_gen_base == 1 {
                // 泛型基类型：保持开放（不加闭合括号），由后续 gt 收束
            } else {
                atom = atom + ")";
            }
        }
        stk.push(atom);
        k = k + 1;
    }
    let mut result = String::from("?");
    if stk.len() > 0 {
        let r_opt = stk.pop();
        result = match r_opt { Option::Some(x) => x, Option::None => String::new() };
    }
    result
}

// 解析类型标注 token 序列 -> 规范类型串（M-M2b2：路径 / & / &mut / 泛型，迭代式 @GEN@ 栈）。
//   泛型 `<BASE ARG1 ARG2>` 以 @GEN@ 标记收束，支持任意嵌套。
fn parse_type_tokens(ts: Vec<String>) -> String {
    let n = ts.len;
    let mut k = 0;
    let mut ref_kind = 0;   // 0 无 / 1 & / 2 &mut
    if n > 0 && ts.get(0) == "bitand" {
        ref_kind = 1; k = 1;
        if k < n {
            if ts.get(k) == "mut" { ref_kind = 2; k = k + 1; }
        }
    }
    let core = parse_type_core(ts, k);
    let mut result = core;
    if ref_kind == 1 { result = "(ref " + core + ")"; }
    if ref_kind == 2 { result = "(ref mut " + core + ")"; }
    result
}

// 解析 let 模式 token 序列 -> 规范模式串（非递归，M-M2b2 仅扁平元组）。
//   单标识符 "(var x)" -> "x"；通配 "(var _)" -> "_"；
//   元组 "( a , b )" -> "(tuple-pat a b)"（元素为单标识符或 _）。
//   全程语句式 if + 累加变量，不使用 return / if-表达式作为返回值。
fn parse_pattern_tokens(ts: Vec<String>) -> String {
    let n = ts.len;
    let mut result = String::from("?");
    if n > 0 {
        let first = ts.get(0);
        if first != "(" {
            let t = ts.get(0);
            if t == "(var _)" { result = String::from("_"); }
            else { result = typename_of(t); }
        } else {
            let mut s = String::from("(tuple-pat");
            let mut k = 1;
            while k < n {
                let t = ts.get(k);
                if t == ")" { break; }
                if t == "," { k = k + 1; continue; }
                let mut elem = String::from("");
                if t == "(var _)" { elem = String::from("_"); }
                else { elem = typename_of(t); }
                s = s + " " + elem;
                k = k + 1;
            }
            result = s + ")";
        }
    }
    result
}

// 解析单个模式原子 token -> 规范模式串（M-M5）：
//   (var x) -> x / (var _) -> _ / (var true|false) -> (lit-bool ..) /
//   (int N) -> (lit-int N) / (str S) -> (lit-str S) / .. -> .. / 其余 -> ?
fn pat_atom(t: String) -> String {
    if t == "(var _)" { return String::from("_"); }
    if t == "(var true)" { return String::from("(lit-bool true)"); }
    if t == "(var false)" { return String::from("(lit-bool false)"); }
    if t == "(bool true)" { return String::from("(lit-bool true)"); }
    if t == "(bool false)" { return String::from("(lit-bool false)"); }
    if t == ".." { return String::from(".."); }
    if t.substring(0, 5) == "(int " {
        let inner = t.substring(5, t.len - 1);   // (int N) -> N
        return "(lit-int " + inner + ")";
    }
    if t.substring(0, 5) == "(str " {
        let inner = t.substring(5, t.len - 1);
        return "(lit-str " + inner + ")";
    }
    if t.substring(0, 5) == "(var " {
        let inner = t.substring(5, t.len - 1);   // (var NAME) -> NAME
        return inner;
    }
    String::from("?")
}

// 解析模式 token 序列 -> 规范模式串（M-M5，非递归、单向依赖）：
//   或模式：含 bitor -> 按首个 bitor 拆左右（两侧经 render_or_side 渲染）；
//   范围：含 ..< / ... / <.. -> (range-pat L U li ui)，边界按表达式渲染（对齐 oracle 的 (int N)）；
//   元组：以 ( 起 ) 收 -> 按 , 拆段，元素为单 token / 范围（不递归，嵌套元组超出 M-M5a）；
//   单 token -> pat_atom。
fn render_range_pat(ts: Vec<String>) -> String {
    let n = ts.len;
    let mut l0 = 0; let mut u0 = 0; let mut q = 0;
    while q < n {
        let t = ts.get(q);
        if t == "..<" || t == "..." || t == "<.." { l0 = q; u0 = q + 1; break; }
        q = q + 1;
    }
    let rop = ts.get(l0);
    let mut lv: Vec<String> = Vec::new();
    let mut a = 0;
    while a < l0 { lv.push(ts.get(a)); a = a + 1; }
    let mut uv: Vec<String> = Vec::new();
    let mut b = u0;
    while b < n { uv.push(ts.get(b)); b = b + 1; }
    // 边界按表达式渲染（与 oracle 的 render_expr_canonical 对齐：整数字面量 -> (int N)）
    let ls = if lv.len == 1 { parse_expr(lv, 0).s } else { pat_atom(lv.get(0)) };
    let us = if uv.len == 1 { parse_expr(uv, 0).s } else { pat_atom(uv.get(0)) };
    let mut li = 1; let mut ui = 1;
    if rop == "..<" { li = 1; ui = 0; }
    else if rop == "..." { li = 1; ui = 1; }
    else if rop == "<.." { li = 0; ui = 1; }
    "(range-pat " + ls + " " + us + " " + int_to_string(li) + " " + int_to_string(ui) + ")"
}

// 或模式单侧渲染（单 token -> pat_atom；含范围运算符 -> render_range_pat；其余 -> pat_atom(0)）
fn render_or_side(seg: Vec<String>) -> String {
    let n = seg.len;
    if n == 1 { return pat_atom(seg.get(0)); }
    let mut rk = 0; let mut is_r = 0;
    while rk < n {
        let t = seg.get(rk);
        if t == "..<" || t == "..." || t == "<.." { is_r = 1; break; }
        rk = rk + 1;
    }
    if is_r == 1 { return render_range_pat(seg); }
    pat_atom(seg.get(0))
}

// 元组模式渲染：按 , 拆段，每段经 render_or_side（单 token / 范围）；不递归（M-M5a 仅扁平单 token 元素）
fn render_tuple_pat(ts: Vec<String>) -> String {
    let n = ts.len;
    let mut s = String::from("(tuple-pat");
    let mut seg: Vec<String> = Vec::new();
    let mut i = 1;
    while i < n - 1 {
        let t = ts.get(i);
        if t == "," {
            if seg.len > 0 { s = s + " " + render_or_side(seg); }
            seg = Vec::new();
            i = i + 1;
            continue;
        }
        seg.push(t);
        i = i + 1;
    }
    if seg.len > 0 { s = s + " " + render_or_side(seg); }
    s = s + ")";
    s
}

fn parse_match_pattern(ts: Vec<String>) -> String {
    let n = ts.len;
    if n == 0 { return String::from("?"); }
    // 或模式（最低优先级）：含 bitor -> 按首个 bitor 拆左右
    let mut k = 0; let mut has_or = 0; let mut oi = 0;
    while k < n {
        if ts.get(k) == "bitor" { has_or = 1; oi = k; break; }
        k = k + 1;
    }
    if has_or == 1 {
        let mut left: Vec<String> = Vec::new();
        let mut m = 0;
        while m < oi { left.push(ts.get(m)); m = m + 1; }
        let mut right: Vec<String> = Vec::new();
        let mut p = oi + 1;
        while p < n { right.push(ts.get(p)); p = p + 1; }
        return "(or " + render_or_side(left) + " " + render_or_side(right) + ")";
    }
    // 范围模式：含 ..< / ... / <..
    let mut rk = 0; let mut is_range = 0;
    while rk < n {
        let t = ts.get(rk);
        if t == "..<" || t == "..." || t == "<.." { is_range = 1; break; }
        rk = rk + 1;
    }
    if is_range == 1 { return render_range_pat(ts); }
    // 元组：以 ( 起始且 ) 收尾
    if ts.get(0) == "(" && ts.get(n - 1) == ")" { return render_tuple_pat(ts); }
    // 单 token
    pat_atom(ts.get(0))
}

// 渲染单个函数参数段 -> " (param [mut] NAME TYPE)"（M-M6）。
//   段形如 [mut] NAME : TYPE（TYPE 可含泛型，>> 已拆为两个 gt）。
fn render_param(seg: Vec<String>) -> String {
    let n = seg.len;
    if n == 0 { return String::new(); }
    let mut k = 0;
    let mut is_mut = 0;
    if seg.get(0) == "mut" { is_mut = 1; k = 1; }
    let mut name = String::from("?");
    if k < n { name = typename_of(seg.get(k)); }
    k = k + 1;
    let mut typ = String::from("?");
    let mut has_colon = 0;
    if k < n {
        if seg.get(k) == ":" { has_colon = 1; }
    }
    if has_colon == 1 {
        k = k + 1;
        let mut tt: Vec<String> = Vec::new();
        while k < n { tt.push(seg.get(k)); k = k + 1; }
        let mut fixed: Vec<String> = Vec::new();
        let mut fi = 0;
        while fi < tt.len {
            if tt.get(fi) == "shr" { fixed.push(String::from("gt")); fixed.push(String::from("gt")); }
            else { fixed.push(tt.get(fi)); }
            fi = fi + 1;
        }
        typ = parse_type_tokens(fixed);
    }
    let mut s = String::from(" (param ");
    if is_mut == 1 { s = s + "mut "; }
    s = s + name + " " + typ + ")";
    s
}

// 解析函数项头（M-M6）：`<name>(<params>) [-> <ret>]` -> 规范 header 串 + 新位置（指向 body '{'）。
//   独立成函数以缩小 parse_program（避免过多局部变量致编译产物异常）。
fn parse_fn_header(tokens: Vec<String>, ti0: i64, is_pub: i64) -> FHdr {
    let n = tokens.len;
    let mut ti = ti0;
    let name = typename_of(tokens.get(ti));
    ti = ti + 1;
    // 参数列表：越过 '('，收集至匹配的 ')'（&& 不短路，故用嵌套 if 保护 get）
    if ti < n {
        if tokens.get(ti) == "(" { ti = ti + 1; }
    }
    let mut ptoks: Vec<String> = Vec::new();
    let mut pdepth = 0;
    while ti < n {
        let t = tokens.get(ti);
        if t == ")" && pdepth == 0 { ti = ti + 1; break; }
        if t == "(" { pdepth = pdepth + 1; }
        if t == ")" { pdepth = pdepth - 1; }
        ptoks.push(t);
        ti = ti + 1;
    }
    // 拆参数（按 ','），每段 NAME [: TYPE]
    let mut pstr = String::new();
    let mut seg: Vec<String> = Vec::new();
    let pn = ptoks.len;
    let mut pi = 0;
    while pi < pn {
        let t = ptoks.get(pi);
        if t == "," {
            pstr = pstr + render_param(seg);
            seg = Vec::new();
            pi = pi + 1;
            continue;
        }
        seg.push(t);
        pi = pi + 1;
    }
    if seg.len > 0 { pstr = pstr + render_param(seg); }
    // 返回类型（可选）：-> TYPE 直到 '{'
    let mut ret = String::new();
    let mut has_arrow = 0;
    if ti < n {
        if tokens.get(ti) == "arrow" { has_arrow = 1; }
    }
    if has_arrow == 1 {
        ti = ti + 1;
        let mut rtoks: Vec<String> = Vec::new();
        while ti < n {
            if tokens.get(ti) == "{" { break; }
            rtoks.push(tokens.get(ti));
            ti = ti + 1;
        }
        let mut fixed: Vec<String> = Vec::new();
        let mut fi = 0;
        while fi < rtoks.len {
            if rtoks.get(fi) == "shr" { fixed.push(String::from("gt")); fixed.push(String::from("gt")); }
            else { fixed.push(rtoks.get(fi)); }
            fi = fi + 1;
        }
        ret = parse_type_tokens(fixed);
    }
    let mut header = String::new();
    if is_pub == 1 { header = String::from("pub "); }
    header = header + name + " (params" + pstr + ")";
    if ret.len > 0 { header = header + " " + ret; }
    FHdr { h: header, ti: ti }
}

// 解析整段程序源码，返回规范 S-表达式程序文本（见本文件顶部"规范格式"）。
fn parse_program(src: String) -> String {
    let tokens = tokenize(src);
    let n = tokens.len;
    let mut ti = 0;
    let mut bufstack: Vec<String> = Vec::new();
    bufstack.push("");   // 程序体 buffer
    // 控制帧栈（M-M2c）：与 bufstack 配合解析 if/while/loop 的块体（不引入递归）。
    //   Rlyeh 的 Vec<i64>::get/pop 返回 ptr 而非 i64，故所有整数态均以 String 编码
    //   （int_to_string / "0"/"1"/"2" / "stmt"/"let" / "1"=else-if 子帧）。
    //   cf_kind:   "if"/"while"/"loop"
    //   cf_cond:   条件 S-表达式（if/while）；loop 为 ""
    //   cf_then:  then 块 S-表达式（then '}' 闭合时写入）
    //   cf_else:  else 块 S-表达式（else '}' 闭合时写入；否则 "")
    //   cf_state: "0"=等待 then '}'；"1"=then 已闭合（无 else 或 else 已就绪）；"2"=已见 else 待 else 块 '}'
    //   cf_elseif:"1"=本帧是 else if 子帧，收束结果挂到其下方帧的 else；"0"=否
    //   cf_sink:  "stmt"=收束按 semi/bare 落到父 buffer；"let"=收束组装 (let ...)（pending let）
    //   cf_depth: int_to_string(建帧时 bufstack 深度)，用于判定闭合 '}' 是否属于本帧
    let mut cf_kind: Vec<String> = Vec::new();
    let mut cf_cond: Vec<String> = Vec::new();
    let mut cf_then: Vec<String> = Vec::new();
    let mut cf_else: Vec<String> = Vec::new();
    let mut cf_state: Vec<String> = Vec::new();
    let mut cf_elseif: Vec<String> = Vec::new();
    let mut cf_sink: Vec<String> = Vec::new();
    let mut cf_depth: Vec<String> = Vec::new();
    // cf_pat: for 循环的模式（其余控制帧不占用，按顺序与 cf_kind 同步 pop）
    let mut cf_pat: Vec<String> = Vec::new();
    // M-M5 match 帧专用缓冲（match 同一时刻仅一层在顶层活动，故模式缓冲为全局单份）
    let mut match_pat: Vec<String> = Vec::new();            // 当前臂正在收集的模式/守卫 token
    let mut match_arm_pat_done: Vec<String> = Vec::new();   // 有守卫时暂存的 finalized 模式 token
    let mut match_seen: Vec<String> = Vec::new();           // per-frame："0"=未见 =>（区分 match 自身 {} 与臂 body {}）
    let mut match_arm_pat_str: Vec<String> = Vec::new();    // per-frame：当前臂 finalized 模式串
    let mut match_arm_guard_str: Vec<String> = Vec::new();  // per-frame：当前臂守卫串（无守卫为 ""）
    // pending let 信息（M-M2c：控制流作 let 初始化时暂存，帧收束后组装 (let ...)）
    let mut pend_let_pat: Vec<String> = Vec::new();
    let mut pend_let_type: Vec<String> = Vec::new();
    let mut pend_let_mut: Vec<String> = Vec::new();
    while ti < n {
        let tok = tokens.get(ti);
        // 空语句
        if tok == ";" { ti = ti + 1; continue; }
        // match 帧顶层路由（M-M5）：仅当 bufstack 深度 == match 帧深度时拦截模式/=>/守卫/,/{}
        if cf_kind.len() > 0 {
            let mk_idx = cf_kind.len() - 1;
            if cf_kind.get(mk_idx) == "match" {
                let mk_depth = cf_depth.get(mk_idx);
                if int_to_string(bufstack.len()) == mk_depth {
                    // 顶层：模式收集 / => / if(守卫) / ,(臂分隔) / {}(body 或 match 自身花括号)
                    if tok == "{" {
                        if match_seen.get(mk_idx) == "0" {
                            // match 自身的花括号：跳过，不入 buffer 栈
                            match_seen.set(mk_idx, String::from("1"));
                            ti = ti + 1;
                            continue;
                        }
                        // 臂 body 开始：正常入 buffer 栈
                        ti = ti + 1;
                        bufstack.push("");
                        continue;
                    }
                    if tok == "}" {
                        // 闭合 match：组装 (match <SUBJ> <ARMS>)
                        let subject = cf_cond.get(mk_idx);
                        let arms = cf_then.get(mk_idx);
                        let node = "(match " + subject + arms + ")";
                        ti = ti + 1;
                        cf_kind.pop(); cf_cond.pop(); cf_then.pop(); cf_else.pop();
                        cf_state.pop(); cf_elseif.pop(); cf_sink.pop(); cf_depth.pop();
                        cf_pat.pop(); match_seen.pop();
                        match_arm_pat_str.pop(); match_arm_guard_str.pop();
                        while match_pat.len() > 0 { match_pat.pop(); }
                        while match_arm_pat_done.len() > 0 { match_arm_pat_done.pop(); }
                        let nxt = if ti < n { tokens.get(ti) } else { String::from("") };
                        let mut s = node;
                        if nxt == ";" { ti = ti + 1; s = "(semi " + node + ")"; }
                        else if nxt == "}" { s = node; }
                        else { s = "(semi " + node + ")"; }
                        let po = bufstack.pop();
                        let mut p = match po { Option::Some(x) => x, Option::None => String::new() };
                        p = p + " " + s;
                        bufstack.push(p);
                        continue;
                    }
                    if tok == "fatarrow" {
                        // 收束当前臂的模式/守卫
                        let st = cf_state.get(mk_idx);
                        let mut pat = String::new();
                        let mut grd = String::new();
                        if st == "3" {
                            // 有守卫：pattern 存于 match_arm_pat_done，guard 存于 match_pat
                            pat = parse_match_pattern(match_arm_pat_done);
                            let ge = parse_expr(match_pat, 0);
                            grd = ge.s;
                            while match_arm_pat_done.len() > 0 { match_arm_pat_done.pop(); }
                        } else {
                            pat = parse_match_pattern(match_pat);
                        }
                        while match_pat.len() > 0 { match_pat.pop(); }
                        match_arm_pat_str.set(mk_idx, pat);
                        match_arm_guard_str.set(mk_idx, grd);
                        cf_state.set(mk_idx, String::from("1"));
                        ti = ti + 1;
                        continue;
                    }
                    if tok == "if" && cf_state.get(mk_idx) == "0" {
                        // 进入守卫收集：把已收集模式 token 转存到 match_arm_pat_done
                        let mut kk = 0;
                        while kk < match_pat.len() {
                            match_arm_pat_done.push(match_pat.get(kk));
                            kk = kk + 1;
                        }
                        while match_pat.len() > 0 { match_pat.pop(); }
                        cf_state.set(mk_idx, String::from("3"));
                        ti = ti + 1;
                        continue;
                    }
                    if tok == "," {
                        if match_pat.len() > 0 {
                            // 模式内逗号（元组 / 守卫中的调用参数）：累积进 match_pat
                            match_pat.push(tok);
                            ti = ti + 1;
                            continue;
                        }
                        // 臂分隔符（body 已闭合、match_pat 为空，等待下一臂）
                        ti = ti + 1;
                        continue;
                    }
                    // 其余：模式 token，累积到 match_pat
                    match_pat.push(tok);
                    ti = ti + 1;
                    continue;
                }
            }
        }
        // 块结束
        if tok == "}" {
            if bufstack.len() > 1 {
                ti = ti + 1;
                let body_opt = bufstack.pop();
                let body = match body_opt { Option::Some(x) => x, Option::None => String::new() };
                let block = "(block" + body + ")";
                // 是否属于某个控制帧的块体？
                if cf_state.len() > 0 {
                    let t_idx = cf_state.len() - 1;
                    let top_state = cf_state.get(t_idx);
                    let top_depth = cf_depth.get(t_idx);
                    let top_kind = cf_kind.get(t_idx);
                    if int_to_string(bufstack.len()) == top_depth {
                        if top_kind == "match" {
                            // match 臂闭合：body = 刚弹出的块；组装 (arm <pat> [<guard>] <body>)
                            let pat = match_arm_pat_str.get(t_idx);
                            let grd = match_arm_guard_str.get(t_idx);
                            let mut arm = String::from("(arm ");
                            arm = arm + pat;
                            if grd.len() > 0 { arm = arm + " " + grd; }
                            arm = arm + " " + block + ")";
                            let at = cf_then.get(t_idx);
                            cf_then.set(t_idx, at + " " + arm);
                            cf_state.set(t_idx, String::from("0"));  // 等待下一臂或 match 闭合
                            continue;
                        }
                        if top_kind == "fn" {
                            // 函数体闭合：组装 (fn <HDR> (block ...)) 并落到父 buffer（裸）
                            let hdr = cf_cond.get(t_idx);
                            let node = "(fn " + hdr + " " + block + ")";
                            cf_kind.pop(); cf_cond.pop(); cf_then.pop(); cf_else.pop();
                            cf_state.pop(); cf_elseif.pop(); cf_sink.pop(); cf_depth.pop(); cf_pat.pop();
                            let po = bufstack.pop();
                            let mut p = match po { Option::Some(x) => x, Option::None => String::new() };
                            p = p + " " + node;
                            bufstack.push(p);
                            continue;
                        }
                        // 该 '}' 闭合的是控制帧自身的 then/else 块
                        let mut do_fin = 0;
                        if top_state == "0" {
                            cf_then.set(t_idx, block);
                            cf_state.set(t_idx, String::from("1"));
                            let nxt = if ti < n { tokens.get(ti) } else { String::from("") };
                            if nxt == "else" {
                                ti = ti + 1;
                                let n2 = if ti < n { tokens.get(ti) } else { String::from("") };
                                if n2 == "{" {
                                    cf_state.set(t_idx, String::from("2"));   // 待 else 块 '}'
                                } else if n2 == "if" {
                                    // else if：开启新帧（elseif 子帧），收束结果挂到其下方帧的 else
                                    ti = ti + 1;
                                    let r = parse_expr(tokens, ti);
                                    cf_kind.push(String::from("if"));
                                    cf_cond.push(r.s);
                                    cf_then.push(String::new());
                                    cf_else.push(String::new());
                                    cf_state.push(String::from("0"));
                                    cf_elseif.push(String::from("1"));
                                    cf_sink.push(String::from("stmt"));
                                    cf_depth.push(int_to_string(bufstack.len()));
                                    ti = r.ti;
                                } else {
                                    do_fin = 1;   // else 后非块非 if：按无 else 收束
                                }
                            } else {
                                do_fin = 1;       // 无 else：收束
                            }
                        } else {
                            // top_state == "2"：else 块闭合；或异常态：直接收束
                            if top_state == "2" {
                                cf_else.set(t_idx, block);
                                cf_state.set(t_idx, String::from("1"));
                            }
                            do_fin = 1;
                        }
                        if do_fin == 1 {
                            // 控制帧收束（含 else if 级联）：组装节点并落到目标
                            let mut fin = 1;
                            while fin == 1 {
                                let fk_o = cf_kind.pop();
                                let fk = match fk_o { Option::Some(x) => x, Option::None => String::new() };
                                let fc_o = cf_cond.pop();
                                let fc = match fc_o { Option::Some(x) => x, Option::None => String::new() };
                                let ft_o = cf_then.pop();
                                let ft = match ft_o { Option::Some(x) => x, Option::None => String::new() };
                                let fe_o = cf_else.pop();
                                let fe = match fe_o { Option::Some(x) => x, Option::None => String::new() };
                                let fs_o = cf_state.pop();
                                let fs = match fs_o { Option::Some(x) => x, Option::None => String::new() };
                                let feif_o = cf_elseif.pop();
                                let feif = match feif_o { Option::Some(x) => x, Option::None => String::new() };
                                let fd_o = cf_depth.pop();
                                let fd = match fd_o { Option::Some(x) => x, Option::None => String::new() };
                                let sk_o = cf_sink.pop();
                                let sk = match sk_o { Option::Some(x) => x, Option::None => String::new() };
                                let fp_o = cf_pat.pop();
                                let fp = match fp_o { Option::Some(x) => x, Option::None => String::new() };
                                let mut node = String::new();
                                if fk == "if" {
                                    node = "(if " + fc + " " + ft;
                                    if fe.len() > 0 { node = node + " " + fe; }
                                    node = node + ")";
                                } else if fk == "while" {
                                    node = "(while " + fc + " " + ft + ")";
                                } else if fk == "for" {
                                    node = "(for " + fp + " " + fc + " " + ft + ")";
                                } else {
                                    node = "(loop " + ft + ")";
                                }
                                if feif == "1" {
                                    // 级联：else if 的 else 分支是块，故将嵌套 if 包成 (block ...)
                                    // 再挂到下方帧的 else，继续收束下方帧
                                    let below = cf_else.len() - 1;
                                    cf_else.set(below, "(block " + node + ")");
                                    cf_state.set(below, String::from("1"));
                                } else if sk == "let" {
                                    // 挂到 pending let 初始化
                                    let lp_o = pend_let_pat.pop();
                                    let lp = match lp_o { Option::Some(x) => x, Option::None => String::new() };
                                    let lt_o = pend_let_type.pop();
                                    let lt = match lt_o { Option::Some(x) => x, Option::None => String::new() };
                                    let lm_o = pend_let_mut.pop();
                                    let lm = match lm_o { Option::Some(x) => x, Option::None => String::new() };
                                    let mut ln = String::new();
                                    if lm == "1" { ln = "(let mut " + lp; } else { ln = "(let " + lp; }
                                    if lt.len() > 0 { ln = ln + " " + lt; }
                                    ln = ln + " " + node + ")";
                                    let po = bufstack.pop();
                                    let mut pp = match po { Option::Some(x) => x, Option::None => String::new() };
                                    pp = pp + " " + ln;
                                    bufstack.push(pp);
                                    fin = 0;
                                } else {
                                    // 顶层控制语句：按 semi/bare 落到父 buffer
                                    let nx = if ti < n { tokens.get(ti) } else { String::from("") };
                                    let mut s = node;
                                    if nx == ";" { ti = ti + 1; s = "(semi " + node + ")"; }
                                    else if nx == "}" { s = node; }
                                    else { s = "(semi " + node + ")"; }
                                    let po = bufstack.pop();
                                    let mut pp = match po { Option::Some(x) => x, Option::None => String::new() };
                                    pp = pp + " " + s;
                                    bufstack.push(pp);
                                    fin = 0;
                                }
                            }
                        }
                        continue;
                    }
                }
                // 普通块闭合（控制帧体内的内层块 / 非控制块）
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
        // let 语句（M-M2b2：支持模式（扁平元组 / _）与类型标注）
        if tok == "let" {
            ti = ti + 1;
            let mut is_mut = 0;
            if ti < n && tokens.get(ti) == "mut" { is_mut = 1; ti = ti + 1; }
            // 收集模式 token，直到 ':' 或 '='
            let mut pat_toks: Vec<String> = Vec::new();
            while ti < n {
                let p = tokens.get(ti);
                if p == ":" { break; }
                if p == "=" { break; }
                pat_toks.push(p);
                ti = ti + 1;
            }
            let pattern = parse_pattern_tokens(pat_toks);
            // 类型标注（可选）
            let mut type_sexpr = String::from("");
            if ti < n && tokens.get(ti) == ":" {
                let mut typ_toks: Vec<String> = Vec::new();
                ti = ti + 1;
                while ti < n {
                    let q = tokens.get(ti);
                    if q == "=" { break; }
                    typ_toks.push(q);
                    ti = ti + 1;
                }
                // 类型上下文：嵌套泛型闭合的 ">>" 被词法记为右移 shr，需拆成两个 '>'
                let mut fixed: Vec<String> = Vec::new();
                let mut fi = 0;
                while fi < typ_toks.len {
                    let ft = typ_toks.get(fi);
                    if ft == "shr" {
                        fixed.push(String::from("gt"));
                        fixed.push(String::from("gt"));
                    } else {
                        fixed.push(ft);
                    }
                    fi = fi + 1;
                }
                type_sexpr = parse_type_tokens(fixed);
            }
            // 越过 '='
            if ti < n && tokens.get(ti) == "=" { ti = ti + 1; }
            // 控制流作 let 初始化表达式（M-M2c：表达式双形态之一）
            if ti < n && (tokens.get(ti) == "if" || tokens.get(ti) == "while" || tokens.get(ti) == "loop") {
                let ck = tokens.get(ti);
                ti = ti + 1;
                let mut cinit = String::new();
                if ck == "loop" {
                    cinit = String::new();
                } else {
                    let r2 = parse_expr(tokens, ti);
                    cinit = r2.s;
                    ti = r2.ti;
                }
                pend_let_pat.push(pattern);
                pend_let_type.push(type_sexpr);
                pend_let_mut.push(int_to_string(is_mut));
                cf_kind.push(ck);
                cf_cond.push(cinit);
                cf_then.push(String::new());
                cf_else.push(String::new());
                cf_state.push(String::from("0"));
                cf_elseif.push(String::from("0"));
                cf_sink.push(String::from("let"));
                cf_depth.push(int_to_string(bufstack.len()));
                continue;
            }
            let r = parse_expr(tokens, ti);
            let init = r.s;
            ti = r.ti;
            if ti < n && tokens.get(ti) == ";" { ti = ti + 1; }
            let mut node = String::new();
            if is_mut == 1 { node = "(let mut " + pattern; }
            else { node = "(let " + pattern; }
            if type_sexpr.len > 0 { node = node + " " + type_sexpr; }
            node = node + " " + init + ")";
            let p_opt = bufstack.pop();
            let mut p = match p_opt { Option::Some(x) => x, Option::None => String::new() };
            p = p + " " + node;
            bufstack.push(p);
            continue;
        }
        // 控制流语句（M-M2c）：if / while / loop（语句形态）
        if tok == "if" {
            ti = ti + 1;
            let r = parse_expr(tokens, ti);
            cf_kind.push(String::from("if"));
            cf_cond.push(r.s);
            cf_then.push(String::new());
            cf_else.push(String::new());
            cf_state.push(String::from("0"));
            cf_elseif.push(String::from("0"));
            cf_sink.push(String::from("stmt"));
            cf_depth.push(int_to_string(bufstack.len()));
            ti = r.ti;
            continue;
        }
        if tok == "while" {
            ti = ti + 1;
            let r = parse_expr(tokens, ti);
            cf_kind.push(String::from("while"));
            cf_cond.push(r.s);
            cf_then.push(String::new());
            cf_else.push(String::new());
            cf_state.push(String::from("0"));
            cf_elseif.push(String::from("0"));
            cf_sink.push(String::from("stmt"));
            cf_depth.push(int_to_string(bufstack.len()));
            ti = r.ti;
            continue;
        }
        if tok == "loop" {
            ti = ti + 1;
            cf_kind.push(String::from("loop"));
            cf_cond.push(String::new());
            cf_then.push(String::new());
            cf_else.push(String::new());
            cf_state.push(String::from("0"));
            cf_elseif.push(String::from("0"));
            cf_sink.push(String::from("stmt"));
            cf_depth.push(int_to_string(bufstack.len()));
            continue;
        }
        // for 循环（M-M4）：for <pattern> in <iterator> <block>
        if tok == "for" {
            ti = ti + 1;
            // 收集模式 token 直到 'in'
            let mut pat_toks: Vec<String> = Vec::new();
            while ti < n {
                let p = tokens.get(ti);
                if p == "in" { break; }
                pat_toks.push(p);
                ti = ti + 1;
            }
            let pattern = parse_pattern_tokens(pat_toks);
            // 越过 'in'
            if ti < n && tokens.get(ti) == "in" { ti = ti + 1; }
            // 解析迭代器表达式直到块体 '{'
            let r = parse_expr(tokens, ti);
            let iter = r.s;
            ti = r.ti;
            // 推帧（复用控制帧机制：cf_cond 存迭代器、cf_pat 存模式）
            cf_kind.push(String::from("for"));
            cf_cond.push(iter);
            cf_pat.push(pattern);
            cf_then.push(String::new());
            cf_else.push(String::new());
            cf_state.push(String::from("0"));
            cf_elseif.push(String::from("0"));
            cf_sink.push(String::from("stmt"));
            cf_depth.push(int_to_string(bufstack.len()));
            continue;
        }
        // match 表达式（M-M5）：match <subject> { <pat> => <block>, ... }
        if tok == "match" {
            ti = ti + 1;
            // 解析被匹配表达式直到 '{'
            let r = parse_expr(tokens, ti);
            let subject = r.s;
            ti = r.ti;
            // 越过 match 自身的 '{'（不入 buffer 栈）
            if ti < n && tokens.get(ti) == "{" { ti = ti + 1; }
            // 清空全局模式缓冲
            while match_pat.len() > 0 { match_pat.pop(); }
            while match_arm_pat_done.len() > 0 { match_arm_pat_done.pop(); }
            cf_kind.push(String::from("match"));
            cf_cond.push(subject);
            cf_then.push(String::new());   // 累积臂：(arm ...)(arm ...)
            cf_else.push(String::new());
            cf_state.push(String::from("0"));  // 0=收集模式 / 3=收集守卫 / 1=等待 body
            cf_elseif.push(String::from("0"));
            cf_sink.push(String::from("stmt"));
            cf_depth.push(int_to_string(bufstack.len()));
            cf_pat.push(String::new());
            match_seen.push(String::from("1"));        // match 自身 '{' 已在分发处跳过，路由中 '{' 一律为臂 body
            match_arm_pat_str.push(String::new());     // 当前臂 finalized 模式串
            match_arm_guard_str.push(String::new());   // 当前臂守卫串（无守卫为 ""）
            continue;
        }
        // 函数项（M-M6）：[pub] fn <name>(<params>) [-> <ret>] <block>
        // 注意：Rlyeh 的 && 不短路，故下一 token 须先安全预取（不可写 tokens.get(ti+1)）
        let mut peek1 = String::new();
        if ti + 1 < n { peek1 = tokens.get(ti + 1); }
        if tok == "fn" || (tok == "pub" && peek1 == "fn") {
            let mut fpub = 0;
            if tok == "pub" { fpub = 1; ti = ti + 1; }   // 跨越 pub
            ti = ti + 1;                                  // 跨越 fn
            // 解析函数头（名 / 参数 / 返回类型）-> 规范 header 串 + 新位置
            let h = parse_fn_header(tokens, ti, fpub);
            let header = h.h;
            ti = h.ti;
            // 推 fn 帧（body 块由 '{' 分支入 buffer 栈，'}' 收束时组装）
            cf_kind.push(String::from("fn"));
            cf_cond.push(header);
            cf_then.push(String::new());
            cf_else.push(String::new());
            cf_state.push(String::from("0"));
            cf_elseif.push(String::from("0"));
            cf_sink.push(String::from("stmt"));
            cf_depth.push(int_to_string(bufstack.len()));
            cf_pat.push(String::new());
            continue;
        }
        // 非 fn 的 pub 项（M-M6a 未覆盖）：跳过 pub token
        if tok == "pub" { ti = ti + 1; continue; }
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
