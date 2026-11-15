// ============================================================================
// M-M2b2/M-M2b3/M-M3b：类型标注与模式渲染。
//
// M-M3b（SH-P2-7）：所有归约点不再拼接规范文本，而是调用 ast.rl 的共享 arena
// 构造辅助（ast_push_leaf / ast_push_list / ast_cat / ast_push_atom）直接构造
// 类型化节点，返回「节点索引串」；最终由 render_node / render_ast 统一渲染。
// 迭代式显式栈保持不变（@GEN@ / @TUP@ / @ARR@ / @ASEP@ 哨兵；@TB@NAME 为
// 「待收束的泛型基类型」标记，避免生成孤儿节点）。
//
// 顺序约定（Rlyeh 前向调用被推断为 i64）：被调函数定义在调用方之前。
//   operand_node(02_expr) -> parse_type_core -> parse_type_tokens -> ...
// ============================================================================

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
//   '<' 压 @GEN@，'>' 弹出 @GEN@ 之上全部实参 + @GEN@ 之下基类型（@TB@NAME 标记），
//       包成 (type BASE ARG...) 节点；
//   '(' 压 @TUP@，')' 弹出直到 @TUP@ 包成 (tuple-type ...) 节点；
//   '[' 压 @ARR@，';' 压 @ASEP@（其后为长度表达式，原子按字面渲染不包 (type ...)），
//        ']' 弹出直到 @ARR@，以 @ASEP@ 为界拆出 类型 T 与 长度 N，包成 (array T N)。
// 实参/元素逆序弹出后反转还原。必须在 parse_type_tokens 之前定义，避免前向调用推断为 i64。
fn parse_type_core(ts: Vec<String>, start: i64, nodes: &mut Vec<AstNode>) -> String {
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
            // 泛型收束：(type BASE ARG...) 节点
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
            let base = match base_opt { Option::Some(x) => x, Option::None => String::from("@TB@?") };
            let mut bname = base;
            if bname.len > 4 && bname.substring(0, 4) == "@TB@" {
                bname = bname.substring(4, bname.len);
            }
            let mut kids = ast_push_atom(nodes, bname);
            let mut xi = 0;
            while xi < args.len {
                kids = ast_cat(kids, args.get(xi));
                xi = xi + 1;
            }
            stk.push(ast_push_list(nodes, String::from("type"), kids));
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
            let mut kids = String::from("");
            let mut xi = 0;
            while xi < elems.len {
                kids = ast_cat(kids, elems.get(xi));
                xi = xi + 1;
            }
            stk.push(ast_push_list(nodes, String::from("tuple-type"), kids));
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
            // 数组收束：[T; N] -> (array T N) 节点
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
            let mut nstr = String::from("");
            let mut tstr = String::from("");
            if sep_idx >= 0 {
                // 长度部分 = parts[0..sep_idx]（顶->sep），反转还原左到右
                let mut li = sep_idx - 1;
                while li >= 0 {
                    nstr = ast_cat(parts.get(li), nstr);
                    li = li - 1;
                }
                // 类型部分 = parts[sep_idx+1..end]（sep->底，顺序为逆），反转还原左到右
                let mut ti = parts.len - 1;
                while ti > sep_idx {
                    tstr = ast_cat(parts.get(ti), tstr);
                    ti = ti - 1;
                }
            }
            if nstr == "" { nstr = ast_push_atom(nodes, String::from("?")); }
            if tstr == "" { tstr = ast_push_atom(nodes, String::from("?")); }
            stk.push(ast_push_list(nodes, String::from("array"), ast_cat(tstr, nstr)));
            in_len = 0;
            k = k + 1; continue;
        }
        if t == "," { k = k + 1; continue; }
        // 原子：路径 / 推断 / 数组长度字面（in_len 时按字面）
        let mut atom = String::from("");
        if in_len == 1 {
            atom = operand_node(nodes, t);   // 数组长度表达式（如 (int 4)）节点
        } else if t == "(var _)" {
            atom = ast_push_list(nodes, String::from("infer"), String::from(""));
        } else {
            let nm = typename_of(t);
            let mut is_gen_base = 0;
            if k + 1 < n {
                if ts.get(k + 1) == "lt" { is_gen_base = 1; }
            }
            if is_gen_base == 1 {
                // 泛型基类型：暂不建节点，压 @TB@NAME 标记由后续 gt 收束
                atom = "@TB@" + nm;
            } else {
                atom = ast_push_leaf(nodes, String::from("type"), nm);
            }
        }
        stk.push(atom);
        k = k + 1;
    }
    let mut result = String::from("");
    if stk.len() > 0 {
        let r_opt = stk.pop();
        result = match r_opt { Option::Some(x) => x, Option::None => String::new() };
    }
    if result == "" { result = ast_push_atom(nodes, String::from("?")); }
    result
}

