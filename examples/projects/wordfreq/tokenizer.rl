// ===== 词法切分：文本 → 单词列表 =====
//
// 非字母字符（空白 / 标点 / 数字 / 符号）一律作为单词分隔符；
// ASCII 大写归一为小写（`Rlyeh` 与 `rlyeh` 计为同一词）。
// MVP 字符集仅 ASCII 字母（字节语义，String::chars 迭代码点，ASCII 即字节），
// UTF-8 非 ASCII 字节视为分隔符。

pub fn tokenize(text: String) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    // V2（2026-08-29）：`chars()` 现返回 `Chars` 码点迭代器（不再返回可索引的
    // Vec），改用 for 顺序遍历码点（MVP 字符集仅 ASCII 字节语义）。
    for c in text.chars() {
        if is_letter(c as i64) == 1 {
            cur.push_byte(lower_byte(c as i64));
        } else {
            if cur.len() > 0 {
                words.push(cur);
                cur = String::new();
            }
        }
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
