// convert/module.rl：数值 / JSON <-> 字符串转换自由函数（2026-09-18 由 core.rl 拆分）。

// ---------------------------------------------------------------------------
// 数值 <-> 字符串转换（顶层自由函数，i64 与 String 互转）
// ---------------------------------------------------------------------------
// 整数转字符串：负数加 '-'（45）前缀；0 特判输出 '0'（48）；先算位数
// ndigits，再以位权 place（10^(ndigits-1)）从最高位逐位 `(v / place) % 10`
// 输出。全部 i64 算术（不用 Vec——`Vec::new()` 的 Infer 类型参数在标准库
// 自举时无上下文注解可统一）。i64::MIN 取负溢出 MVP 不处理。
fn int_to_string(n: i64) -> String {
    let mut buf = String::new();
    let mut v = n;
    if v < 0 {
        buf.push_byte(45);
        v = 0 - v;
    }
    if v == 0 {
        buf.push_byte(48);
        return buf;
    }
    let mut ndigits = 0;
    let mut tmp = v;
    while tmp > 0 {
        ndigits = ndigits + 1;
        tmp = tmp / 10;
    }
    let mut place = 1;
    let mut k = 1;
    while k < ndigits {
        place = place * 10;
        k = k + 1;
    }
    while place >= 1 {
        let d = (v / place) % 10;
        buf.push_byte(48 + d);
        place = place / 10;
    }
    buf
}
// JSON 字符串转义（L2 serde，`json.stringify` 使用）：
// `"`(34) → `\"`、`\`(92) → `\\`、换行(10) → `\n`、制表(9) → `\t`，其余字节原样
fn json_escape(s: String) -> String {
    let mut buf = String::new();
    let mut i = 0;
    while i < s.len {
        let c = s.get(i);
        if c == 34 {
            buf.push_byte(92);
            buf.push_byte(34);
        } else if c == 92 {
            buf.push_byte(92);
            buf.push_byte(92);
        } else if c == 10 {
            buf.push_byte(92);
            buf.push_byte(110);
        } else if c == 9 {
            buf.push_byte(92);
            buf.push_byte(116);
        } else {
            buf.push_byte(c);
        }
        i = i + 1;
    }
    buf
}
// JSON 字符串反转义（L2 serde，`json.parse::<String>` 使用）：
// 剥离首尾引号（34），还原 `\"`/`\\`/`\n`/`\t` 转义序列
fn json_unescape(s: String) -> String {
    let mut buf = String::new();
    let mut start = 0;
    if s.len > 0 {
        if s.get(0) == 34 {
            start = 1;
        }
    }
    let mut end = s.len;
    if s.len > 0 {
        if s.get(s.len - 1) == 34 {
            end = s.len - 1;
        }
    }
    let mut i = start;
    while i < end {
        let c = s.get(i);
        if c == 92 {
            if i + 1 < end {
                let n = s.get(i + 1);
                if n == 34 {
                    buf.push_byte(34);
                } else if n == 92 {
                    buf.push_byte(92);
                } else if n == 110 {
                    buf.push_byte(10);
                } else if n == 116 {
                    buf.push_byte(9);
                } else {
                    buf.push_byte(c);
                    buf.push_byte(n);
                }
                i = i + 2;
            } else {
                buf.push_byte(c);
                i = i + 1;
            }
        } else {
            buf.push_byte(c);
            i = i + 1;
        }
    }
    buf
}
// 字符串转整数：解析十进制数字（'0'-'9' = 48-57）累加；'-'（45）前缀为负；
// 遇非数字字符停止解析（返回已解析部分）；空串/无数字返回 0。
fn string_to_int(s: String) -> i64 {
    let mut result = 0;
    let mut i = 0;
    let mut negative = 0;
    if s.len > 0 {
        if s.get(0) == 45 {
            negative = 1;
            i = 1;
        }
    }
    while i < s.len {
        let c = s.get(i);
        if 48 <= c <= 57 {
            result = result * 10 + (c - 48);
            i = i + 1;
        } else {
            i = s.len;
        }
    }
    if negative == 1 {
        result = 0 - result;
    }
    result
}