// 解析类型标注 token 序列 -> 类型节点索引串（M-M2b2：路径 / & / &mut / 泛型）。
//   泛型 `<BASE ARG1 ARG2>` 以 @GEN@ 标记收束，支持任意嵌套。
fn parse_type_tokens(ts: Vec<String>, nodes: &mut Vec<AstNode>) -> String {
    let n = ts.len;
    let mut k = 0;
    let mut ref_kind = 0;   // 0 无 / 1 & / 2 &mut
    if n > 0 && ts.get(0) == "bitand" {
        ref_kind = 1; k = 1;
        if k < n {
            if ts.get(k) == "mut" { ref_kind = 2; k = k + 1; }
        }
    }
    let core = parse_type_core(ts, k, nodes);
    let mut result = core;
    if ref_kind == 1 { result = ast_push_list(nodes, String::from("ref"), core); }
    if ref_kind == 2 {
        let m = ast_push_atom(nodes, String::from("mut"));
        result = ast_push_list(nodes, String::from("ref"), ast_cat(m, core));
    }
    result
}

// 解析 let/for 模式 token 序列 -> 模式节点索引串（非递归，M-M2b2 仅扁平元组）。
//   单标识符 -> 原子 NAME；通配 -> 原子 _；元组 -> (tuple-pat ...) 节点。
//   全程语句式 if + 累加变量，不使用 return / if-表达式作为返回值。
fn parse_pattern_tokens(ts: Vec<String>, nodes: &mut Vec<AstNode>) -> String {
    let n = ts.len;
    let mut result = String::from("");
    if n > 0 {
        let first = ts.get(0);
        if first != "(" {
            let t = ts.get(0);
            if t == "(var _)" { result = ast_push_atom(nodes, String::from("_")); }
            else { result = ast_push_atom(nodes, typename_of(t)); }
        } else {
            let mut kids = String::from("");
            let mut k = 1;
            while k < n {
                let t = ts.get(k);
                if t == ")" { break; }
                if t == "," { k = k + 1; continue; }
                if t == "(var _)" { kids = ast_cat(kids, ast_push_atom(nodes, String::from("_"))); }
                else { kids = ast_cat(kids, ast_push_atom(nodes, typename_of(t))); }
                k = k + 1;
            }
            result = ast_push_list(nodes, String::from("tuple-pat"), kids);
        }
    }
    if result == "" { result = ast_push_atom(nodes, String::from("?")); }
    result
}

// 解析单个模式原子 token -> 模式节点索引串（M-M5）：
//   (var x) -> 原子 x / (var _) -> 原子 _ / true|false -> (lit-bool ...) /
//   (int N) -> (lit-int N) / (str S) -> (lit-str S) / .. -> 原子 .. / 其余 -> 原子 ?
fn pat_atom(t: String, nodes: &mut Vec<AstNode>) -> String {
    let mut res = String::from("");
    if t == "(var _)" { res = ast_push_atom(nodes, String::from("_")); }
    else if t == "(var true)" { res = ast_push_leaf(nodes, String::from("lit-bool"), String::from("true")); }
    else if t == "(var false)" { res = ast_push_leaf(nodes, String::from("lit-bool"), String::from("false")); }
    else if t == "(bool true)" { res = ast_push_leaf(nodes, String::from("lit-bool"), String::from("true")); }
    else if t == "(bool false)" { res = ast_push_leaf(nodes, String::from("lit-bool"), String::from("false")); }
    else if t == ".." { res = ast_push_atom(nodes, String::from("..")); }
    else if t.len > 5 && t.substring(0, 5) == "(int " {
        res = ast_push_leaf(nodes, String::from("lit-int"), t.substring(5, t.len - 1));
    }
    else if t.len > 5 && t.substring(0, 5) == "(str " {
        res = ast_push_leaf(nodes, String::from("lit-str"), t.substring(5, t.len - 1));
    }
    else if t.len > 5 && t.substring(0, 5) == "(var " {
        res = ast_push_atom(nodes, t.substring(5, t.len - 1));
    }
    if res == "" { res = ast_push_atom(nodes, String::from("?")); }
    res
}

