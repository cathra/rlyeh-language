// M-M1（SH-P2-7）Rlyeh 版 lexer 切片1：标识符/关键字/整数/字符串/运算符/注释。
//
// 这是「用 Rlyeh 重写编译器前端」自举 PoC 的首个切片：前端逻辑体已是 Rlyeh
// 源码，可由自有工具链（现有 Rust rlyeh-driver）编译运行。输出规范化 token 文本
// （每行一个），格式与 `rlyeh run <f> --emit tokens` 的 Rust oracle 逐行对齐，
// 由 `crates/rlyeh-driver/tests/self_host_lexer.rs` 差分对拍。
//
// 切片1 范围（M-M1a，已落地）：
//   跳过：空格/制表/\r/\n、行注释 `//`、块注释 `/* */`（支持嵌套）
//   标识符 + 全量关键字表
//   整数：十进制、0x(hex)/0b(bin)/0o(oct)、`_` 分隔、尾随类型后缀（吸收）
//   字符串：`"..."`（切片1 不含转义，corpus 避免转义字符）
//   运算符：单字符 + 多字符（== != <= >= && || -> => += -= *= /= %= << >> .. ..< ... <..）
// 切片1 不含（后续切片）：浮点、字符/生命周期、时间字面量、原始字符串、转义解码。
//
// Rlyeh 语言注意点（已踩坑）：
//   * 字符串字面量是 `string` 值类型，需用 `String::from(...)` 转成 `String` 对象
//     才能调用 `.len`/`.get`/`.substring`；从 `-> String` 函数返回字面量须用
//     `String::from` 包裹（直接 `return "x"` 会返回槽类型错配 -> 运行时崩溃）。

fn is_ident_cont(c: i64) -> i64 {
    if c == 95 { return 1; }
    if c >= 65 && c <= 90 { return 1; }
    if c >= 97 && c <= 122 { return 1; }
    if c >= 48 && c <= 57 { return 1; }
    0
}

fn char_str(b: i64) -> String {
    let s = String::with_capacity(1);
    s.push_byte(b);
    s
}

fn i64_to_string(n: i64) -> String {
    if n == 0 { return String::from("0"); }
    let mut neg = 0;
    let mut v = n;
    if n < 0 { neg = 1; v = -n; }
    let mut buf = String::new();
    while v > 0 {
        let d = v % 10;
        buf = char_str(48 + d) + buf;
        v = v / 10;
    }
    if neg == 1 { buf = String::from("-") + buf; }
    buf
}

fn keyword_or_ident(text: String) -> String {
    if text == "let" { return String::from("let"); }
    if text == "mut" { return String::from("mut"); }
    if text == "const" { return String::from("const"); }
    if text == "static" { return String::from("static"); }
    if text == "fn" { return String::from("fn"); }
    if text == "return" { return String::from("return"); }
    if text == "pub" { return String::from("pub"); }
    if text == "priv" { return String::from("priv"); }
    if text == "if" { return String::from("if"); }
    if text == "else" { return String::from("else"); }
    if text == "match" { return String::from("match"); }
    if text == "for" { return String::from("for"); }
    if text == "while" { return String::from("while"); }
    if text == "loop" { return String::from("loop"); }
    if text == "break" { return String::from("break"); }
    if text == "continue" { return String::from("continue"); }
    if text == "true" { return String::from("true"); }
    if text == "false" { return String::from("false"); }
    if text == "and" { return String::from("and"); }
    if text == "or" { return String::from("or"); }
    if text == "not" { return String::from("not"); }
    if text == "struct" { return String::from("struct"); }
    if text == "enum" { return String::from("enum"); }
    if text == "impl" { return String::from("impl"); }
    if text == "protocol" { return String::from("protocol"); }
    if text == "type" { return String::from("type"); }
    if text == "where" { return String::from("where"); }
    if text == "Self" { return String::from("Self"); }
    if text == "region" { return String::from("region"); }
    if text == "gc_region" { return String::from("gc_region"); }
    if text == "in" { return String::from("in"); }
    if text == "transfer" { return String::from("transfer"); }
    if text == "out" { return String::from("out"); }
    if text == "of" { return String::from("of"); }
    if text == "unsafe" { return String::from("unsafe"); }
    if text == "actor" { return String::from("actor"); }
    if text == "async" { return String::from("async"); }
    if text == "await" { return String::from("await"); }
    if text == "spawn" { return String::from("spawn"); }
    if text == "send" { return String::from("send"); }
    if text == "recv" { return String::from("recv"); }
    if text == "mod" { return String::from("mod"); }
    if text == "use" { return String::from("use"); }
    if text == "as" { return String::from("as"); }
    if text == "extern" { return String::from("extern"); }
    if text == "dyn" { return String::from("dyn"); }
    "IDENT " + text
}

