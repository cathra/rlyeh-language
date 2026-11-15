// M-M3c（SH-P2-7）Rlyeh 版最小声明式宏展开器：macro_rules 子集 → 规范化 token 文本。
//
// 与 Rust oracle（`crates/rlyeh-driver/src/macro_oracle.rs` 的
// `emit_macro_expand` / `emit_macro_expand_rules`，底层为 `crates/rlyeh-macro`）
// 逐行对拍：输出格式 = 每行一个规范 token（与 `self-host/lexer.rl` 的 `lex` 同格式），
// 由 `crates/rlyeh-driver/tests/self_host_macro.rs` 差分验证。
//
// 本文件**不自带 lex**：harness 会按 `self-host/lexer.rl` + `self-host/macro.rl`
// 顺序拼接后交给 driver 编译（Rlyeh 无 include），故可直接调用 lexer.rl 的
// `lex` / `i64_to_string`。
//
// 实现范围（与 rlyeh-macro 的 MVP 语义逐项对齐）：
//   - 单规则 / 多规则（首个 matcher 完全消费调用 token 的规则胜出）
//   - 字面 token 精确匹配、嵌套定界组（`(...)` / `[...]` / `{...}`）
//   - 元变量 `$x:ident` / `$x:ty` / `$x:tt` / `$x:expr`
//     （ty = 单个非定界 token；tt = 单 token 或整组；expr = token 级优先级爬升捕获）
//   - 重复 `$($inner),sep op`（op = `*` / `+` / `?`；分隔符可选）
//   - transcriber 中 `$x` 取最近一次捕获，重复片段按捕获数迭代
//
// 不支持（与 Rust oracle 同源的 MVP 限制）：hygiene、前向引用、`$crate`、
// 嵌套重复、规则片段 `$x:pat`/`$x:stmt` 等其它元变量种类。
//
// Rlyeh 约束遵守（见 docs/tasks/leaf/sh-p2-7-driver.md「Rlyeh 语言踩坑点」）：
//   * 返回 String 的函数禁用 return 提前返回（用累加变量 + 尾部表达式）；
//   * && / || 不短路（边界判断一律嵌套 if）；
//   * 整数索引/状态统一以 String 存放（避免 Vec<i64>.get 返回 ptr）；
//   * 被调函数定义在调用方之前；
//   * 函数可递归（本文件 parse / match / expand 均自递归，已实测可行）。

/// 扁平 arena 节点（matcher / transcriber 共用）。
/// kind: "L" 字面 token / "M" 元变量 / "D" 定界组 / "R" 重复 / "S" 序列根
/// 字段：a = 字面 token | 元变量名 | 开定界符 | 重复分隔符；
///       b = 元变量种类 | 闭定界符 | 重复操作符；
///       kids = 空格分隔的子节点索引串。
struct MNode {
    kind: String,
    a: String,
    b: String,
    kids: String,
}

/// 元变量绑定表 + 作用域快照栈（支持 delim / repeat 迭代的子作用域提交/回滚）。
///
/// names 为绑定名（插入序）；每个名字的捕获区间是 lo[i]..hi[i]（索引进 cs/ce）；
/// 每次捕获记录为调用 token 流的区间 [cs[k], ce[k])，故展开时可直接取 token 行。
/// 作用域快照：sn_names/sn_cs = 进入作用域时的 names / captures 数量，
/// sc_start = 该作用域已保存 hi 快照在 sc_saved 中的起点。
struct MBind {
    names: Vec<String>,
    lo: Vec<String>,
    hi: Vec<String>,
    cs: Vec<String>,
    ce: Vec<String>,
    sn_names: Vec<String>,
    sn_cs: Vec<String>,
    sc_saved: Vec<String>,
    sc_start: Vec<String>,
}

// ===== 基础字符串/arena 辅助 =====

/// 向空格分隔的子节点索引串追加一个索引（空串时直接返回该索引）。
fn m_append_kid(k: String, id: String) -> String {
    let mut r = k;
    if r == "" {
        r = id;
    } else {
        r = r + " " + id;
    }
    r
}

