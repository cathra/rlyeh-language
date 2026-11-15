// M-M3a（SH-P2-7）Rlyeh 版类型化 AST 模型：扁平 arena + 前序 head 序列 / 回渲染。
//
// 输入为 Rust oracle `emit_ast_canonical` / `emit_ast_canonical_expr` 产出的
// 规范 S-表达式文本（括号是唯一结构定界符）。本模型不直接拼串：先把文本解析为
// 显式节点 arena，再由 arena 派生「前序 head 序列」（与 Rust oracle 节点级对拍）
// 与「规范文本回渲染」（逐字节 roundtrip 对拍）。
//
// 节点形态（扁平 arena，规避 Rlyeh 递归/共享 Vec 语义限制）：
//   struct AstNode { head: String, kids: String, kind: String }
//     head : 列表节点为 S-表达式 head（program/let/bin/int/...）；原子节点为原子原文
//     kids : 空格分隔的子节点索引（i64 十进制编码为 String，规避 Vec<i64>.get 返回 ptr）
//     kind : "L" 列表节点 / "A" 原子节点（原子不是 AST 节点，仅用于回渲染）
//
// Rlyeh 约束遵守（见 docs/tasks/leaf/sh-p2-7-driver.md「Rlyeh 语言踩坑点」）：
//   * 返回 String 的函数禁用 return 提前返回 -> 累加变量 + 尾部表达式；
//   * 不把 if/else 当值使用 -> 语句式 if 赋值；
//   * && / || 不短路 -> 边界判断一律嵌套 if；
//   * 整数态一律 Vec<String> 编码，不出现 Vec<i64>；
//   * 被调函数定义在调用方之前（byte_str/int_str/append_kid 均先于 parse_ast）；
//   * `recv`/`out`/`send` 等保留字避让（本文件用 acc/sink 类命名）；
//   * 全程迭代（显式栈 + remove 取栈顶），不使用递归；
//   * Vec<T>.pop() 返回 Option<T>：改用 len+get+remove 原地操作规避 Option 解包；
//   * Vec 的原地修改（set/remove/push）直接作用于局部变量，不经「按值传参 + 返回」
//     的辅助函数（避免多返回值/共享 Vec 语义风险）。

/// 扁平 arena 节点。head/kids 语义见文件头注释。
struct AstNode {
    head: String,
    kids: String,
    kind: String,
}

/// 类型化 AST 根对象：节点 arena + 根节点索引（-1 表示解析失败/空输入）。
struct Ast {
    nodes: Vec<AstNode>,
    root: i64,
}

/// 单字节 -> String（字符拼接用；Rlyeh String 无 push(char)）。
fn byte_str(b: i64) -> String {
    let s = String::with_capacity(1);
    s.push_byte(b);
    s
}

/// i64 -> 十进制 String（非递归；0 单独处理）。
fn int_str(n: i64) -> String {
    let mut acc = String::from("");
    let mut v = n;
    if v == 0 {
        acc = String::from("0");
    }
    while v > 0 {
        let d = v % 10;
        acc = byte_str(48 + d) + acc;
        v = v / 10;
    }
    acc
}

/// 向空格分隔的 kids 串追加一个子节点索引。
fn append_kid(k: String, id: String) -> String {
    let mut r = k;
    if r == "" {
        r = id;
    } else {
        r = r + " " + id;
    }
    r
}

/// 原子节点（kind = "A"）：head 保存原子原文，无子节点。
fn atom_node(text: String) -> AstNode {
    AstNode { head: text, kids: String::from(""), kind: String::from("A") }
}

/// 列表节点（kind = "L"）。
fn list_node(head: String, kids: String) -> AstNode {
    AstNode { head: head, kids: kids, kind: String::from("L") }
}