// 范围模式渲染：含 ..< / ... / <.. -> (range-pat L U li ui) 节点；
//   边界按表达式渲染（对齐 oracle 的 render_expr_canonical：(int N)）。
fn render_range_pat(ts: Vec<String>, nodes: &mut Vec<AstNode>) -> String {
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
    let mut ls = String::from("");
    if lv.len == 1 {
        let lr = parse_expr_arena(lv, 0, nodes);
        ls = lr.id;
    } else if lv.len > 1 {
        ls = pat_atom(lv.get(0), nodes);
    } else {
        ls = ast_push_atom(nodes, String::from("?"));
    }
    let mut us = String::from("");
    if uv.len == 1 {
        let ur = parse_expr_arena(uv, 0, nodes);
        us = ur.id;
    } else if uv.len > 1 {
        us = pat_atom(uv.get(0), nodes);
    } else {
        us = ast_push_atom(nodes, String::from("?"));
    }
    let mut li = 1; let mut ui = 1;
    if rop == "..<" { li = 1; ui = 0; }
    else if rop == "..." { li = 1; ui = 1; }
    else if rop == "<.." { li = 0; ui = 1; }
    let mut kids = ast_cat(ls, us);
    kids = ast_cat(kids, ast_push_atom(nodes, int_to_string(li)));
    kids = ast_cat(kids, ast_push_atom(nodes, int_to_string(ui)));
    ast_push_list(nodes, String::from("range-pat"), kids)
}

// 或模式单侧渲染（单 token -> pat_atom；含范围运算符 -> render_range_pat；其余 -> pat_atom(0)）
fn render_or_side(seg: Vec<String>, nodes: &mut Vec<AstNode>) -> String {
    let n = seg.len;
    let mut res = String::from("");
    if n == 1 {
        res = pat_atom(seg.get(0), nodes);
    } else {
        let mut rk = 0; let mut is_r = 0;
        while rk < n {
            let t = seg.get(rk);
            if t == "..<" || t == "..." || t == "<.." { is_r = 1; break; }
            rk = rk + 1;
        }
        if is_r == 1 { res = render_range_pat(seg, nodes); }
        else { res = pat_atom(seg.get(0), nodes); }
    }
    res
}

// 元组模式渲染：按 , 拆段，每段经 render_or_side（单 token / 范围）；不递归（M-M5a 仅扁平单 token 元素）
fn render_tuple_pat(ts: Vec<String>, nodes: &mut Vec<AstNode>) -> String {
    let n = ts.len;
    let mut kids = String::from("");
    let mut seg: Vec<String> = Vec::new();
    let mut i = 1;
    while i < n - 1 {
        let t = ts.get(i);
        if t == "," {
            if seg.len > 0 { kids = ast_cat(kids, render_or_side(seg, nodes)); }
            seg = Vec::new();
            i = i + 1;
            continue;
        }
        seg.push(t);
        i = i + 1;
    }
    if seg.len > 0 { kids = ast_cat(kids, render_or_side(seg, nodes)); }
    ast_push_list(nodes, String::from("tuple-pat"), kids)
}

// 解析模式 token 序列 -> 模式节点索引串（M-M5，非递归、单向依赖）：
//   或模式：含 bitor -> 按首个 bitor 拆左右（两侧经 render_or_side 渲染）；
//   范围：含 ..< / ... / <.. -> (range-pat L U li ui)，边界按表达式渲染；
//   元组：以 ( 起 ) 收 -> 按 , 拆段，元素为单 token / 范围（不递归）；
//   单 token -> pat_atom。
fn parse_match_pattern(ts: Vec<String>, nodes: &mut Vec<AstNode>) -> String {
    let n = ts.len;
    let mut res = String::from("");
    if n == 0 { res = ast_push_atom(nodes, String::from("?")); }
    else {
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
        let mut kids = String::from("");
        if left.len > 0 { kids = render_or_side(left, nodes); }
        else { kids = ast_push_atom(nodes, String::from("?")); }
        if right.len > 0 { kids = ast_cat(kids, render_or_side(right, nodes)); }
        else { kids = ast_cat(kids, ast_push_atom(nodes, String::from("?"))); }
        res = ast_push_list(nodes, String::from("or"), kids);
    } else {
    // 范围模式：含 ..< / ... / <..
    let mut rk = 0; let mut is_range = 0;
    while rk < n {
        let t = ts.get(rk);
        if t == "..<" || t == "..." || t == "<.." { is_range = 1; break; }
        rk = rk + 1;
    }
    if is_range == 1 { res = render_range_pat(ts, nodes); }
    else if ts.get(0) == "(" && ts.get(n - 1) == ")" { res = render_tuple_pat(ts, nodes); }
    else { res = pat_atom(ts.get(0), nodes); }
    }
    }
    res
}