/// 拆分空格分隔的索引串；空串返回空 Vec。
fn m_split_ids(kids: String) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    if kids != "" {
        ids = kids.split(" ");
    }
    ids
}

/// 向 arena 追加节点，返回其索引。
fn m_push_node(nodes: &mut Vec<MNode>, kind: String, a: String, b: String, kids: String) -> i64 {
    nodes.push(MNode { kind: kind, a: a, b: b, kids: kids });
    nodes.len() - 1
}

/// token 是否为开定界符。
fn m_is_open(t: String) -> i64 {
    if t == "(" { return 1; }
    if t == "[" { return 1; }
    if t == "{" { return 1; }
    0
}

/// token 是否为闭定界符。
fn m_is_close(t: String) -> i64 {
    if t == ")" { return 1; }
    if t == "]" { return 1; }
    if t == "}" { return 1; }
    0
}

/// 开定界符对应的闭定界符（非开定界符返回空串）。
fn m_close_for(open: String) -> String {
    let mut r = String::from("");
    if open == "(" { r = String::from(")"); }
    else if open == "[" { r = String::from("]"); }
    else if open == "{" { r = String::from("}"); }
    r
}

/// 从 tokens[pos]（须为开定界符）找到配对闭合，返回「闭定界符后一位」下标；失败 -1。
fn m_find_group(tokens: Vec<String>, pos: i64) -> i64 {
    let n = tokens.len;
    if pos >= n { return 0 - 1; }
    let open = tokens.get(pos);
    if m_is_open(open) == 0 { return 0 - 1; }
    let mut depth = 1;
    let mut i = pos + 1;
    while i < n {
        let t = tokens.get(i);
        if m_is_open(t) == 1 {
            depth = depth + 1;
        } else if m_is_close(t) == 1 {
            depth = depth - 1;
            if depth == 0 { return i + 1; }
        }
        i = i + 1;
    }
    0 - 1
}

/// 取 "IDENT name" 规范 token 行的 name 部分（非 ident 返回空串）。
fn m_strip_ident(t: String) -> String {
    let mut r = String::from("");
    let n = t.len;
    if n > 6 {
        if t.substring(0, 6) == "IDENT " {
            r = t.substring(6, n);
        }
    }
    r
}

// ===== matcher / transcriber token 解析（递归下降，arena 构造） =====
//
// mode = "matcher"：`$name:kind` 形式（kind 为下一个 IDENT token）；
// mode = "trans"  ：`$name` 形式。
// 返回 kids 索引串；根节点由 m_parse 追加。

fn m_parse_range(tokens: Vec<String>, pos0: i64, end: i64, mode: String, nodes: &mut Vec<MNode>) -> String {
    let mut kids = String::from("");
    let mut pos = pos0;
    while pos < end {
        let tok = tokens.get(pos);
        if tok == "$" {
            let nxt = tokens.get(pos + 1);
            if nxt == "(" {
                // 重复：$( inner ) [sep] op
                let g = m_find_group(tokens, pos + 1);
                let inner_end = g - 1;
                let inner = m_parse_range(tokens, pos + 2, inner_end, mode, nodes);
                let mut sep = String::from("");
                let mut op = String::from("");
                let mut q = g;
                let t = tokens.get(q);
                if t == "*" || t == "+" || t == "?" {
                    op = t;
                    q = q + 1;
                } else {
                    sep = t;
                    op = tokens.get(q + 1);
                    q = q + 2;
                }
                let id = m_push_node(nodes, String::from("R"), sep, op, inner);
                kids = m_append_kid(kids, int_to_string(id));
                pos = q;
            } else if mode == "matcher" {
                // $name:kind
                let name = m_strip_ident(nxt);
                let kind = tokens.get(pos + 3);
                let id = m_push_node(nodes, String::from("M"), name, kind, String::from(""));
                kids = m_append_kid(kids, int_to_string(id));
                pos = pos + 4;
            } else {
                // $name
                let name = m_strip_ident(nxt);
                let id = m_push_node(nodes, String::from("M"), name, String::from(""), String::from(""));
                kids = m_append_kid(kids, int_to_string(id));
                pos = pos + 2;
            }
        } else if m_is_open(tok) == 1 {
            let g = m_find_group(tokens, pos);
            let inner = m_parse_range(tokens, pos + 1, g - 1, mode, nodes);
            let close = m_close_for(tok);
            let id = m_push_node(nodes, String::from("D"), tok, close, inner);
            kids = m_append_kid(kids, int_to_string(id));
            pos = g;
        } else {
            let id = m_push_node(nodes, String::from("L"), tok, String::from(""), String::from(""));
            kids = m_append_kid(kids, int_to_string(id));
            pos = pos + 1;
        }
    }
    kids
}