/// 把规范 S-表达式文本解析为扁平 arena AST。
///
/// 扫描规则：`(` 压入新帧；`)` 弹出帧并落成一个列表节点（post-order 入 arena，
/// 故子节点索引恒小于父节点索引）；其余文本按「到下一个括号为止」的整段取为
/// segment——首个 segment 的首个空白分隔词是 head，其余部分（可能含空格，如
/// `(str hello world)` 的内容）作为一个原子子节点。这样原子以括号为唯一定界，
/// 不会因按空白切分而丢失 `(str ...)` 内容。
fn parse_ast(text: String) -> Ast {
    let mut nodes: Vec<AstNode> = Vec::new();
    let mut st_head: Vec<String> = Vec::new();
    let mut st_kids: Vec<String> = Vec::new();
    let mut st_started: Vec<String> = Vec::new();
    let mut root = 0 - 1;
    let n = text.len;
    let mut i = 0;
    while i < n {
        let c = text.get(i);
        if c == 40 {
            // '('：新列表帧
            st_head.push(String::from(""));
            st_kids.push(String::from(""));
            st_started.push(String::from("0"));
            i = i + 1;
        } else if c == 41 {
            // ')'：收束当前帧为列表节点
            let htop = st_head.len() - 1;
            let h = st_head.remove(htop);
            let ktop = st_kids.len() - 1;
            let k = st_kids.remove(ktop);
            let stop = st_started.len() - 1;
            st_started.remove(stop);
            let nid = nodes.len();
            nodes.push(list_node(h, k));
            if st_kids.len() > 0 {
                let ptop = st_kids.len() - 1;
                let pk = st_kids.get(ptop);
                st_kids.set(ptop, append_kid(pk, int_str(nid)));
            } else {
                root = nid;
            }
            i = i + 1;
        } else {
            // 原子/head 段：扫到下一个 '(' 或 ')' 为止；首个词是 head，其后为原子子节点。
            // 非 str 原子按 trim 归一；str/lit-str 载荷原样保留（可含首尾空格/空串）。
            let start = i;
            while i < n {
                let d = text.get(i);
                if d == 40 {
                    break;
                }
                if d == 41 {
                    break;
                }
                i = i + 1;
            }
            let raw = String::from(text.substring(start, i));
            let raw0 = String::from(raw.trim_start());
            let seg = String::from(raw0.trim());
            if seg != "" {
                if st_started.len() > 0 {
                    let top = st_started.len() - 1;
                    let st = st_started.get(top);
                    if st == "0" {
                        // 首个词是列表 head，其余作为原子子节点
                        let ws = raw0.split(" ");
                        let h = ws.get(0);
                        st_head.set(top, h);
                        st_started.set(top, String::from("1"));
                        let mut keep = String::from("");
                        let mut has = 0;
                        if raw0.len > h.len {
                            let rest_raw = raw0.substring(h.len + 1, raw0.len);
                            let rest_trim = String::from(rest_raw.trim());
                            if rest_trim != "" {
                                keep = rest_trim;
                                has = 1;
                            }
                            // str / lit-str 的载荷可为空串或含首尾空格：原样保留（含空载荷），
                            // 以便 render_ast 拼回 "(str <原样载荷>)"（空载荷时仍保留分隔空格）。
                            if h == "str" {
                                keep = rest_raw;
                                has = 1;
                            } else if h == "lit-str" {
                                keep = rest_raw;
                                has = 1;
                            }
                        }
                        if has == 1 {
                            let aid = nodes.len();
                            nodes.push(atom_node(keep));
                            let ktop2 = st_kids.len() - 1;
                            let kcur = st_kids.get(ktop2);
                            st_kids.set(ktop2, append_kid(kcur, int_str(aid)));
                        }
                    } else {
                        let aid = nodes.len();
                        nodes.push(atom_node(seg));
                        let ktop3 = st_kids.len() - 1;
                        let kcur3 = st_kids.get(ktop3);
                        st_kids.set(ktop3, append_kid(kcur3, int_str(aid)));
                    }
                }
            }
        }
    }
    Ast { nodes: nodes, root: root }
}

/// 前序 head 序列（节点级对拍用）：只发射列表节点 head，原子节点跳过。
/// 迭代式显式栈（先序：父节点 -> 子节点，按 kids 顺序）。
fn ast_kinds(ast: Ast) -> Vec<String> {
    let mut acc: Vec<String> = Vec::new();
    let mut work: Vec<String> = Vec::new();
    work.push(int_str(ast.root));
    while work.len() > 0 {
        let wtop = work.len() - 1;
        let w = work.remove(wtop);
        let idx = string_to_int(w);
        let nd = ast.nodes.get(idx);
        if nd.kind == "L" {
            acc.push(nd.head);
        }
        if nd.kids != "" {
            let ks = nd.kids.split(" ");
            let mut j = ks.len();
            while j > 0 {
                j = j - 1;
                work.push(ks.get(j));
            }
        }
    }
    acc
}

/// 从 arena 回渲染规范 S-表达式文本（应与输入逐字节一致）。
/// 迭代式显式栈：节点项为纯索引串；字面量项以 '@' 前缀（'@' 后为原文，如 " " / ")"）。
fn render_node(nodes: &Vec<AstNode>, root: String) -> String {
    let mut acc = String::from("");
    let mut work: Vec<String> = Vec::new();
    work.push(root);
    while work.len() > 0 {
        let wtop = work.len() - 1;
        let w = work.remove(wtop);
        if w.starts_with("@") {
            acc = acc + w.substring(1, w.len);
        } else {
            let idx = string_to_int(w);
            let nd = nodes.get(idx);
            if nd.kind == "A" {
                acc = acc + nd.head;
            } else {
                acc = acc + String::from("(") + nd.head;
                let mut seq: Vec<String> = Vec::new();
                if nd.kids != "" {
                    let ks = nd.kids.split(" ");
                    let mut k = 0;
                    while k < ks.len() {
                        seq.push(String::from("@ "));
                        seq.push(ks.get(k));
                        k = k + 1;
                    }
                }
                seq.push(String::from("@)"));
                let mut q = seq.len();
                while q > 0 {
                    q = q - 1;
                    work.push(seq.get(q));
                }
            }
        }
    }
    acc
}