fn lex(src: String) -> Vec<String> {
    let mut toks: Vec<String> = Vec::new();
    let n = src.len;
    let mut i = 0;
    while i < n {
        let c = src.get(i);
        if c == 32 || c == 9 || c == 13 || c == 10 {
            i = i + 1;
            continue;
        }
        if c == 47 && i + 1 < n && src.get(i + 1) == 47 {
            i = i + 2;
            while i < n && src.get(i) != 10 { i = i + 1; }
            continue;
        }
        if c == 47 && i + 1 < n && src.get(i + 1) == 42 {
            i = i + 2;
            let mut depth = 1;
            while depth > 0 && i < n {
                if i + 1 < n && src.get(i) == 47 && src.get(i + 1) == 42 {
                    i = i + 2;
                    depth = depth + 1;
                } else if i + 1 < n && src.get(i) == 42 && src.get(i + 1) == 47 {
                    i = i + 2;
                    depth = depth - 1;
                } else {
                    i = i + 1;
                }
            }
            continue;
        }
        if c == 95 || (c >= 65 && c <= 90) || (c >= 97 && c <= 122) {
            let start = i;
            while i < n && is_ident_cont(src.get(i)) == 1 {
                i = i + 1;
            }
            let text = src.substring(start, i);
            toks.push(keyword_or_ident(text));
            continue;
        }
        if c >= 48 && c <= 57 {
            let mut value = 0;
            let mut base = 10;
            let mut idx = i;
            if c == 48 && i + 1 < n {
                let d = src.get(i + 1);
                if d == 120 { base = 16; idx = i + 2; }
                else if d == 98 { base = 2; idx = i + 2; }
                else if d == 111 { base = 8; idx = i + 2; }
            }
            while idx < n {
                let d = src.get(idx);
                if d >= 48 && d <= 57 {
                    value = value * base + (d - 48);
                    idx = idx + 1;
                } else if base == 16 && d >= 97 && d <= 102 {
                    value = value * 16 + (d - 87);
                    idx = idx + 1;
                } else if base == 16 && d >= 65 && d <= 70 {
                    value = value * 16 + (d - 55);
                    idx = idx + 1;
                } else if d == 95 {
                    idx = idx + 1;
                } else if d >= 65 && d <= 90 {
                    idx = idx + 1;
                } else if d >= 97 && d <= 122 {
                    idx = idx + 1;
                } else {
                    break;
                }
            }
            i = idx;
            toks.push("INT " + i64_to_string(value));
            continue;
        }
        if c == 34 {
            i = i + 1;
            let mut s = String::new();
            while i < n && src.get(i) != 34 {
                s.push_byte(src.get(i));
                i = i + 1;
            }
            i = i + 1;
            toks.push("STR " + s);
            continue;
        }
        let c2 = if i + 1 < n { src.get(i + 1) } else { 0 };
        let c3 = if i + 2 < n { src.get(i + 2) } else { 0 };
        if c == 46 && c2 == 46 {
            if c3 == 46 { i = i + 3; toks.push("DOTDOTDOT"); }
            else if c3 == 60 { i = i + 3; toks.push("DOTDOTLT"); }
            else { i = i + 2; toks.push("RANGE"); }
            continue;
        }
        if c == 43 && c2 == 61 { i = i + 2; toks.push("+="); continue; }
        if c == 45 && c2 == 62 { i = i + 2; toks.push("->"); continue; }
        if c == 45 && c2 == 61 { i = i + 2; toks.push("-="); continue; }
        if c == 42 && c2 == 61 { i = i + 2; toks.push("*="); continue; }
        if c == 47 && c2 == 61 { i = i + 2; toks.push("/="); continue; }
        if c == 37 && c2 == 61 { i = i + 2; toks.push("%="); continue; }
        if c == 61 && c2 == 61 { i = i + 2; toks.push("=="); continue; }
        if c == 61 && c2 == 62 { i = i + 2; toks.push("=>"); continue; }
        if c == 33 && c2 == 61 { i = i + 2; toks.push("!="); continue; }
        if c == 60 && c2 == 61 { i = i + 2; toks.push("<="); continue; }
        if c == 60 && c2 == 46 && c3 == 46 { i = i + 3; toks.push("LTDOTDOT"); continue; }
        if c == 60 && c2 == 60 { i = i + 2; toks.push("<<"); continue; }
        if c == 62 && c2 == 61 { i = i + 2; toks.push(">="); continue; }
        if c == 62 && c2 == 62 { i = i + 2; toks.push(">>"); continue; }
        if c == 38 && c2 == 38 { i = i + 2; toks.push("&&"); continue; }
        if c == 124 && c2 == 124 { i = i + 2; toks.push("||"); continue; }
        i = i + 1;
        if c == 43 { toks.push("+"); }
        else if c == 45 { toks.push("-"); }
        else if c == 42 { toks.push("*"); }
        else if c == 47 { toks.push("/"); }
        else if c == 37 { toks.push("%"); }
        else if c == 61 { toks.push("="); }
        else if c == 60 { toks.push("<"); }
        else if c == 62 { toks.push(">"); }
        else if c == 33 { toks.push("!"); }
        else if c == 38 { toks.push("&"); }
        else if c == 124 { toks.push("|"); }
        else if c == 94 { toks.push("^"); }
        else if c == 40 { toks.push("("); }
        else if c == 41 { toks.push(")"); }
        else if c == 123 { toks.push("{"); }
        else if c == 125 { toks.push("}"); }
        else if c == 91 { toks.push("["); }
        else if c == 93 { toks.push("]"); }
        else if c == 44 { toks.push(","); }
        else if c == 58 { toks.push(":"); }
        else if c == 59 { toks.push(";"); }
        else if c == 46 { toks.push("."); }
        else if c == 64 { toks.push("@"); }
        else if c == 35 { toks.push("#"); }
        else if c == 36 { toks.push("$"); }
        else if c == 63 { toks.push("?"); }
        else { toks.push("?"); }
    }
    toks
}