/// 解析一段 canonical token 流为 arena；根节点（kind="S"）为最后一个节点。
fn m_parse(tokens: Vec<String>, mode: String) -> Vec<MNode> {
    let mut nodes: Vec<MNode> = Vec::new();
    let end = tokens.len;
    let kids = m_parse_range(tokens, 0, end, mode, &mut nodes);
    nodes.push(MNode { kind: String::from("S"), a: String::from(""), b: String::from(""), kids: kids });
    nodes
}

// ===== 元变量绑定 =====

fn mb_new() -> MBind {
    MBind {
        names: Vec::new(),
        lo: Vec::new(),
        hi: Vec::new(),
        cs: Vec::new(),
        ce: Vec::new(),
        sn_names: Vec::new(),
        sn_cs: Vec::new(),
        sc_saved: Vec::new(),
        sc_start: Vec::new(),
    }
}

/// 查绑定名下标；未找到 -1。
fn mb_find_name(b: &MBind, name: String) -> i64 {
    let mut found = 0 - 1;
    let mut i = 0;
    while i < b.names.len {
        if b.names.get(i) == name {
            found = i;
            i = b.names.len;
        } else {
            i = i + 1;
        }
    }
    found
}

/// 向绑定名追加一次捕获（token 区间 [s, e)）。
fn mb_push_capture(b: &mut MBind, name: String, s: i64, e: i64) {
    let idx = mb_find_name(b, name);
    let cs_len = b.cs.len;
    b.cs.push(int_to_string(s));
    b.ce.push(int_to_string(e));
    if idx < 0 {
        b.names.push(name);
        b.lo.push(int_to_string(cs_len));
        b.hi.push(int_to_string(cs_len + 1));
    } else {
        b.hi.set(idx, int_to_string(cs_len + 1));
    }
}

/// 进入子作用域（记录快照）。
fn mb_push_scope(b: &mut MBind) {
    let nl = b.names.len;
    let cl = b.cs.len;
    let sl = b.sc_saved.len;
    b.sn_names.push(int_to_string(nl));
    b.sn_cs.push(int_to_string(cl));
    b.sc_start.push(int_to_string(sl));
    let mut i = 0;
    while i < nl {
        let h = b.hi.get(i);
        b.sc_saved.push(h);
        i = i + 1;
    }
}

/// 提交子作用域（捕获保留，丢弃快照）。
fn mb_commit(b: &mut MBind) {
    let ti = b.sn_names.len - 1;
    b.sn_names.remove(ti);
    b.sn_cs.remove(ti);
    let st = string_to_int(b.sc_start.remove(ti));
    while b.sc_saved.len > st {
        let lt = b.sc_saved.len - 1;
        b.sc_saved.remove(lt);
    }
}