/// 从 arena 回渲染规范 S-表达式文本（应与输入逐字节一致）。
/// 注意：此处内联同一渲染循环（不转调 render_node(&ast.nodes, ...)）——Rlyeh 对
/// 「借用结构体形参的 Vec 字段再传入 &Vec 形参」会生成异常 IR（运行时段错误，
/// 实测 parse_program -> render_ast 崩溃）；形参为局部 Vec 的 render_node 无此问题。
fn render_ast(ast: Ast) -> String {
    let mut acc = String::from("");
    let mut work: Vec<String> = Vec::new();
    work.push(int_str(ast.root));
    while work.len() > 0 {
        let wtop = work.len() - 1;
        let w = work.remove(wtop);
        if w.starts_with("@") {
            acc = acc + w.substring(1, w.len);
        } else {
            let idx = string_to_int(w);
            let nd = ast.nodes.get(idx);
            if nd.kind == "A" {
                acc = acc + nd.head;
            } else {
                acc = acc + String::from("(") + nd.head;
                let mut seq: Vec<String> = Vec::new();
                if nd.kids != "" {
                    let ks = nd.kids.split(" ");
                    let mut k = 0;
                    while k < ks.len {
                        seq.push(String::from("@ "));
                        seq.push(ks.get(k));
                        k = k + 1;
                    }
                }
                seq.push(String::from("@)"));
                let mut q = seq.len();
                while q > 0 {
                    q = q - 1;
                    work.push(seq.get(q));
                }
            }
        }
    }
    acc
}

/// 节点总数（调试/断言辅助）。
fn ast_node_count(ast: Ast) -> i64 {
    ast.nodes.len()
}

/// 列表节点 head 序列行数（调试/断言辅助）。
fn ast_kind_count(ast: Ast) -> i64 {
    ast_kinds(ast).len()
}

/// 列表节点（kind = "L"）总数（M-M3b 节点级断言辅助）。
/// 原子节点数会因 parse_ast 的「连续原子合并为一段」策略与手工构造存在 ±差，
/// 但列表节点一一对应 S-表达式的括号结构，故列表节点数是可靠的结构对拍量。
fn ast_list_node_count(ast: Ast) -> i64 {
    let mut cnt = 0;
    let mut i = 0;
    while i < ast.nodes.len {
        let nd = ast.nodes.get(i);
        if nd.kind == "L" { cnt = cnt + 1; }
        i = i + 1;
    }
    cnt
}

// ============================================================================
// M-M3b：parser 侧共享 arena 节点构造辅助。
//
// parser 在归约点调用以下函数**直接向 arena 追加类型化节点**（不再拼接规范文本），
// 最终由 render_ast / render_node 统一渲染。返回值均为「节点索引串」（int_str 编码，
// 与 AstNode.kids 同构），可直接交给 append_kid / ast_cat 组合。
// 约束：使用 &mut Vec<AstNode> 形参；Rlyeh 已实证可跨函数层级传递与原地修改
// （get 读字段 + set 写回），但不经「按值传参 + 返回」的辅助函数。
// ============================================================================

/// 向 arena 追加原子节点，返回其索引串。
fn ast_push_atom(nodes: &mut Vec<AstNode>, text: String) -> String {
    nodes.push(atom_node(text));
    int_str(nodes.len() - 1)
}

/// 向 arena 追加列表节点，返回其索引串。
fn ast_push_list(nodes: &mut Vec<AstNode>, head: String, kids: String) -> String {
    nodes.push(list_node(head, kids));
    int_str(nodes.len() - 1)
}

/// 向 arena 追加「列表 head + 单个原子子节点」的节点（如 `(int 1)` / `(var x)`）。
fn ast_push_leaf(nodes: &mut Vec<AstNode>, head: String, text: String) -> String {
    let a = ast_push_atom(nodes, text);
    ast_push_list(nodes, head, a)
}

/// 拼接两段索引串（空格分隔）；b 为空时原样返回 a（不产生尾随空格）。
fn ast_cat(a: String, b: String) -> String {
    let mut r = a;
    if b != "" { r = append_kid(r, b); }
    r
}
