// str_ext/module.rl：String / Chars / Lines 方法扩展（2026-09-18 由 core.rl 拆分）。
// 平铺分片：由 rlyeh-driver/src/stdlib.rs 在 core 入口之后按固定顺序拼接，全名与符号顺序不变。

impl String {
    fn len(&self) -> i64 {
        self.len
    }
    fn cap(&self) -> i64 {
        self.cap
    }
    fn is_empty(&self) -> bool {
        self.len == 0
    }
    // 按字节读取（UTF-8 字符需组合字节；MVP 索引粒度 = 字节，与 s[i] 一致）
    fn get(&self, i: i64) -> i64 {
        self.data[i]
    }
    fn push_byte(&mut self, b: i64) {
        if self.len >= self.cap {
            self.grow();
        }
        self.data[self.len] = b;
        self.len = self.len + 1;
    }
    // 拼接：追加另一字符串的全部字节（other 值拷贝 3 槽共享缓冲）。
    // 先循环扩容一次到位（grow 翻倍），再执行无分支的纯字节拷贝循环——
    // clang -O3 的 loop idiom 识别会将其合成为 memcpy，避免逐字节 push_byte
    // 的每字节容量检查分支开销（strcat 类拼接基准收益显著）。
    // `a + b` 运算符在 typecheck 层 desugar 为
    // `let __s = a.clone(); __s.push_str(b); __s`）
    fn push_str(&mut self, other: String) {
        while self.cap - self.len < other.len {
            self.grow();
        }
        let mut i = 0;
        while i < other.len {
            self.data[self.len + i] = other.data[i];
            i = i + 1;
        }
        self.len = self.len + other.len;
    }
    // 快速路径：追加编译期已知字节序列（typecheck 对 `push_str(字面量实参)`
    // 特判改调本方法）。`src` 为 &str 只读视图（指向编译期全局常量），`n` 为
    // 字节数，零分配、零 String 对象构造（`push_str("ab")` 的字面量实参不再
    // 每次 alloc_bytes + copy_bytes 深拷贝——strcat 类拼接基准收益 ~3 个数量级）。
    fn push_bytes(&mut self, src: &str, n: i64) {
        while self.cap - self.len < n {
            self.grow();
        }
        let mut i = 0;
        while i < n {
            self.data[self.len + i] = src[i];
            i = i + 1;
        }
        self.len = self.len + n;
    }
    // 深拷贝：返回全新缓冲，内容与 self 相等但互不影响。
    // `a + b` 运算符 desugar 依赖此语义（A3：消除共享缓冲别名隐患——
    // 拼接结果与左操作数不再指向同一缓冲）。
    fn clone(&self) -> String {
        let mut buf = String::new();
        let mut i = 0;
        while i < self.len {
            buf.push_byte(self.data[i]);
            i = i + 1;
        }
        buf
    }
    // 子串：截取 [start, end) 字节区间（UTF-8 需调用方保证边界不切分多字节
    // 字符；MVP 索引粒度 = 字节，与 s[i] 一致）。边界越界时 clamp 到
    // [0, len]，start >= end 返回空串。返回全新缓冲，原字符串不受影响。
    fn substring(&self, start: i64, end: i64) -> String {
        let mut s = start;
        if s < 0 {
            s = 0;
        }
        if s > self.len {
            s = self.len;
        }
        let mut e = end;
        if e < 0 {
            e = 0;
        }
        if e > self.len {
            e = self.len;
        }
        let mut buf = String::new();
        let mut i = s;
        while i < e {
            buf.push_byte(self.data[i]);
            i = i + 1;
        }
        buf
    }
    // 子串查找：`self` 中首次出现 `sub` 的起始字节下标，未找到返回 -1；
    // 空子串恒返回 0。朴素滑动窗口匹配（MVP）。
    // 注意：结果经 `result` 变量返回，避免 while 块后直接跟 `-1` 被解析为减法
    fn find(&self, sub: String) -> i64 {
        let mut result = -1;
        if sub.len == 0 {
            return 0;
        }
        if sub.len > self.len {
            return -1;
        }
        let mut i = 0;
        while i + sub.len <= self.len {
            let mut j = 0;
            let mut ok = 1;
            while j < sub.len {
                if self.data[i + j] != sub.data[j] {
                    ok = 0;
                }
                j = j + 1;
            }
            if ok == 1 {
                return i;
            }
            i = i + 1;
        }
        result
    }
    // 子串包含：`self` 中是否出现 `sub`（find >= 0）；空子串恒为真。
    fn contains(&self, sub: String) -> bool {
        self.find(sub) >= 0
    }
    // 前缀判断：`self` 是否以 `prefix` 开头。prefix 长于 self 恒 false，
    // 空 prefix 恒 true（内层 while 不执行，ok 保持 1）。逐字节比较。
    fn starts_with(&self, prefix: String) -> bool {
        let mut result = false;
        if prefix.len > self.len {
            return result;
        }
        let mut ok = 1;
        let mut i = 0;
        while i < prefix.len {
            if self.data[i] != prefix.data[i] {
                ok = 0;
            }
            i = i + 1;
        }
        if ok == 1 {
            result = true;
        }
        result
    }
    // 后缀判断：`self` 是否以 `suffix` 结尾。suffix 长于 self 恒 false，
    // 空 suffix 恒 true。从 self 尾部对齐比较。
    fn ends_with(&self, suffix: String) -> bool {
        let mut result = false;
        if suffix.len > self.len {
            return result;
        }
        let mut ok = 1;
        let mut i = 0;
        while i < suffix.len {
            if self.data[self.len - suffix.len + i] != suffix.data[i] {
                ok = 0;
            }
            i = i + 1;
        }
        if ok == 1 {
            result = true;
        }
        result
    }
    // 替换：将 `self` 中所有 `old` 子串替换为 `new`，返回新缓冲。
    // 滑动窗口扫描：命中处追加 `new` 全部字节并跳过 `old.len`，否则追加原字节。
    // 空 `old` 不替换（返回自身拷贝），避免死循环。
    fn replace(&self, old: String, new: String) -> String {
        let mut buf = String::new();
        if old.len == 0 {
            let mut i = 0;
            while i < self.len {
                buf.push_byte(self.data[i]);
                i = i + 1;
            }
            return buf;
        }
        let mut i = 0;
        while i < self.len {
            if i + old.len <= self.len {
                let mut ok = 1;
                let mut j = 0;
                while j < old.len {
                    if self.data[i + j] != old.data[j] {
                        ok = 0;
                    }
                    j = j + 1;
                }
                if ok == 1 {
                    buf.push_str(new);
                    i = i + old.len;
                } else {
                    buf.push_byte(self.data[i]);
                    i = i + 1;
                }
            } else {
                buf.push_byte(self.data[i]);
                i = i + 1;
            }
        }
        buf
    }
    // 转大写：逐字节拷贝到新缓冲，ASCII 小写字母（'a'-'z' = 97-122）减 32
    // 转大写；其余字节原样。非 ASCII（UTF-8 多字节）不转换（MVP）。
    fn to_upper(&self) -> String {
        let mut buf = String::new();
        let mut i = 0;
        while i < self.len {
            let b = self.data[i];
            if 97 <= b <= 122 {
                buf.push_byte(b - 32);
            } else {
                buf.push_byte(b);
            }
            i = i + 1;
        }
        buf
    }
    // 转小写：逐字节拷贝到新缓冲，ASCII 大写字母（'A'-'Z' = 65-90）加 32
    // 转小写；其余字节原样。非 ASCII 不转换（MVP）。
    fn to_lower(&self) -> String {
        let mut buf = String::new();
        let mut i = 0;
        while i < self.len {
            let b = self.data[i];
            if 65 <= b <= 90 {
                buf.push_byte(b + 32);
            } else {
                buf.push_byte(b);
            }
            i = i + 1;
        }
        buf
    }
    // T1b：目标 API 别名 to_uppercase ≡ to_upper（ASCII 语义；Unicode 全角 / 多字节
    // 大小写转换规划——MVP 字节级，与 to_upper 一致）。
    fn to_uppercase(&self) -> String {
        self.to_upper()
    }
    fn to_lowercase(&self) -> String {
        self.to_lower()
    }
    // 裁剪：剥离首尾空白（空格 32 / 制表 9 / 换行 10 / 回车 13），返回 `&str`
    // 子区间视图（V2 零拷贝，StrFat `{ data+start, end-start }`，对齐 Rust `trim`）。
    // 正向扫描跳过开头空白找 start，反向扫描跳过结尾空白找 end，再 as_str_range；
    // 全空白串返回空视图（start 推进到 len 后 end == start → 空子区间）。
    fn trim(&self) -> &str {
        let mut start = 0;
        let mut scanning = 1;
        while scanning == 1 {
            if start < self.len {
                let b = self.data[start];
                if b == 32 || b == 9 || b == 10 || b == 13 {
                    start = start + 1;
                } else {
                    scanning = 0;
                }
            } else {
                scanning = 0;
            }
        }
        let mut end = self.len;
        let mut scanning2 = 1;
        while scanning2 == 1 {
            if end > start {
                let b = self.data[end - 1];
                if b == 32 || b == 9 || b == 10 || b == 13 {
                    end = end - 1;
                } else {
                    scanning2 = 0;
                }
            } else {
                scanning2 = 0;
            }
        }
        self.as_str_range(start, end)
    }
    // V2-B：trim_start——仅剥离开头空白，返回 `&str` 子区间视图（StrFat）。
    fn trim_start(&self) -> &str {
        let mut start = 0;
        let mut scanning = 1;
        while scanning == 1 {
            if start < self.len {
                let b = self.data[start];
                if b == 32 || b == 9 || b == 10 || b == 13 {
                    start = start + 1;
                } else {
                    scanning = 0;
                }
            } else {
                scanning = 0;
            }
        }
        self.as_str_range(start, self.len)
    }
    // V2-B：trim_end——仅剥离结尾空白，返回 `&str` 子区间视图（StrFat）。
    fn trim_end(&self) -> &str {
        let mut end = self.len;
        let mut scanning = 1;
        while scanning == 1 {
            if end > 0 {
                let b = self.data[end - 1];
                if b == 32 || b == 9 || b == 10 || b == 13 {
                    end = end - 1;
                } else {
                    scanning = 0;
                }
            } else {
                scanning = 0;
            }
        }
        self.as_str_range(0, end)
    }
    // V2：子区间视图 `&str`（StrFat 双槽 `{ data+start, end-start }`，零拷贝）。
    // typecheck 特判构造；此声明仅供 std 方法解析（body 不被使用）。
    fn as_str_range(&self, start: i64, end: i64) -> &str {
        self.as_str()
    }
    // V2（2026-08-29）：字符码点迭代器——返回 `Chars`（UTF-8 码点解码，
    // `next() -> Option<char>`）。`chars()` 即目标签名入口（此前兼容版为
    // `chars_iter()`，现二者等价，`chars_iter` 保留为别名）。
    fn chars(&self) -> Chars {
        Chars { s: self, pos: 0, len: self.len }
    }
    // V2 别名：与 `chars()` 等价（历史迭代器入口名）。
    fn chars_iter(&self) -> Chars {
        self.chars()
    }
    // V2（2026-08-29）：行迭代器——返回 `Lines`（按 \n/\r\n 分行，剥 \r；
    // `next() -> Option<String>`）。`lines()` 即目标签名入口（此前兼容版为
    // `lines_iter()`，现二者等价，`lines_iter` 保留为别名）。
    fn lines(&self) -> Lines {
        Lines { s: self, pos: 0, len: self.len }
    }
    // V2 别名：与 `lines()` 等价（历史迭代器入口名）。
    fn lines_iter(&self) -> Lines {
        self.lines()
    }
    fn grow(&mut self) {
        let new_cap = if self.cap == 0 { 8 } else { self.cap * 2 };
        let new_data = alloc_bytes(new_cap);
        copy_bytes(new_data, self.data, self.len);
        array_free(self.data);
        self.data = new_data;
        self.cap = new_cap;
    }
    // 分割：以 sep 为分隔符拆分 self 为 Vec<String>（每段均为新缓冲）。
    // 空 sep 特判返回整体自身拷贝（对齐 replace 空 old 语义，避免死循环）。
    // 连续分隔符产生空串段；尾部分隔符后也有段（可为空）。滑动窗口匹配。
    fn split(&self, sep: String) -> Vec<String> {
        let mut parts: Vec<String> = Vec::new();
        if sep.len == 0 {
            parts.push(self.substring(0, self.len));
            return parts;
        }
        let mut start = 0;
        let mut i = 0;
        while i < self.len {
            if i + sep.len <= self.len {
                let mut ok = 1;
                let mut j = 0;
                while j < sep.len {
                    if self.data[i + j] != sep.data[j] {
                        ok = 0;
                    }
                    j = j + 1;
                }
                if ok == 1 {
                    parts.push(self.substring(start, i));
                    i = i + sep.len;
                    start = i;
                } else {
                    i = i + 1;
                }
            } else {
                i = i + 1;
            }
        }
        parts.push(self.substring(start, self.len));
        parts
    }
    // 重复：将 self 拼接 count 次，返回新缓冲。count <= 0 返回空串。
    // 逐字节 push_byte 拷贝（`push_str` 参数为值 String，`&self` 无法直传，
    // 与 pad_start 拷贝循环同构）。
    fn repeat(&self, count: i64) -> String {
        let mut buf = String::new();
        let mut i = 0;
        while i < count {
            let mut j = 0;
            while j < self.len {
                buf.push_byte(self.data[j]);
                j = j + 1;
            }
            i = i + 1;
        }
        buf
    }
    // 左填充：pad 字节重复填充到总长 total（total <= len 原样拷贝）。
    // pad 为字节值（如 48 = '0'，45 = '-'），与 push_byte 一致。
    fn pad_start(&self, total: i64, pad: i64) -> String {
        let mut buf = String::new();
        let mut i = 0;
        while i + self.len < total {
            buf.push_byte(pad);
            i = i + 1;
        }
        let mut j = 0;
        while j < self.len {
            buf.push_byte(self.data[j]);
            j = j + 1;
        }
        buf
    }
    // 右填充：pad 字节重复填充到总长 total（total <= len 原样拷贝）。
    fn pad_end(&self, total: i64, pad: i64) -> String {
        let mut buf = String::new();
        let mut j = 0;
        while j < self.len {
            buf.push_byte(self.data[j]);
            j = j + 1;
        }
        let mut i = 0;
        while i + self.len < total {
            buf.push_byte(pad);
            i = i + 1;
        }
        buf
    }
    // 去除前缀：以 prefix 开头则返回去掉前缀的剩余部分（新缓冲），否则 None。
    // 逐字节窗口比较（与 split 同构）；空前缀返回自身拷贝；prefix 长于 self 返回 None。
    fn strip_prefix(&self, prefix: String) -> Option<String> {
        let mut result: Option<String> = Option::None;
        if prefix.len <= self.len {
            let mut ok = 1;
            let mut j = 0;
            while j < prefix.len {
                if self.data[j] != prefix.data[j] {
                    ok = 0;
                }
                j = j + 1;
            }
            if ok == 1 {
                result = Option::Some(self.substring(prefix.len, self.len));
            }
        }
        result
    }
    // 去除后缀：以 suffix 结尾则返回去掉后缀的剩余部分（新缓冲），否则 None。
    // 从 self.len - suffix.len 处对齐比较；空后缀返回自身拷贝；suffix 长于 self 返回 None。
    fn strip_suffix(&self, suffix: String) -> Option<String> {
        let mut result: Option<String> = Option::None;
        if suffix.len <= self.len {
            let mut ok = 1;
            let mut j = 0;
            while j < suffix.len {
                if self.data[self.len - suffix.len + j] != suffix.data[j] {
                    ok = 0;
                }
                j = j + 1;
            }
            if ok == 1 {
                result = Option::Some(self.substring(0, self.len - suffix.len));
            }
        }
        result
    }
    // 截断：返回前 new_len 字节的新缓冲。越界由 substring clamp
    //（负数 → 空串，> len → 整体拷贝），返回新缓冲不影响 self。
    fn truncate(&self, new_len: i64) -> String {
        let result = self.substring(0, new_len);
        result
    }
}

