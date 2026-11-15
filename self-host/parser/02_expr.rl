// ============================================================================
// M-M2e + M-M3：表达式后缀统一标记化（字段 .field / 索引 [ ] / 调用 ( ) /
//   元组字面量 ( a, b ) / 数组字面量 [ a, b ]）。
//   设计：shunting-yard 主循环把每个后缀转成 RPN 标记，末位建树阶段归约。
//   规避 Rlyeh 编译器对前向递归 / if-表达式返回值的已知限制——全程迭代，
//   索引/调用/元组/数组的子表达式直接在 RPN 内展开（靠 opstack 括号界定的
//   运算符冲刷），不需要递归调用 parse_expr。
// M-M3b（SH-P2-7）：建树阶段不再拼接规范文本，而是调用 ast.rl 的共享 arena
//   构造辅助（ast_push_leaf / ast_push_list / ast_cat / ast_push_atom）**直接
//   构造类型化 AstNode**；表达式规范文本最终由 render_node 统一渲染。
//   操作数栈 ast 中保存的是「节点索引串」，记号（@PARENSENT@ 等）仍是哨兵串。
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

// 操作数 token -> arena 叶节点，返回节点索引串（M-M3b）。
// token 形态：(int N) / (var NAME) / (str 内容) / (bool true|false)；其余整段作原子。
fn operand_node(nodes: &mut Vec<AstNode>, tok: String) -> String {
    let n = tok.len;
    let mut res = String::from("");
    if n > 5 {
        if tok.substring(0, 5) == "(int " {
            res = ast_push_leaf(nodes, String::from("int"), tok.substring(5, n - 1));
        } else if tok.substring(0, 5) == "(var " {
            res = ast_push_leaf(nodes, String::from("var"), tok.substring(5, n - 1));
        } else if tok.substring(0, 5) == "(str " {
            res = ast_push_leaf(nodes, String::from("str"), tok.substring(5, n - 1));
        }
    }
    if res == "" {
        if n > 6 {
            if tok.substring(0, 6) == "(bool " {
                res = ast_push_leaf(nodes, String::from("bool"), tok.substring(6, n - 1));
            }
        }
    }
    if res == "" { res = ast_push_atom(nodes, tok); }
    res
}

