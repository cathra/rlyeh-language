// ===== 词法切分：文本 → 单词列表 =====
//
// 非字母字符（空白 / 标点 / 数字 / 符号）一律作为单词分隔符；
// ASCII 大写归一为小写（`Rlyeh` 与 `rlyeh` 计为同一词）。
// MVP 字符集仅 ASCII 字母（字节语义，String::chars 为字节级），
// UTF-8 非 ASCII 字节视为分隔符。

pub fn tokenize(text: String) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    let cs = text.chars();
    let n = cs.len();
    let mut i = 0;
    while i < n {
        let c = cs[i];
        if is_letter(c) == 1 {
            cur.push_byte(lower_byte(c));
        } else {
            if cur.len() > 0 {
                words.push(cur);
                cur = String::new();
            }
        }
        i = i + 1;
    }
    if cur.len() > 0 {
        words.push(cur);
    }
    words
}

// ASCII 字母判断（'A'-'Z' = 65-90，'a'-'z' = 97-122）；返回 1 / 0
fn is_letter(c: i64) -> i64 {
    if 65 <= c <= 90 {
        return 1;
    }
    if 97 <= c <= 122 {
        return 1;
    }
    0
}

// ASCII 大写转小写（+32），其余字节原样
fn lower_byte(c: i64) -> i64 {
    if 65 <= c <= 90 {
        return c + 32;
    }
    c
}
