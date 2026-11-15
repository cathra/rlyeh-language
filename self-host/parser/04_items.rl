// ============================================================================
// M-M6/M-M8/M-M10 + M-M3b：函数项头 / 泛型参数 / struct·enum 项解析。
//
// M-M3b（SH-P2-7）：所有归约点直接构造 arena 节点（ast_push_* / ast_cat），
// 返回节点索引串；最终由 render_node / render_ast 统一渲染，不再拼规范文本。
// 函数头形如：[pub] NAME [generics] (params ...) [RET]，以「子节点索引序列」
// （空格分隔，可直接交给 ast_cat）返回，供 (fn ...) 节点拼接 body。
// ============================================================================

// 渲染单个函数参数段 -> (param [mut] NAME TYPE) 节点索引串（M-M6）。
//   段形如 [mut] NAME : TYPE（TYPE 可含泛型，>> 已拆为两个 gt）。
fn render_param(seg: Vec<String>, nodes: &mut Vec<AstNode>) -> String {
    let n = seg.len;
    let mut res = String::from("");
    if n > 0 {
        let mut k = 0;
        let mut is_mut = 0;
        if seg.get(0) == "mut" { is_mut = 1; k = 1; }
        let mut name = String::from("?");
        if k < n { name = typename_of(seg.get(k)); }
        k = k + 1;
        let mut typ = String::from("");
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
            typ = parse_type_tokens(fixed, nodes);
        } else {
            typ = ast_push_atom(nodes, String::from("?"));
        }
        let mut kids = String::from("");
        if is_mut == 1 { kids = ast_push_atom(nodes, String::from("mut")); }
        kids = ast_cat(kids, ast_push_atom(nodes, name));
        kids = ast_cat(kids, typ);
        res = ast_push_list(nodes, String::from("param"), kids);
    }
    res
}

// 解析函数项头（M-M6）：`<name>(<params>) [-> <ret>]` -> header 子节点索引序列 +
// 新位置（指向 body '{'）。序列为 [pub] NAME [generics] (params ...) [RET]，
// 由 (fn ...) 归约时 ast_cat 上 body block 节点。
//   独立成函数以缩小 parse_program（避免过多局部变量致编译产物异常）。
fn parse_fn_header(tokens: Vec<String>, ti0: i64, is_pub: i64, nodes: &mut Vec<AstNode>) -> FHdr {
    let n = tokens.len;
    let mut ti = ti0;
    let name = typename_of(tokens.get(ti));
    ti = ti + 1;
    // 泛型参数（M-M10）：`fn f<T, U>(...)`
    let g = parse_generics(tokens, ti, nodes);
    let generics = g.s;
    ti = g.ti;
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
    let mut pkids = String::new();
    let mut seg: Vec<String> = Vec::new();
    let pn = ptoks.len;
    let mut pi = 0;
    while pi < pn {
        let t = ptoks.get(pi);
        if t == "," {
            pkids = ast_cat(pkids, render_param(seg, nodes));
            seg = Vec::new();
            pi = pi + 1;
            continue;
        }
        seg.push(t);
        pi = pi + 1;
    }
    if seg.len > 0 { pkids = ast_cat(pkids, render_param(seg, nodes)); }
    let params = ast_push_list(nodes, String::from("params"), pkids);
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
        ret = parse_type_tokens(fixed, nodes);
    }
    let mut header = String::new();
    if is_pub == 1 { header = ast_push_atom(nodes, String::from("pub")); }
    header = ast_cat(header, ast_push_atom(nodes, name));
    if generics != "" { header = ast_cat(header, generics); }
    header = ast_cat(header, params);
    if ret != "" { header = ast_cat(header, ret); }
    FHdr { h: header, ti: ti }
}

// 类型 token 列表 -> 类型节点索引串（M-M8）：>> 拆两个 gt 后交 parse_type_tokens。
fn type_of_tokens(ts: Vec<String>, nodes: &mut Vec<AstNode>) -> String {
    let mut fixed: Vec<String> = Vec::new();
    let mut fi = 0;
    while fi < ts.len {
        if ts.get(fi) == "shr" { fixed.push(String::from("gt")); fixed.push(String::from("gt")); }
        else { fixed.push(ts.get(fi)); }
        fi = fi + 1;
    }
    parse_type_tokens(fixed, nodes)
}

// 解析泛型参数列表（M-M10）：`<T, U>` -> (generics T U) 节点索引串；
//   非 `<` 起始返回空串 + 原位置。仅处理无 bound 的简单形参（`T: Bound` 超出 M-M10a）。
fn parse_generics(tokens: Vec<String>, ti0: i64, nodes: &mut Vec<AstNode>) -> GRes {
    let n = tokens.len;
    let mut i = ti0;
    let mut has_lt = 0;
    if i < n {
        if tokens.get(i) == "lt" { has_lt = 1; i = i + 1; }
    }
    let mut res = String::from("");
    if has_lt == 1 {
        let mut names = String::new();
        while i < n {
            let t = tokens.get(i);
            if t == "gt" { i = i + 1; break; }
            if t == "," { i = i + 1; continue; }
            names = ast_cat(names, ast_push_atom(nodes, typename_of(t)));
            i = i + 1;
        }
        res = ast_push_list(nodes, String::from("generics"), names);
    } else {
        i = ti0;
    }
    GRes { s: res, ti: i }
}