// X2（2026-08-30）：f64 <-> String 转换（TOML f64 序列化/反序列化用）。
// 手写十进制格式化：符号 + 整数部分（复用 int_to_string）+ 最多 6 位小数（去尾随零）。
fn float_to_string(f: f64) -> String {
    let neg = f < 0.0;
    let v = if neg { 0.0 - f } else { f };
    let intpart = v as i64;
    let mut buf = int_to_string(intpart);
    let mut frac = v - (intpart as f64);
    if frac > 0.0 {
        buf.push_str(".");
        let mut i = 0;
        while i < 6 {
            frac = frac * 10.0;
            let d = frac as i64;
            buf.push_str(int_to_string(d));
            frac = frac - (d as f64);
            i = i + 1;
        }
    }
    if neg {
        let mut nb = String::from("-");
        nb.push_str(buf);
        nb
    } else {
        buf
    }
}

// X2（2026-08-30）：String -> f64 解析（支持可选负号 + 整数/小数部分，MVP 不支持指数）。
fn string_to_float(s: String) -> f64 {
    let mut i = 0;
    let mut neg = 0;
    if s.len > 0 {
        if s.get(0) == 45 {
            neg = 1;
            i = 1;
        }
    }
    let mut intval = 0.0;
    let mut dot_seen = 0;
    let mut stop = 0;
    while i < s.len && stop == 0 {
        let c = s.get(i);
        if c == 46 {
            dot_seen = 1;
            stop = 1;
        } else if c < 48 || c > 57 {
            stop = 1;
        } else {
            intval = intval * 10.0 + (c - 48) as f64;
            i = i + 1;
        }
    }
    let mut frac = 0.0;
    let mut divisor = 1.0;
    if dot_seen == 1 {
        i = i + 1;
        let mut fstop = 0;
        while i < s.len && fstop == 0 {
            let c = s.get(i);
            if c < 48 || c > 57 {
                fstop = 1;
            } else {
                frac = frac * 10.0 + (c - 48) as f64;
                divisor = divisor * 10.0;
                i = i + 1;
            }
        }
    }
    let mut result = intval + (frac / divisor);
    if neg == 1 {
        result = 0.0 - result;
    }
    result
}

// X2（2026-08-30）：严格 f64 解析（`toml.try_parse` 用）——合法十进制浮点返回
// `Ok`，含非数字字符（除可选负号 / 小数点）或空 / 无数字返回 `Err`。
fn parse_float_strict(s: String) -> Result<f64, String> {
    let mut i = 0;
    let mut neg = 0;
    if s.len > 0 {
        if s.get(0) == 45 {
            neg = 1;
            i = 1;
        }
    }
    if i >= s.len {
        return Result::Err(String::from("invalid float"));
    }
    let mut seen_dot = 0;
    let mut seen_digit = 0;
    while i < s.len {
        let c = s.get(i);
        if 48 <= c <= 57 {
            seen_digit = 1;
            i = i + 1;
        } else if c == 46 {
            if seen_dot == 1 {
                return Result::Err(String::from("invalid float"));
            }
            seen_dot = 1;
            i = i + 1;
        } else {
            return Result::Err(String::from("invalid float"));
        }
    }
    if seen_digit == 0 {
        return Result::Err(String::from("invalid float"));
    }
    Result::Ok(string_to_float(s))
}

// P2（2026-08-28）：严格整数解析（`json.try_parse`/`toml.try_parse` 用）——
// 全数字 + 可选负号，非法/部分合法输入返回 Err（替代 string_to_int 的宽松停止解析）。
fn parse_int_strict(s: String) -> Result<i64, String> {
    let mut i = 0;
    let mut negative = 0;
    if s.len > 0 {
        if s.get(0) == 45 {
            negative = 1;
            i = 1;
        }
    }
    if i >= s.len {
        return Result::Err(String::from("invalid integer"));
    }
    let mut result = 0;
    while i < s.len {
        let c = s.get(i);
        if 48 <= c <= 57 {
            result = result * 10 + (c - 48);
            i = i + 1;
        } else {
            return Result::Err(String::from("invalid integer"));
        }
    }
    if negative == 1 {
        result = 0 - result;
    }
    Result::Ok(result)
}