/// 回滚子作用域（恢复到进入时状态）。
fn mb_rollback(b: &mut MBind) {
    let ti = b.sn_names.len - 1;
    let names_len = string_to_int(b.sn_names.remove(ti));
    let cs_len = string_to_int(b.sn_cs.remove(ti));
    let st = string_to_int(b.sc_start.remove(ti));
    let mut i = 0;
    while i < names_len {
        let h = b.sc_saved.get(st + i);
        b.hi.set(i, h);
        i = i + 1;
    }
    while b.sc_saved.len > st {
        let lt = b.sc_saved.len - 1;
        b.sc_saved.remove(lt);
    }
    while b.names.len > names_len {
        let lt = b.names.len - 1;
        b.names.remove(lt);
        b.lo.remove(lt);
        b.hi.remove(lt);
    }
    while b.cs.len > cs_len {
        let lt = b.cs.len - 1;
        b.cs.remove(lt);
        b.ce.remove(lt);
    }
}

/// 绑定名的捕获次数（未绑定为 0）。
fn mb_cap_count(b: &MBind, name: String) -> i64 {
    let idx = mb_find_name(b, name);
    let mut cnt = 0;
    if idx >= 0 {
        let lo = string_to_int(b.lo.get(idx));
        let hi = string_to_int(b.hi.get(idx));
        cnt = hi - lo;
    }
    cnt
}

/// 绑定名第 r 次捕获在 cs/ce 中的下标（越界 -1）。
fn mb_cap_at(b: &MBind, name: String, r: i64) -> i64 {
    let idx = mb_find_name(b, name);
    let mut c = 0 - 1;
    if idx >= 0 {
        let lo = string_to_int(b.lo.get(idx));
        let hi = string_to_int(b.hi.get(idx));
        if lo + r < hi {
            c = lo + r;
        }
    }
    c
}

/// 把绑定名的最近一次捕获 token 追加到输出。
fn mb_emit_last(b: &MBind, name: String, tokens: Vec<String>, sink: &mut Vec<String>) {
    let cnt = mb_cap_count(b, name);
    if cnt == 0 { panic(String::from("macro: unbound metavariable")); }
    let c = mb_cap_at(b, name, cnt - 1);
    let s = string_to_int(b.cs.get(c));
    let e = string_to_int(b.ce.get(c));
    let mut j = s;
    while j < e {
        sink.push(tokens.get(j));
        j = j + 1;
    }
}

// ===== token 分类（与 rlyeh-macro 的匹配语义逐项对齐） =====

fn m_is_ident(t: String) -> i64 {
    let mut r = 0;
    if t.starts_with("IDENT ") { r = 1; }
    r
}

/// 表达式原子 token（对应 Rust `consume_operand` 的原子集合）。
fn m_is_atom_token(t: String) -> i64 {
    let mut r = 0;
    if t == "true" { r = 1; }
    else if t == "false" { r = 1; }
    else if t.starts_with("IDENT ") { r = 1; }
    else if t.starts_with("INT ") { r = 1; }
    else if t.starts_with("FLOAT ") { r = 1; }
    else if t.starts_with("STR ") { r = 1; }
    else if t.starts_with("CHAR ") { r = 1; }
    else if t.starts_with("BOOL ") { r = 1; }
    else if t.starts_with("TIME ") { r = 1; }
    r
}

/// 前缀一元运算符（与 Rust `is_prefix_op` 一致）。
fn m_is_prefix_op(t: String) -> i64 {
    let mut r = 0;
    if t == "-" { r = 1; }
    else if t == "+" { r = 1; }
    else if t == "!" { r = 1; }
    else if t == "not" { r = 1; }
    else if t == "&" { r = 1; }
    else if t == "*" { r = 1; }
    r
}