// 从 token 流位置 ti 起解析一个表达式（共享 arena 版，M-M3b）：
// 返回 PRes{ id: 表达式根节点索引串, ti: 新位置 }；s 留空（供 wrapper 填文本）。
// 表达式在以下 token 处停止：';' '}' '=' ':' '{' 'let' 及 EOF；顶层 ','（非调用
//   元组/数组上下文）亦停止（交由语句层处理）。
fn parse_expr_arena(tokens: Vec<String>, ti0: i64, nodes: &mut Vec<AstNode>) -> PRes {
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
        // 路径后缀 ::seg（M-M9）：主循环只把段名记为 RPN 标记 "@path@SEG"，
        //   建树阶段（M-M3b）再把前一操作数节点与段名合并为 (path ...)（左结合）。
        //   Rlyeh && 不短路，故下一 token 须先安全预取。
        if tok == "pathsep" {
            let mut seg_tok = String::new();
            if ti + 1 < n { seg_tok = tokens.get(ti + 1); }
            let seg = typename_of(seg_tok);
            rpn.push("@path@" + seg);
            prev_op = 1;
            ti = ti + 2;   // 越过 pathsep 与段名
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
    // 由 RPN 迭代建树（M-M3b：直接构造 arena 节点，ast 栈存节点索引串）
    let mut ast: Vec<String> = Vec::new();
    let mut k = 0;
    while k < rpn.len {
        let tk = rpn.get(k);
        let f = tk.get(0);
        if f == 40 {
            let id = operand_node(nodes, tk);
            ast.push(id);
        } else if tk == "u-" {
            // 弹出操作数（跳过分组/元组/调用/数组等哨兵，它们非真实节点）
            let mut a = String::from("");
            let mut sk = 1;
            while sk == 1 {
                let ao = ast.pop();
                let av = match ao { Option::Some(x) => x, Option::None => String::new() };
                if av == "" { sk = 0; break; }
                if av == "@PARENSENT@" || av == "@TUPSEP@" || av == "@ARGSEP@" || av == "@OPEN@" || av == "@ARRSENT@" || av == "@ARRSEP@" {
                    continue;
                }
                a = av; sk = 0; break;
            }
            if a == "" { a = ast_push_atom(nodes, String::from("?")); }
            ast.push(ast_push_list(nodes, String::from("neg"), a));
        } else if f == 64 {
            // '@' 起头的后缀标记
            if tk.substring(0, 7) == "@field@" {
                let name = tk.substring(7, tk.len);
                let a_opt = ast.pop();
                let mut a = match a_opt { Option::Some(x) => x, Option::None => String::new() };
                if a == "" { a = ast_push_atom(nodes, String::from("?")); }
                let name_atom = ast_push_atom(nodes, name);
                ast.push(ast_push_list(nodes, String::from("field"), ast_cat(a, name_atom)));
            } else if tk.substring(0, 6) == "@path@" {
                // 路径归约（M-M9/M-M3b）：左操作数若为 (var X) 则新建 (path X SEG)；
                // 若已是 (path ...) 则原地追加 SEG（get 字段 + set 写回，保持左结合）。
                let seg = tk.substring(6, tk.len);
                let a_opt = ast.pop();
                let a = match a_opt { Option::Some(x) => x, Option::None => String::new() };
                let mut node = String::from("");
                if a != "" {
                    if a.get(0) != 64 {
                        let li = string_to_int(a);
                        let nd = nodes.get(li);
                        let nkind = nd.kind;
                        let nhead = nd.head;
                        let nkids = nd.kids;
                        if nkind == "L" {
                            if nhead == "path" {
                                let seg_atom = ast_push_atom(nodes, seg);
                                let kids2 = ast_cat(nkids, seg_atom);
                                nodes.set(li, list_node(String::from("path"), kids2));
                                node = a;
                            } else if nhead == "var" {
                                // 原地把 (var X) 节点改写为 (path X SEG)，复用其原子子节点，
                                // 避免留下孤儿 var 列表节点（节点数与 AST 模型对拍要求无孤儿）。
                                let seg_atom = ast_push_atom(nodes, seg);
                                let kids2 = ast_cat(nkids, seg_atom);
                                nodes.set(li, list_node(String::from("path"), kids2));
                                node = a;
                            }
                        }
                    }
                }
                if node == "" { node = ast_push_atom(nodes, String::from("?")); }
                ast.push(node);
            } else if tk == "@open-index@" {
                ast.push(String::from("@OPEN@"));
            } else if tk == "@close-index@" {
                let idx_o = ast.pop();
                let idx = match idx_o { Option::Some(x) => x, Option::None => String::from("") };
                if idx == "@OPEN@" {
                    ast.push(ast_push_atom(nodes, String::from("?")));   // 空索引（异常输入）
                } else {
                    let _open = ast.pop();   // 丢弃 @OPEN@
                    // 接收者可能位于嵌套分组哨兵 @PARENSENT@ 之下，跳过之
                    let mut rcv = String::from("");
                    let mut sk = 1;
                    while sk == 1 {
                        let ro = ast.pop();
                        let rv = match ro { Option::Some(x) => x, Option::None => String::new() };
                        if rv == "" { sk = 0; break; }
                        if rv == "@PARENSENT@" { continue; }
                        rcv = rv; sk = 0; break;
                    }
                    if rcv == "" { rcv = ast_push_atom(nodes, String::from("?")); }
                    ast.push(ast_push_list(nodes, String::from("index"), ast_cat(rcv, idx)));
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
                let mut callee = match callee_o { Option::Some(x) => x, Option::None => String::new() };
                if callee == "" { callee = ast_push_atom(nodes, String::from("?")); }
                let mut kids = callee;
                let mut ai2 = 0;
                while ai2 < args.len {
                    kids = ast_cat(kids, args.get(ai2));
                    ai2 = ai2 + 1;
                }
                ast.push(ast_push_list(nodes, String::from("call"), kids));
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
                let mut kids = String::from("");
                let mut ai2 = 0;
                while ai2 < elems.len {
                    kids = ast_cat(kids, elems.get(ai2));
                    ai2 = ai2 + 1;
                }
                ast.push(ast_push_list(nodes, String::from("tuple"), kids));
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
                let mut kids = String::from("");
                let mut ai2 = 0;
                while ai2 < elems.len {
                    kids = ast_cat(kids, elems.get(ai2));
                    ai2 = ai2 + 1;
                }
                ast.push(ast_push_list(nodes, String::from("array-lit"), kids));
            } else {
                ast.push(tk);
            }
        } else {
            if tk.substring(0, 1) == "r" {
                // 范围运算符归约：(range LOWER UPPER lower_inclusive upper_inclusive)
                let bo = ast.pop();
                let mut bv = match bo { Option::Some(x) => x, Option::None => String::new() };
                if bv == "" { bv = ast_push_atom(nodes, String::from("?")); }
                let ao = ast.pop();
                let mut av = match ao { Option::Some(x) => x, Option::None => String::new() };
                if av == "" { av = ast_push_atom(nodes, String::from("?")); }
                let mut li = 1; let mut ui = 1;
                if tk == "r..<" { li = 1; ui = 0; }
                else if tk == "r..." { li = 1; ui = 1; }
                else if tk == "r<.." { li = 0; ui = 1; }
                else { li = 1; ui = 1; }
                let li_atom = ast_push_atom(nodes, int_to_string(li));
                let ui_atom = ast_push_atom(nodes, int_to_string(ui));
                let mut kids = ast_cat(av, bv);
                kids = ast_cat(kids, li_atom);
                kids = ast_cat(kids, ui_atom);
                ast.push(ast_push_list(nodes, String::from("range"), kids));
            } else {
            // 弹出右操作数（跳过分组/元组/调用/数组等哨兵，它们非真实节点）
            let mut b = String::from("");
            let mut sk = 1;
            while sk == 1 {
                let bo = ast.pop();
                let bv = match bo { Option::Some(x) => x, Option::None => String::new() };
                if bv == "" { sk = 0; break; }
                if bv == "@PARENSENT@" || bv == "@TUPSEP@" || bv == "@ARGSEP@" || bv == "@OPEN@" || bv == "@ARRSENT@" || bv == "@ARRSEP@" {
                    continue;
                }
                b = bv; sk = 0; break;
            }
            // 弹出左操作数（跳过哨兵）
            let mut a = String::from("");
            let mut sk2 = 1;
            while sk2 == 1 {
                let ao = ast.pop();
                let av = match ao { Option::Some(x) => x, Option::None => String::new() };
                if av == "" { sk2 = 0; break; }
                if av == "@PARENSENT@" || av == "@TUPSEP@" || av == "@ARGSEP@" || av == "@OPEN@" || av == "@ARRSENT@" || av == "@ARRSEP@" {
                    continue;
                }
                a = av; sk2 = 0; break;
            }
            if a == "" { a = ast_push_atom(nodes, String::from("?")); }
            if b == "" { b = ast_push_atom(nodes, String::from("?")); }
            let op_atom = ast_push_atom(nodes, tk);
            let mut kids = ast_cat(op_atom, a);
            kids = ast_cat(kids, b);
            ast.push(ast_push_list(nodes, String::from("bin"), kids));
            }
        }
        k = k + 1;
    }
    if ast.len() == 0 {
        if ret_flag == 1 {
            let r = ast_push_list(nodes, String::from("return"), String::from(""));
            return PRes { s: String::from(""), id: r, ti: ti };
        }
        let q = ast_push_atom(nodes, String::from("?"));
        return PRes { s: String::from(""), id: q, ti: ti };
    }
    let res = ast.get(ast.len() - 1);
    if ret_flag == 1 {
        let r = ast_push_list(nodes, String::from("return"), res);
        return PRes { s: String::from(""), id: r, ti: ti };
    }
    PRes { s: String::from(""), id: res, ti: ti }
}

// 兼容入口（外部/历史调用）：解析表达式并返回规范文本（M-M3b 内部走共享 arena）。
// 供 harness（self_host_parser.rs）与旧式调用使用；parser 内部统一用 parse_expr_arena。
fn parse_expr(tokens: Vec<String>, ti0: i64) -> PRes {
    let mut nodes: Vec<AstNode> = Vec::new();
    let r = { parse_expr_arena(tokens, ti0, &mut nodes) };
    let txt = render_node(&nodes, r.id);
    PRes { s: txt, id: r.id, ti: r.ti }
}