// 解析结构体声明（M-M8）：`<name> { <field>: <Type>, ... }`
//   -> (struct [pub] NAME [generics] (field NAME TYPE) ...) 节点索引串
fn parse_struct_decl(tokens: Vec<String>, ti0: i64, is_pub: i64, nodes: &mut Vec<AstNode>) -> DRes {
    let n = tokens.len;
    let mut i = ti0;
    let name = typename_of(tokens.get(i));
    i = i + 1;
    // 泛型参数（M-M10）：`struct P<T> { ... }`
    let g = parse_generics(tokens, i, nodes);
    let generics = g.s;
    i = g.ti;
    // 越过 '{'
    if i < n {
        if tokens.get(i) == "{" { i = i + 1; }
    }
    let mut fields = String::new();
    while i < n {
        let t = tokens.get(i);
        if t == "}" { i = i + 1; break; }
        if t == "," { i = i + 1; continue; }
        // 字段名
        let fname = typename_of(t);
        i = i + 1;
        // 越过 ':'
        if i < n {
            if tokens.get(i) == ":" { i = i + 1; }
        }
        // 收集类型 token 直到 ',' 或 '}'
        let mut tt: Vec<String> = Vec::new();
        while i < n {
            let q = tokens.get(i);
            if q == "," { break; }
            if q == "}" { break; }
            tt.push(q);
            i = i + 1;
        }
        let ftype = type_of_tokens(tt, nodes);
        let mut fkids = ast_push_atom(nodes, fname);
        fkids = ast_cat(fkids, ftype);
        fields = ast_cat(fields, ast_push_list(nodes, String::from("field"), fkids));
    }
    let mut kids = String::from("");
    if is_pub == 1 { kids = ast_push_atom(nodes, String::from("pub")); }
    kids = ast_cat(kids, ast_push_atom(nodes, name));
    if generics != "" { kids = ast_cat(kids, generics); }
    kids = ast_cat(kids, fields);
    DRes { s: ast_push_list(nodes, String::from("struct"), kids), ti: i }
}

// 解析枚举声明（M-M8）：`<name> { V1, V2(T, U), V3 { f: T } }`
//   -> (enum [pub] NAME [generics] (variant NAME ...) ...) 节点索引串。
//   变体三形态：单元 / 元组负载（类型节点序列）/ 结构式负载（(field NAME TYPE) 节点）。
fn parse_enum_decl(tokens: Vec<String>, ti0: i64, is_pub: i64, nodes: &mut Vec<AstNode>) -> DRes {
    let n = tokens.len;
    let mut i = ti0;
    let name = typename_of(tokens.get(i));
    i = i + 1;
    // 泛型参数（M-M10）：`enum E<T> { ... }`
    let g = parse_generics(tokens, i, nodes);
    let generics = g.s;
    i = g.ti;
    // 越过 '{'
    if i < n {
        if tokens.get(i) == "{" { i = i + 1; }
    }
    let mut variants = String::new();
    while i < n {
        let t = tokens.get(i);
        if t == "}" { i = i + 1; break; }
        if t == "," { i = i + 1; continue; }
        // 变体名
        let vname = typename_of(t);
        i = i + 1;
        let mut payload = String::new();
        // 元组负载 V(T, U)
        let mut has_paren = 0;
        if i < n {
            if tokens.get(i) == "(" { has_paren = 1; i = i + 1; }
        }
        if has_paren == 1 {
            let mut tt: Vec<String> = Vec::new();
            while i < n {
                let q = tokens.get(i);
                if q == ")" { i = i + 1; break; }
                tt.push(q);
                i = i + 1;
            }
            // 按 ',' 拆各类型
            let mut tseg: Vec<String> = Vec::new();
            let mut ti2 = 0;
            while ti2 < tt.len {
                let q = tt.get(ti2);
                if q == "," {
                    payload = ast_cat(payload, type_of_tokens(tseg, nodes));
                    tseg = Vec::new();
                    ti2 = ti2 + 1;
                    continue;
                }
                tseg.push(q);
                ti2 = ti2 + 1;
            }
            if tseg.len > 0 { payload = ast_cat(payload, type_of_tokens(tseg, nodes)); }
        }
        // 结构式负载 V { f: T }
        let mut has_brace = 0;
        if i < n {
            if tokens.get(i) == "{" { has_brace = 1; i = i + 1; }
        }
        if has_brace == 1 {
            while i < n {
                let q = tokens.get(i);
                if q == "}" { i = i + 1; break; }
                if q == "," { i = i + 1; continue; }
                let fname = typename_of(q);
                i = i + 1;
                if i < n {
                    if tokens.get(i) == ":" { i = i + 1; }
                }
                let mut tt2: Vec<String> = Vec::new();
                while i < n {
                    let z = tokens.get(i);
                    if z == "," { break; }
                    if z == "}" { break; }
                    tt2.push(z);
                    i = i + 1;
                }
                let ftype = type_of_tokens(tt2, nodes);
                let mut fkids = ast_push_atom(nodes, fname);
                fkids = ast_cat(fkids, ftype);
                payload = ast_cat(payload, ast_push_list(nodes, String::from("field"), fkids));
            }
        }
        let mut vkids = ast_push_atom(nodes, vname);
        vkids = ast_cat(vkids, payload);
        variants = ast_cat(variants, ast_push_list(nodes, String::from("variant"), vkids));
    }
    let mut kids = String::from("");
    if is_pub == 1 { kids = ast_push_atom(nodes, String::from("pub")); }
    kids = ast_cat(kids, ast_push_atom(nodes, name));
    if generics != "" { kids = ast_cat(kids, generics); }
    kids = ast_cat(kids, variants);
    DRes { s: ast_push_list(nodes, String::from("enum"), kids), ti: i }
}