// ---------------------------------------------------------------------------
// V2：字符码点迭代器 Chars——持有原 String data 裸指针 + 游标，next() 做
// UTF-8 码点解码（首字节定宽 + 连续字节校验），返回 `Option<char>`。
// 与 `Iter<T>` 同约束：迭代期间不得对原 String 做结构性修改（指针悬垂）。
// （`struct Chars` 定义在文件前部 `struct String` 之后，保证 `impl String`
// 的方法返回类型 `Chars` 可前向解析。）
// ---------------------------------------------------------------------------
impl Chars {
    // 取下一个 UTF-8 码点并推进；耗尽返回 None。
    // 解码：首字节 b0 确定码点宽度（0-7F=1 字节 ASCII；C2-DF=2；E0-EF=3；
    // F0-F4=4），读取后续连续字节（10xxxxxx）校验并组合码点。
    // V2：`next() -> Option<char>`。Rlyeh `char` 已于 2026-08-29 拓宽为 32 位
    // Unicode 码点，可表达全部 Unicode（含多字节）；UTF-8 码点解码后直接以
    // `char` 返回（此前因 char 仅 ASCII 而退回 i64 码点值，现无需）。
    fn next(&mut self) -> Option<char> {
        if self.pos >= self.len {
            return Option::None;
        }
        let b0 = self.s.data[self.pos] as i64;
        if b0 < 0x80 {
            self.pos = self.pos + 1;
            return Option::Some(b0 as char);
        } else if b0 >= 0xE0 {
            if b0 >= 0xF0 {
                // 4 字节
                if self.pos + 3 < self.len {
                    let b1 = self.s.data[self.pos + 1] as i64;
                    let b2 = self.s.data[self.pos + 2] as i64;
                    let b3 = self.s.data[self.pos + 3] as i64;
                    let cp = ((b0 & 0x07) << 18) | ((b1 & 0x3F) << 12) | ((b2 & 0x3F) << 6) | (b3 & 0x3F);
                    self.pos = self.pos + 4;
                    return Option::Some(cp as char);
                }
            } else {
                // 3 字节
                if self.pos + 2 < self.len {
                    let b1 = self.s.data[self.pos + 1] as i64;
                    let b2 = self.s.data[self.pos + 2] as i64;
                    let cp = ((b0 & 0x0F) << 12) | ((b1 & 0x3F) << 6) | (b2 & 0x3F);
                    self.pos = self.pos + 3;
                    return Option::Some(cp as char);
                }
            }
        } else {
            // 2 字节
            if self.pos + 1 < self.len {
                let b1 = self.s.data[self.pos + 1] as i64;
                let cp = ((b0 & 0x1F) << 6) | (b1 & 0x3F);
                self.pos = self.pos + 2;
                return Option::Some(cp);
            }
        }
        // 不完整序列 / 无法解码：按单字节推进（鲁棒降级）
        self.pos = self.pos + 1;
        Option::Some(b0 as char)
    }
    fn is_empty(&self) -> bool {
        self.pos >= self.len
    }
}