/// 二元中缀运算符（与 Rust `is_binary_op` 一致）。
fn m_is_binary_op(t: String) -> i64 {
    let mut r = 0;
    if t == "||" { r = 1; }
    else if t == "&&" { r = 1; }
    else if t == "|" { r = 1; }
    else if t == "^" { r = 1; }
    else if t == "&" { r = 1; }
    else if t == "==" { r = 1; }
    else if t == "!=" { r = 1; }
    else if t == "<" { r = 1; }
    else if t == "<=" { r = 1; }
    else if t == ">" { r = 1; }
    else if t == ">=" { r = 1; }
    else if t == "<<" { r = 1; }
    else if t == ">>" { r = 1; }
    else if t == "+" { r = 1; }
    else if t == "-" { r = 1; }
    else if t == "*" { r = 1; }
    else if t == "/" { r = 1; }
    else if t == "%" { r = 1; }
    else if t == "in" { r = 1; }
    else if t == "NOTIN" { r = 1; }
    else if t == "RANGE" { r = 1; }
    else if t == "DOTDOTLT" { r = 1; }
    else if t == "DOTDOTDOT" { r = 1; }
    else if t == "LTDOTDOT" { r = 1; }
    r
}

// ===== 表达式 token 级优先级爬升（对应 Rust consume_expr） =====

/// 消费一个类型 token 序列（`i64` / `Vec<i64>` / `mod::Type`），返回新位置；失败 -1。
fn m_consume_ty(tokens: Vec<String>, pos0: i64, limit: i64) -> i64 {
    let mut pos = pos0;
    if pos >= limit { return 0 - 1; }
    if m_is_ident(tokens.get(pos)) == 0 { return 0 - 1; }
    pos = pos + 1;
    // 路径段 ::（与 Rust 一致：每段须为 ident）
    let mut go = 1;
    while go == 1 {
        if pos + 1 < limit {
            if tokens.get(pos) == ":" {
                if tokens.get(pos + 1) == ":" {
                    if pos + 2 < limit {
                        if m_is_ident(tokens.get(pos + 2)) == 1 {
                            pos = pos + 3;
                        } else {
                            return 0 - 1;
                        }
                    } else {
                        return 0 - 1;
                    }
                } else {
                    go = 0;
                }
            } else {
                go = 0;
            }
        } else {
            go = 0;
        }
    }
    // 泛型参数 <...>（尖括号深度配对）
    if pos < limit {
        if tokens.get(pos) == "<" {
            let mut depth = 1;
            let mut q = pos + 1;
            let mut stop = 0;
            while stop == 0 {
                if q >= limit {
                    stop = 1;
                } else {
                    let t = tokens.get(q);
                    if t == "<" {
                        depth = depth + 1;
                    } else if t == ">" {
                        depth = depth - 1;
                    }
                    q = q + 1;
                    if depth == 0 { stop = 1; }
                }
            }
            pos = q;
        }
    }
    pos
}

/// 消费一个操作数（前缀一元 * 原子 * 后缀），返回新位置；失败 -1。
fn m_consume_operand(tokens: Vec<String>, pos0: i64, limit: i64) -> i64 {
    let mut pos = pos0;
    let mut go = 1;
    while go == 1 {
        if pos < limit {
            if m_is_prefix_op(tokens.get(pos)) == 1 {
                pos = pos + 1;
            } else {
                go = 0;
            }
        } else {
            go = 0;
        }
    }
    if pos >= limit { return 0 - 1; }
    let t = tokens.get(pos);
    let mut atom = 0;
    if m_is_open(t) == 1 { atom = 1; }
    else if m_is_atom_token(t) == 1 { atom = 1; }
    if atom == 0 { return 0 - 1; }
    if m_is_open(t) == 1 {
        let g = m_find_group(tokens, pos);
        if g < 0 { return 0 - 1; }
        if g > limit { return 0 - 1; }
        pos = g;
    } else {
        pos = pos + 1;
    }
    // 后缀：调用/索引/成员/?/宏调用 !/as 转换
    let mut sfx = 1;
    while sfx == 1 {
        if pos >= limit {
            sfx = 0;
        } else {
            let s = tokens.get(pos);
            if s == "(" {
                let g = m_find_group(tokens, pos);
                if g < 0 { return 0 - 1; }
                if g > limit { return 0 - 1; }
                pos = g;
            } else if s == "[" {
                let g = m_find_group(tokens, pos);
                if g < 0 { return 0 - 1; }
                if g > limit { return 0 - 1; }
                pos = g;
            } else if s == "." {
                pos = pos + 1;
                if pos >= limit { return 0 - 1; }
                if m_is_ident(tokens.get(pos)) == 0 { return 0 - 1; }
                pos = pos + 1;
            } else if s == "?" {
                pos = pos + 1;
            } else if s == "!" {
                if pos + 1 < limit {
                    if m_is_open(tokens.get(pos + 1)) == 1 {
                        pos = pos + 1;
                        let g = m_find_group(tokens, pos);
                        if g < 0 { return 0 - 1; }
                        if g > limit { return 0 - 1; }
                        pos = g;
                    } else {
                        sfx = 0;
                    }
                } else {
                    sfx = 0;
                }
            } else if s == "as" {
                pos = pos + 1;
                let q = m_consume_ty(tokens, pos, limit);
                if q < 0 { return 0 - 1; }
                pos = q;
            } else {
                sfx = 0;
            }
        }
    }
    pos
}