// P2（2026-08-28）：严格 JSON 字符串还原（`json.try_parse` 用）——校验首尾双引号闭合 +
// 转义合法性，失败返回 Err（替代 json_unescape 的无校验剥引号）。
fn json_unescape_checked(s: String) -> Result<String, String> {
    if s.len < 2 {
        return Result::Err(String::from("invalid string"));
    }
    if s.get(0) != 34 || s.get(s.len - 1) != 34 {
        return Result::Err(String::from("unterminated string"));
    }
    let mut buf = String::new();
    let mut i = 1;
    while i < s.len - 1 {
        let c = s.get(i);
        if c == 92 {
            if i + 1 < s.len - 1 {
                let n = s.get(i + 1);
                if n == 34 {
                    buf.push_byte(34);
                } else if n == 92 {
                    buf.push_byte(92);
                } else if n == 110 {
                    buf.push_byte(10);
                } else if n == 116 {
                    buf.push_byte(9);
                } else {
                    return Result::Err(String::from("invalid escape"));
                }
                i = i + 2;
            } else {
                return Result::Err(String::from("invalid escape"));
            }
        } else {
            buf.push_byte(c);
            i = i + 1;
        }
    }
    Result::Ok(buf)
}

// X2（2026-08-27）：引号感知分段——按分隔符分割，但跳过双引号字符串内的分隔符
//（值含逗号的 TOML 内联表/数组）。返回段（含原空格，调用方自行 trim）。
// X2（2026-08-30）：增强——跟踪 `[`/`{`/`"` 嵌套深度，仅当不在引号内且括号深度为 0
// 时逗号才作分隔符（嵌套数组/内联表/HashMap 正确分段）；每段 trim 去除首尾空白。
fn split_quoted(s: String, delim: i64) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut start = 0;
    let mut in_str = false;
    let mut depth = 0;
    let mut i = 0;
    while i < s.len {
        let c = s.get(i);
        if c == 34 {
            // 双引号切换字符串状态
            in_str = !in_str;
        } else if c == 91 || c == 123 {
            // '[' / '{'：进入嵌套（不在引号内才计数）
            if !in_str { depth = depth + 1; }
        } else if c == 93 || c == 125 {
            // ']' / '}'：离开嵌套
            if !in_str { depth = depth - 1; }
        } else if c == delim && !in_str && depth == 0 {
            let part = s.substring(start, i);
            parts.push(String::from(part.trim()));
            start = i + 1;
        }
        i = i + 1;
    }
    let last = s.substring(start, s.len);
    parts.push(String::from(last.trim()));
    parts
}

// ---------------------------------------------------------------------------
// HashMap<K, V>：Robin Hood 线性探测哈希表（开放寻址 + 交换 + 墓碑删除）
// 布局（7 槽）：槽 0 = keys 指针（[K; 0]），槽 1 = vals 指针（[V; 0]），
//               槽 2 = states 指针（[i64; 0]：0=空 1=占用 2=墓碑），
//               槽 3 = len（实际键值对数），槽 4 = used（占用+墓碑槽数），
//               槽 5 = cap（容量，恒为 2 的幂），槽 6 = dist 指针（[i64; 0]）。
// 键类型：整数键走 `hash_value` 内建（Knuth 乘法散列）；String 键由 typecheck
// 特判展开为 djb2 内容哈希（同内容恒同哈希），键相等比较走 String `==` 内容比较。
// 结果经 `& (cap - 1)` 位掩码定位槽位（cap 恒为 2 的幂：new=8、with_capacity 经
// `next_pow2` 规整、grow 翻倍——位掩码替代取模与条件回绕，省 idiv 与分支）。
// 其余聚合对象键暂不支持。
// Robin Hood 均衡：insert/grow 重插均执行「探测 + 交换」（穷者让位、富者就位），
// 链上键距离非减 → find 以 `dist[idx] < d` 提前终止（O(1) 判不存在），
// 距离存 dist 数组（槽 6，String 键零额外哈希）。负载因子 used/cap >= 7/8 时
// 翻倍 rehash（墓碑随之清除）——表更小、扩容总量减半；早退控住高负载探测。
// 注意：grow 重插必须是交换式，纯线性重插会破坏距离不变量 → 早退假阴性。
// 构造器（new / with_capacity）由编译器特判展开（7 槽 + alloc_array 四数组）。
// ---------------------------------------------------------------------------
// 向上取整为 2 的幂（n <= 0 取 1）：`HashMap::with_capacity(n)` 的 cap 经此规整，
// 保证后续所有位掩码定位（hash & (cap-1)）与翻倍扩容恒成立。
pub fn next_pow2(n: i64) -> i64 {
    let mut x = n;
    if x < 1 {
        x = 1;
    }
    let mut p = 1;
    while p < x {
        p = p * 2;
    }
    p
}
