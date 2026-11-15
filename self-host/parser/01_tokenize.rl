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
            while i < n && p_is_ident_cont(src.get(i)) == 1 { i = i + 1; }
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
            else if name == "struct" { toks.push(String::from("struct")); }
            else if name == "enum" { toks.push(String::from("enum")); }
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
        if c == 58 {
            let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
            if c2 == 58 { toks.push(String::from("pathsep")); prev_op = 1; i = i + 2; continue; }   // :: 路径分隔（M-M9）
            toks.push(String::from(":")); prev_op = 0; i = i + 1; continue;
        }
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