// V2（2026-08-29）：Chars 实现 Iterator trait（`type Item = char` 码点），使
// `for c in s.chars()` 接入 V3 迭代器框架（目标签名 `chars() -> Chars`
// 的落地）。inherent next 优先于 trait next。
impl Chars: Iterator {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        if self.pos >= self.len {
            return Option::None;
        }
        let b0 = self.s.data[self.pos] as i64;
        if b0 < 0x80 {
            self.pos = self.pos + 1;
            return Option::Some(b0 as char);
        } else if b0 >= 0xE0 {
            if b0 >= 0xF0 {
                if self.pos + 3 < self.len {
                    let b1 = self.s.data[self.pos + 1] as i64;
                    let b2 = self.s.data[self.pos + 2] as i64;
                    let b3 = self.s.data[self.pos + 3] as i64;
                    let cp = ((b0 & 0x07) << 18) | ((b1 & 0x3F) << 12) | ((b2 & 0x3F) << 6) | (b3 & 0x3F);
                    self.pos = self.pos + 4;
                    return Option::Some(cp as char);
                }
            } else {
                if self.pos + 2 < self.len {
                    let b1 = self.s.data[self.pos + 1] as i64;
                    let b2 = self.s.data[self.pos + 2] as i64;
                    let cp = ((b0 & 0x0F) << 12) | ((b1 & 0x3F) << 6) | (b2 & 0x3F);
                    self.pos = self.pos + 3;
                    return Option::Some(cp as char);
                }
            }
        } else {
            if self.pos + 1 < self.len {
                let b1 = self.s.data[self.pos + 1] as i64;
                let cp = ((b0 & 0x1F) << 6) | (b1 & 0x3F);
                self.pos = self.pos + 2;
                return Option::Some(cp);
            }
        }
        self.pos = self.pos + 1;
        Option::Some(b0 as char)
    }
}