/// 消费一个完整表达式（token 级优先级爬升），返回新位置；失败 -1。
fn m_consume_expr(tokens: Vec<String>, pos0: i64, limit: i64) -> i64 {
    let mut pos = m_consume_operand(tokens, pos0, limit);
    if pos < 0 { return 0 - 1; }
    let mut go = 1;
    while go == 1 {
        if pos < limit {
            if m_is_binary_op(tokens.get(pos)) == 1 {
                pos = pos + 1;
                let q = m_consume_operand(tokens, pos, limit);
                if q < 0 { return 0 - 1; }
                pos = q;
            } else {
                go = 0;
            }
        } else {
            go = 0;
        }
    }
    pos
}

// ===== matcher 匹配（返回消费的 token 数；失败 -1） =====

fn m_match_seq(nodes: &Vec<MNode>, kids: String, tokens: Vec<String>, start: i64, limit: i64, bind: &mut MBind) -> i64 {
    let ids = m_split_ids(kids);
    let mut cur = start;
    let mut i = 0;
    while i < ids.len {
        let id = string_to_int(ids.get(i));
        let nd = nodes.get(id);
        if nd.kind == "L" {
            if cur < limit {
                if tokens.get(cur) == nd.a {
                    cur = cur + 1;
                } else {
                    return 0 - 1;
                }
            } else {
                return 0 - 1;
            }
        } else if nd.kind == "M" {
            let name = nd.a;
            let kind = nd.b;
            if kind == "IDENT ident" {
                if cur < limit {
                    if m_is_ident(tokens.get(cur)) == 1 {
                        mb_push_capture(bind, name, cur, cur + 1);
                        cur = cur + 1;
                    } else {
                        return 0 - 1;
                    }
                } else {
                    return 0 - 1;
                }
            } else if kind == "IDENT ty" {
                if cur < limit {
                    let t = tokens.get(cur);
                    if m_is_open(t) == 0 {
                        if m_is_close(t) == 0 {
                            mb_push_capture(bind, name, cur, cur + 1);
                            cur = cur + 1;
                        } else {
                            return 0 - 1;
                        }
                    } else {
                        return 0 - 1;
                    }
                } else {
                    return 0 - 1;
                }
            } else if kind == "IDENT tt" {
                if cur < limit {
                    let t = tokens.get(cur);
                    if m_is_open(t) == 1 {
                        let g = m_find_group(tokens, cur);
                        if g < 0 { return 0 - 1; }
                        if g > limit { return 0 - 1; }
                        mb_push_capture(bind, name, cur, g);
                        cur = g;
                    } else {
                        mb_push_capture(bind, name, cur, cur + 1);
                        cur = cur + 1;
                    }
                } else {
                    return 0 - 1;
                }
            } else if kind == "IDENT expr" {
                let q = m_consume_expr(tokens, cur, limit);
                if q < 0 { return 0 - 1; }
                mb_push_capture(bind, name, cur, q);
                cur = q;
            } else {
                return 0 - 1;
            }
        } else if nd.kind == "D" {
            if cur < limit {
                if tokens.get(cur) == nd.a {
                    let g = m_find_group(tokens, cur);
                    if g < 0 { return 0 - 1; }
                    if g > limit { return 0 - 1; }
                    let is = cur + 1;
                    let ie = g - 1;
                    mb_push_scope(bind);
                    let c = m_match_seq(nodes, nd.kids, tokens, is, ie, bind);
                    if c == ie - is {
                        mb_commit(bind);
                        cur = g;
                    } else {
                        mb_rollback(bind);
                        return 0 - 1;
                    }
                } else {
                    return 0 - 1;
                }
            } else {
                return 0 - 1;
            }
        } else if nd.kind == "R" {
            let sep = nd.a;
            let op = nd.b;
            let mut iters = 0;
            let mut go = 1;
            while go == 1 {
                mb_push_scope(bind);
                let c = m_match_seq(nodes, nd.kids, tokens, cur, limit, bind);
                if c > 0 {
                    mb_commit(bind);
                    iters = iters + 1;
                    cur = cur + c;
                    // 与 Rust 版一致：每轮成功后若下一个 token 是分隔符则消费（即使随后失败）
                    if sep != "" {
                        if cur < limit {
                            if tokens.get(cur) == sep {
                                cur = cur + 1;
                            }
                        }
                    }
                } else {
                    mb_rollback(bind);
                    go = 0;
                }
            }
            if op == "+" {
                if iters == 0 { return 0 - 1; }
            }
        }
        i = i + 1;
    }
    cur - start
}

