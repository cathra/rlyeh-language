// ============================================================================
// M-M2b/M-M2c/M-M4/M-M5/M-M6/M-M8/M-M9/M-M3b：程序级迭代解析。
//
// M-M3b（SH-P2-7）：归约点不再拼接规范文本，而是调用 ast.rl 的共享 arena
// 构造辅助（ast_push_* / ast_cat）直接构造类型化节点；语句/块/项/程序全部以
// 「节点索引串」组装（buffer 栈保存子节点索引序列），最终由 render_ast 统一渲染。
// 表达式 / 类型 / 模式 / 函数头 / struct·enum 均由各自 shard 返回节点索引串。
//
// 设计要点（延续既有无递归原则）：
//   块：迭代式 buffer 栈（`{` 入栈新 buffer、`}` 出栈包成 block 节点）；
//   控制流：迭代式控制帧栈（cf_*，整数态一律 String 编码规避 Vec<i64>.get 返回 ptr）；
//   全程零递归、零 Vec<i64>。
// ============================================================================

// 程序级解析主循环（M-M3b 共享 arena 版）：返回 program 根节点索引串。
fn parse_program_impl(tokens: Vec<String>, nodes: &mut Vec<AstNode>) -> String {
    let n = tokens.len;
    let mut ti = 0;
    let mut bufstack: Vec<String> = Vec::new();
    bufstack.push(String::new());   // 程序体 buffer：子节点索引序列
    // 控制帧栈（M-M2c）：与 bufstack 配合解析 if/while/loop 的块体（不引入递归）。
    //   Rlyeh 的 Vec<i64>::get/pop 返回 ptr 而非 i64，故所有整数态均以 String 编码
    //   （int_to_string / "0"/"1"/"2" / "stmt"/"let" / "1"=else-if 子帧）。
    //   cf_kind:   "if"/"while"/"loop"/"for"/"match"/"fn"
    //   cf_cond:   条件节点（if/while）、迭代器节点（for）、subject 节点（match）、
    //              header 子节点索引序列（fn）
    //   cf_then:  then 块节点（then '}' 闭合时写入）；match 为臂节点序列；fn 不用
    //   cf_else:  else 块节点（else '}' 闭合时写入；否则 ""）
    //   cf_state: "0"=等待 then '}'；"1"=then 已闭合；"2"=已见 else 待 else 块 '}'
    //   cf_elseif:"1"=本帧是 else if 子帧，收束结果挂到其下方帧的 else；"0"=否
    //   cf_sink:  "stmt"=收束按 semi/bare 落到父 buffer；"let"=收束组装 (let ...)
    //   cf_depth: int_to_string(建帧时 bufstack 深度)，用于判定闭合 '}' 是否属于本帧
    let mut cf_kind: Vec<String> = Vec::new();
    let mut cf_cond: Vec<String> = Vec::new();
    let mut cf_then: Vec<String> = Vec::new();
    let mut cf_else: Vec<String> = Vec::new();
    let mut cf_state: Vec<String> = Vec::new();
    let mut cf_elseif: Vec<String> = Vec::new();
    let mut cf_sink: Vec<String> = Vec::new();
    let mut cf_depth: Vec<String> = Vec::new();
    // cf_pat: for 循环的模式节点（其余控制帧不占用，按顺序与 cf_kind 同步 pop）
    let mut cf_pat: Vec<String> = Vec::new();
    // M-M5 match 帧专用缓冲（match 同一时刻仅一层在顶层活动，故模式缓冲为全局单份）
    let mut match_pat: Vec<String> = Vec::new();            // 当前臂正在收集的模式/守卫 token
    let mut match_arm_pat_done: Vec<String> = Vec::new();   // 有守卫时暂存的 finalized 模式 token
    let mut match_seen: Vec<String> = Vec::new();           // per-frame："0"=未见 =>（区分 match 自身 {} 与臂 body {}）
    let mut match_arm_pat_str: Vec<String> = Vec::new();    // per-frame：当前臂 finalized 模式节点
    let mut match_arm_guard_str: Vec<String> = Vec::new();  // per-frame：当前臂守卫节点（无守卫为 ""）
    // pending let 信息（M-M2c/M-M3b：控制流作 let 初始化时暂存节点，帧收束后组装 let 节点）
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
                        bufstack.push(String::new());
                        continue;
                    }
                    if tok == "}" {
                        // 闭合 match：组装 (match <SUBJ> <ARMS>)
                        let subject = cf_cond.get(mk_idx);
                        let arms = cf_then.get(mk_idx);
                        let node = ast_push_list(nodes, String::from("match"), ast_cat(subject, arms));
                        ti = ti + 1;
                        cf_kind.pop(); cf_cond.pop(); cf_then.pop(); cf_else.pop();
                        cf_state.pop(); cf_elseif.pop(); cf_sink.pop(); cf_depth.pop();
                        cf_pat.pop(); match_seen.pop();
                        match_arm_pat_str.pop(); match_arm_guard_str.pop();
                        while match_pat.len() > 0 { match_pat.pop(); }
                        while match_arm_pat_done.len() > 0 { match_arm_pat_done.pop(); }
                        let nxt = if ti < n { tokens.get(ti) } else { String::from("") };
                        let mut s = node;
                        if nxt == ";" { ti = ti + 1; s = ast_push_list(nodes, String::from("semi"), node); }
                        else if nxt == "}" { s = node; }
                        else { s = ast_push_list(nodes, String::from("semi"), node); }
                        let po = bufstack.pop();
                        let mut p = match po { Option::Some(x) => x, Option::None => String::new() };
                        p = ast_cat(p, s);
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
                            pat = parse_match_pattern(match_arm_pat_done, nodes);
                            let ge = parse_expr_arena(match_pat, 0, nodes);
                            grd = ge.id;
                            while match_arm_pat_done.len() > 0 { match_arm_pat_done.pop(); }
                        } else {
                            pat = parse_match_pattern(match_pat, nodes);
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
                        while kk < match_pat.len {
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
                let block = ast_push_list(nodes, String::from("block"), body);
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
                            let mut akids = pat;
                            if grd.len() > 0 { akids = ast_cat(akids, grd); }
                            akids = ast_cat(akids, block);
                            let arm = ast_push_list(nodes, String::from("arm"), akids);
                            let at = cf_then.get(t_idx);
                            cf_then.set(t_idx, ast_cat(at, arm));
                            cf_state.set(t_idx, String::from("0"));  // 等待下一臂或 match 闭合
                            continue;
                        }
                        if top_kind == "fn" {
                            // 函数体闭合：组装 (fn <HDR kids> (block ...)) 并落到父 buffer（裸）
                            let hdr = cf_cond.get(t_idx);
                            let node = ast_push_list(nodes, String::from("fn"), ast_cat(hdr, block));
                            cf_kind.pop(); cf_cond.pop(); cf_then.pop(); cf_else.pop();
                            cf_state.pop(); cf_elseif.pop(); cf_sink.pop(); cf_depth.pop(); cf_pat.pop();
                            let po = bufstack.pop();
                            let mut p = match po { Option::Some(x) => x, Option::None => String::new() };
                            p = ast_cat(p, node);
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
                                    let r = parse_expr_arena(tokens, ti, nodes);
                                    cf_kind.push(String::from("if"));
                                    cf_cond.push(r.id);
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
                                    let mut kids = ast_cat(fc, ft);
                                    if fe.len() > 0 { kids = ast_cat(kids, fe); }
                                    node = ast_push_list(nodes, String::from("if"), kids);
                                } else if fk == "while" {
                                    node = ast_push_list(nodes, String::from("while"), ast_cat(fc, ft));
                                } else if fk == "for" {
                                    node = ast_push_list(nodes, String::from("for"), ast_cat(ast_cat(fp, fc), ft));
                                } else {
                                    node = ast_push_list(nodes, String::from("loop"), ft);
                                }
                                if feif == "1" {
                                    // 级联：else if 的 else 分支是块，故将嵌套 if 包成 block 节点
                                    // 再挂到下方帧的 else，继续收束下方帧
                                    let below = cf_else.len() - 1;
                                    cf_else.set(below, ast_push_list(nodes, String::from("block"), node));
                                    cf_state.set(below, String::from("1"));
                                } else if sk == "let" {
                                    // 挂到 pending let 初始化
                                    let lp_o = pend_let_pat.pop();
                                    let lp = match lp_o { Option::Some(x) => x, Option::None => String::new() };
                                    let lt_o = pend_let_type.pop();
                                    let lt = match lt_o { Option::Some(x) => x, Option::None => String::new() };
                                    let lm_o = pend_let_mut.pop();
                                    let lm = match lm_o { Option::Some(x) => x, Option::None => String::new() };
                                    let mut lk = String::new();
                                    if lm == "1" { lk = ast_push_atom(nodes, String::from("mut")); }
                                    lk = ast_cat(lk, lp);
                                    if lt.len() > 0 { lk = ast_cat(lk, lt); }
                                    lk = ast_cat(lk, node);
                                    let ln = ast_push_list(nodes, String::from("let"), lk);
                                    let po = bufstack.pop();
                                    let mut pp = match po { Option::Some(x) => x, Option::None => String::new() };
                                    pp = ast_cat(pp, ln);
                                    bufstack.push(pp);
                                    fin = 0;
                                } else {
                                    // 顶层控制语句：按 semi/bare 落到父 buffer
                                    let nx = if ti < n { tokens.get(ti) } else { String::from("") };
                                    let mut s = node;
                                    if nx == ";" { ti = ti + 1; s = ast_push_list(nodes, String::from("semi"), node); }
                                    else if nx == "}" { s = node; }
                                    else { s = ast_push_list(nodes, String::from("semi"), node); }
                                    let po = bufstack.pop();
                                    let mut pp = match po { Option::Some(x) => x, Option::None => String::new() };
                                    pp = ast_cat(pp, s);
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
                    append = ast_push_list(nodes, String::from("semi"), block);
                } else if nxt == "}" {
                    append = block;   // 紧跟 '}' → 父块块尾表达式，裸
                } else {
                    append = ast_push_list(nodes, String::from("semi"), block);  // 后接更多语句 → Semi
                }
                let p_opt = bufstack.pop();
                let mut p = match p_opt { Option::Some(x) => x, Option::None => String::new() };
                p = ast_cat(p, append);
                bufstack.push(p);
            } else {
                ti = ti + 1;   // 多余 '}' 直接跳过
            }
            continue;
        }
        // 块开始
        if tok == "{" {
            ti = ti + 1;
            bufstack.push(String::new());
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
            let pattern = parse_pattern_tokens(pat_toks, nodes);
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
                type_sexpr = parse_type_tokens(fixed, nodes);
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
                    let r2 = parse_expr_arena(tokens, ti, nodes);
                    cinit = r2.id;
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
            let r = parse_expr_arena(tokens, ti, nodes);
            let init = r.id;
            ti = r.ti;
            if ti < n && tokens.get(ti) == ";" { ti = ti + 1; }
            let mut lk = String::new();
            if is_mut == 1 { lk = ast_push_atom(nodes, String::from("mut")); }
            lk = ast_cat(lk, pattern);
            if type_sexpr.len > 0 { lk = ast_cat(lk, type_sexpr); }
            lk = ast_cat(lk, init);
            let node = ast_push_list(nodes, String::from("let"), lk);
            let p_opt = bufstack.pop();
            let mut p = match p_opt { Option::Some(x) => x, Option::None => String::new() };
            p = ast_cat(p, node);
            bufstack.push(p);
            continue;
        }
        // 控制流语句（M-M2c）：if / while / loop（语句形态）
        if tok == "if" {
            ti = ti + 1;
            let r = parse_expr_arena(tokens, ti, nodes);
            cf_kind.push(String::from("if"));
            cf_cond.push(r.id);
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
            let r = parse_expr_arena(tokens, ti, nodes);
            cf_kind.push(String::from("while"));
            cf_cond.push(r.id);
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
            let pattern = parse_pattern_tokens(pat_toks, nodes);
            // 越过 'in'
            if ti < n && tokens.get(ti) == "in" { ti = ti + 1; }
            // 解析迭代器表达式直到块体 '{'
            let r = parse_expr_arena(tokens, ti, nodes);
            let iter = r.id;
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
            let r = parse_expr_arena(tokens, ti, nodes);
            let subject = r.id;
            ti = r.ti;
            // 越过 match 自身的 '{'（不入 buffer 栈）
            if ti < n && tokens.get(ti) == "{" { ti = ti + 1; }
            // 清空全局模式缓冲
            while match_pat.len() > 0 { match_pat.pop(); }
            while match_arm_pat_done.len() > 0 { match_arm_pat_done.pop(); }
            cf_kind.push(String::from("match"));
            cf_cond.push(subject);
            cf_then.push(String::new());   // 累积臂节点序列
            cf_else.push(String::new());
            cf_state.push(String::from("0"));  // 0=收集模式 / 3=收集守卫 / 1=等待 body
            cf_elseif.push(String::from("0"));
            cf_sink.push(String::from("stmt"));
            cf_depth.push(int_to_string(bufstack.len()));
            cf_pat.push(String::new());
            match_seen.push(String::from("1"));        // match 自身 '{' 已在分发处跳过，路由中 '{' 一律为臂 body
            match_arm_pat_str.push(String::new());     // 当前臂 finalized 模式节点
            match_arm_guard_str.push(String::new());   // 当前臂守卫节点（无守卫为 ""）
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
            // 解析函数头（名 / 参数 / 返回类型）-> header 子节点索引序列 + 新位置
            let h = parse_fn_header(tokens, ti, fpub, nodes);
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
        // 结构体声明（M-M8）：[pub] struct <name> { <field>: <Type>, ... }
        if tok == "struct" || (tok == "pub" && peek1 == "struct") {
            let mut spub = 0;
            if tok == "pub" { spub = 1; ti = ti + 1; }
            ti = ti + 1;   // 越过 struct
            let r = parse_struct_decl(tokens, ti, spub, nodes);
            ti = r.ti;
            let p_opt = bufstack.pop();
            let mut p = match p_opt { Option::Some(x) => x, Option::None => String::new() };
            p = ast_cat(p, r.s);
            bufstack.push(p);
            continue;
        }
        // 枚举声明（M-M8）：[pub] enum <name> { V1, V2(T, U), V3 { f: T } }
        if tok == "enum" || (tok == "pub" && peek1 == "enum") {
            let mut epub = 0;
            if tok == "pub" { epub = 1; ti = ti + 1; }
            ti = ti + 1;   // 越过 enum
            let r = parse_enum_decl(tokens, ti, epub, nodes);
            ti = r.ti;
            let p_opt = bufstack.pop();
            let mut p = match p_opt { Option::Some(x) => x, Option::None => String::new() };
            p = ast_cat(p, r.s);
            bufstack.push(p);
            continue;
        }
        // 非 fn/struct/enum 的 pub 项（M-M6a/M-M8a 未覆盖）：跳过 pub token
        if tok == "pub" { ti = ti + 1; continue; }
        // 其余：表达式语句（含 return；块表达式已由 '{' 分支处理）
        let r = parse_expr_arena(tokens, ti, nodes);
        let e = r.id;
        ti = r.ti;
        let nxt = if ti < n { tokens.get(ti) } else { String::from("") };
        let mut s = e;
        if nxt == ";" {
            ti = ti + 1;
            s = ast_push_list(nodes, String::from("semi"), e);
        } else if nxt == "}" {
            s = e;   // 紧跟 '}' → 块尾表达式，裸
        } else {
            s = ast_push_list(nodes, String::from("semi"), e);   // 顶层裸表达式 / 后接更多语句 → Semi
        }
        let p_opt = bufstack.pop();
        let mut p = match p_opt { Option::Some(x) => x, Option::None => String::new() };
        p = ast_cat(p, s);
        bufstack.push(p);
    }
    let body_opt = bufstack.pop();
    let body = match body_opt { Option::Some(x) => x, Option::None => String::new() };
    ast_push_list(nodes, String::from("program"), body)
}

// 解析整段程序源码，返回共享 arena 中的类型化 AST（M-M3b）。
// 供节点级断言（parser 内部节点数 vs ast.rl parse_ast 节点数）使用。
fn parse_program_ast(src: String) -> Ast {
    let tokens = tokenize(src);
    let mut nodes: Vec<AstNode> = Vec::new();
    let root = { parse_program_impl(tokens, &mut nodes) };
    Ast { nodes: nodes, root: string_to_int(root) }
}

// 解析整段程序源码，返回规范 S-表达式程序文本（由 arena 统一渲染，M-M3b）。
fn parse_program(src: String) -> String {
    let a = parse_program_ast(src);
    render_ast(a)
}