impl Lines {
    // 取下一行（不含换行符，`\r\n` 行尾的 `\r` 一并剥除）；EOF 返回 None。
    // 末行若无尾换行也返回；尾随换行后返回一个空行（对齐 split 语义）。
    fn next(&mut self) -> Option<String> {
        if self.pos > self.len {
            return Option::None;
        }
        // 扫描到 \n（10）
        let mut i = self.pos;
        while i < self.len {
            if self.s.data[i] == 10 {
                break;
            }
            i = i + 1;
        }
        // 行内容为 [self.pos, i)；若 i 前一个字节是 \r（13）则剥除
        let mut end = i;
        if end > self.pos && self.s.data[end - 1] == 13 {
            end = end - 1;
        }
        let mut line = String::with_capacity(end - self.pos);
        let mut j = self.pos;
        while j < end {
            line.push_byte(self.s.data[j]);
            j = j + 1;
        }
        // 推进 pos：跳过换行符（若在末尾则 pos 超过 len，下次返回 None）
        if i < self.len {
            self.pos = i + 1;
        } else {
            self.pos = self.len + 1;
        }
        Option::Some(line)
    }
}

// V2（2026-08-29）：Lines 实现 Iterator trait（`type Item = String` 行），使
// `for l in s.lines()` 接入 V3 迭代器框架（目标签名 `lines() -> Lines`
// 的落地）。inherent next 优先于 trait next。
impl Lines: Iterator {
    type Item = String;
    fn next(&mut self) -> Option<String> {
        if self.pos > self.len {
            return Option::None;
        }
        let mut i = self.pos;
        while i < self.len {
            if self.s.data[i] == 10 {
                break;
            }
            i = i + 1;
        }
        let mut end = i;
        if end > self.pos && self.s.data[end - 1] == 13 {
            end = end - 1;
        }
        let mut line = String::with_capacity(end - self.pos);
        let mut j = self.pos;
        while j < end {
            line.push_byte(self.s.data[j]);
            j = j + 1;
        }
        if i < self.len {
            self.pos = i + 1;
        } else {
            self.pos = self.len + 1;
        }
        Option::Some(line)
    }
}