// ===== transcriber 展开 =====

/// 收集片段中出现的元变量名（可重复；展开/投影内部自行去重）。
fn m_collect_meta(nodes: &Vec<MNode>, kids: String, names: &mut Vec<String>) {
    let ids = m_split_ids(kids);
    let mut i = 0;
    while i < ids.len {
        let id = string_to_int(ids.get(i));
        let nd = nodes.get(id);
        if nd.kind == "M" {
            names.push(nd.a);
        } else if nd.kind == "D" || nd.kind == "R" {
            m_collect_meta(nodes, nd.kids, names);
        }
        i = i + 1;
    }
}

/// 为第 r 次重复迭代投影绑定：片段内元变量取第 r 次捕获，其余不携带。
fn mb_project(bind: &MBind, nodes: &Vec<MNode>, kids: String, r: i64) -> MBind {
    let mut sub = mb_new();
    let mut names: Vec<String> = Vec::new();
    m_collect_meta(nodes, kids, &mut names);
    let mut i = 0;
    while i < names.len {
        let name = names.get(i);
        let mut found = 0 - 1;
        { found = mb_find_name(&sub, name); }
        if found < 0 {
            let mut c = mb_cap_at(bind, name, r);
            if c < 0 {
                // 与 Rust project_bindings 一致：该轮无捕获时回退到最近一次捕获
                let cnt = mb_cap_count(bind, name);
                if cnt > 0 { c = mb_cap_at(bind, name, cnt - 1); }
            }
            if c >= 0 {
                let s = string_to_int(bind.cs.get(c));
                let e = string_to_int(bind.ce.get(c));
                mb_push_capture(&mut sub, name, s, e);
            }
        }
        i = i + 1;
    }
    sub
}

/// 展开 transcriber 片段，把 token 行追加进 sink。
fn m_expand(nodes: &Vec<MNode>, kids: String, tokens: Vec<String>, bind: &MBind, sink: &mut Vec<String>) {
    let ids = m_split_ids(kids);
    let mut i = 0;
    while i < ids.len {
        let id = string_to_int(ids.get(i));
        let nd = nodes.get(id);
        if nd.kind == "L" {
            sink.push(nd.a);
        } else if nd.kind == "M" {
            mb_emit_last(bind, nd.a, tokens, sink);
        } else if nd.kind == "D" {
            sink.push(nd.a);
            m_expand(nodes, nd.kids, tokens, bind, sink);
            sink.push(nd.b);
        } else if nd.kind == "R" {
            let mut names: Vec<String> = Vec::new();
            m_collect_meta(nodes, nd.kids, &mut names);
            let mut iters = 0;
            let mut has_meta = 0;
            let mut j = 0;
            while j < names.len {
                has_meta = 1;
                let c = mb_cap_count(bind, names.get(j));
                if c > iters { iters = c; }
                j = j + 1;
            }
            if has_meta == 0 {
                if nd.b == "+" {
                    iters = 1;
                } else {
                    iters = 0;
                }
            }
            if nd.b == "+" {
                if iters == 0 { panic(String::from("macro: repeat requires at least one capture")); }
            }
            let mut r = 0;
            while r < iters {
                if r > 0 {
                    if nd.a != "" {
                        sink.push(nd.a);
                    }
                }
                let sub = mb_project(bind, nodes, nd.kids, r);
                m_expand(nodes, nd.kids, tokens, &sub, sink);
                r = r + 1;
            }
        }
        i = i + 1;
    }
}

// ===== 顶层入口 =====

/// 多规则宏展开：`matcher_srcs[i] => trans_srcs[i]`，取首个完全匹配调用 token 的规则。
///
/// 返回展开后的规范 token 文本（每行一个 token，无尾随换行）。
fn expand_macro_rules(matcher_srcs: Vec<String>, trans_srcs: Vec<String>, inv_src: String) -> String {
    let inv = lex(inv_src);
    let mut chosen = 0 - 1;
    let mut i = 0;
    while i < matcher_srcs.len {
        let mtoks = lex(matcher_srcs.get(i));
        let mnodes = m_parse(mtoks, String::from("matcher"));
        let mroot = mnodes.len() - 1;
        let mkids = mnodes.get(mroot).kids;
        let mut bind = mb_new();
        let consumed = m_match_seq(&mnodes, mkids, inv, 0, inv.len, &mut bind);
        if consumed == inv.len {
            if chosen < 0 { chosen = i; }
        }
        i = i + 1;
    }
    if chosen < 0 {
        panic(String::from("macro: no rule matched"));
    }
    // 选中规则：重新解析/匹配以取得绑定，再展开对应 transcriber。
    let mtoks2 = lex(matcher_srcs.get(chosen));
    let mnodes2 = m_parse(mtoks2, String::from("matcher"));
    let mroot2 = mnodes2.len() - 1;
    let mkids2 = mnodes2.get(mroot2).kids;
    let mut bind2 = mb_new();
    { m_match_seq(&mnodes2, mkids2, inv, 0, inv.len, &mut bind2); }
    let ttoks = lex(trans_srcs.get(chosen));
    let tnodes = m_parse(ttoks, String::from("trans"));
    let troot = tnodes.len() - 1;
    let tkids = tnodes.get(troot).kids;
    let mut sink: Vec<String> = Vec::new();
    m_expand(&tnodes, tkids, inv, &bind2, &mut sink);
    let mut acc = String::from("");
    let mut k = 0;
    while k < sink.len {
        if k > 0 {
            acc = acc + String::from("\n");
        }
        acc = acc + sink.get(k);
        k = k + 1;
    }
    acc
}

/// 单规则宏展开：`matcher_src => trans_src`。
///
/// 参数均为**组内容**（已剥掉外层定界符）的源码：
/// matcher_src = matcher `(...)` 内容，trans_src = transcriber `{...}` 内容，
/// inv_src = 宏调用定界组内的 token 源码。返回规范化 token 文本（每行一个）。
fn expand_macro(matcher_src: String, trans_src: String, inv_src: String) -> String {
    let mut ms: Vec<String> = Vec::new();
    ms.push(matcher_src);
    let mut ts: Vec<String> = Vec::new();
    ts.push(trans_src);
    expand_macro_rules(ms, ts, inv_src)
}
