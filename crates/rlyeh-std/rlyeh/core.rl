// Rlyeh 标准库核心类型（纯 Rlyeh 实现）
//
// 状态：编译器标准库预置（2026-08-20）
// - `rlyeh build/run` 文件入口自动注入本文件（rlyeh-driver/src/stdlib.rs 定位：
//   环境变量 `RLYEH_STD_PATH` 优先，默认 <workspace>/crates/rlyeh-std/rlyeh/core.rl）。
// - CLI `--no-std` 可禁用注入；字符串 API（run_source 等）不注入。
// - 测试内联副本见 rlyeh-driver/tests/std_test.rs 的 CORE_TYPES。
//
// 已知语言限制（见 CODEBUDDY.md / prompts/README.md）：
// - 无 `panic`：`loop {}` 充当永不返回（Never）的崩溃替代；
// - `Option::None` 的泛型参数以 `_`（Infer）占位，由使用上下文推断。

// String 结构体前移声明：`Option::expect`/`Result::expect` 的 `msg: String`
// 参数需要类型在使用前定义（类型符号顺序解析，无前向引用）。

// 动态字符串（UTF-8 字节缓冲，按字符/字节索引步长 1）
// 布局与 Vec 相同：槽 0 = data 指针（[u8; 0] 动态缓冲），槽 1 = len，槽 2 = cap
// 构造器（new / with_capacity / from）由编译器特判展开（alloc_bytes / copy_bytes）
struct String {
    data: [u8; 0],
    len: i64,
    cap: i64,
}

// V2：字符码点迭代器（持有原 String 引用 + 游标 + 长度）。
// 用 `&String` 引用 + `self.s.data[pos]` 按字节索引（Rlyeh 裸指针 `*const u8`
// 按 i64 元素语义解引用，无法逐字节访问）。
struct Chars {
    s: &String,
    pos: i64,
    len: i64,
}

// V2：行迭代器（持有原 String 引用 + 游标）。`next() -> Option<String>` 按
// `\n`（10）分行，`\r\n`（13,10）行尾的 `\r` 一并剥除（改进 MVP 字节级保留）。
struct Lines {
    s: &String,
    pos: i64,
    len: i64,
}

enum Option<T> {
    None,
    Some(T),
}

impl<T> Option<T> {
    fn is_some(self) -> i64 {
        match self {
            Option::Some(v) => 1,
            Option::None => 0,
        }
    }
    fn is_none(self) -> i64 {
        match self {
            Option::Some(v) => 0,
            Option::None => 1,
        }
    }
    fn unwrap(self) -> T {
        match self {
            Option::Some(v) => v,
            Option::None => loop {},
        }
    }
    // 默认值：Some 返回 v；None 返回 default。
    // 注意：MVP 已知 codegen 限制——String（聚合载荷）实例化下 match 分支
    // 返回 default 会得到损坏值（枚举载荷以 Ptr 存储的返回路径 bug），
    // 仅标量（i64/f64）实例化可靠；String 场景请用 expect。
    fn unwrap_or(self, default: T) -> T {
        match self {
            Option::Some(v) => v,
            Option::None => default,
        }
    }
    // 期望值：Some 返回 v；None 以 `loop {}` 充当崩溃替代（MVP 无 panic
    // 机制，消息参数保留对齐 Rust 语义，当前不打印）。
    fn expect(self, msg: String) -> T {
        match self {
            Option::Some(v) => v,
            Option::None => loop {},
        }
    }
}

enum Result<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E> Result<T, E> {
    fn is_ok(self) -> i64 {
        match self {
            Result::Ok(v) => 1,
            Result::Err(e) => 0,
        }
    }
    fn unwrap(self) -> T {
        match self {
            Result::Ok(v) => v,
            Result::Err(e) => loop {},
        }
    }
    // 是否错误：Ok → 0，Err → 1（与 is_ok 互补）。
    fn is_err(self) -> i64 {
        match self {
            Result::Ok(v) => 0,
            Result::Err(e) => 1,
        }
    }
    // 默认值：Ok 返回 v；Err 返回 default（Err 载荷被丢弃）。
    // 同 Option::unwrap_or：String（聚合载荷）实例化受 codegen 返回路径限制，
    // 仅标量实例化可靠；String 场景请用 expect。
    fn unwrap_or(self, default: T) -> T {
        match self {
            Result::Ok(v) => v,
            Result::Err(e) => default,
        }
    }
    // 期望值：Ok 返回 v；Err 以 `loop {}` 充当崩溃替代（消息预留打印）。
    fn expect(self, msg: String) -> T {
        match self {
            Result::Ok(v) => v,
            Result::Err(e) => loop {},
        }
    }
}

// 动态数组（Vec<T>）
//
// - `data` 字段类型 `[T; 0]`：约定长度 0 = 运行时长度（动态数组指针）
//   （数组索引步长固定 8 字节，与 `[T; N]` 定长数组共用同一套内存布局）。
// - `Vec::with_capacity(n)` / `Vec::new()` 由编译器特判展开
//   （泛型 impl 静态方法 MVP 不支持），返回 `Vec<_>` 由上下文统一类型参数。
// - `grow()` 翻倍扩容：`alloc_array` 新分配 + `array_copy` 整块拷贝（槽区共享语义）。

struct Vec<T> {
    data: [T; 0],
    len: i64,
    cap: i64,
}

impl<T> Vec<T> {
    fn len(self) -> i64 {
        self.len
    }
    fn cap(self) -> i64 {
        self.cap
    }
    fn is_empty(self) -> i64 {
        if self.len == 0 {
            1
        } else {
            0
        }
    }
    fn get(self, i: i64) -> T {
        self.data[i]
    }
    fn set(&mut self, i: i64, x: T) {
        self.data[i] = x;
    }
    fn push(&mut self, x: T) {
        if self.len >= self.cap {
            self.grow();
        }
        self.data[self.len] = x;
        self.len = self.len + 1;
    }
    fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            Option::None
        } else {
            self.len = self.len - 1;
            let v = self.data[self.len];
            Option::Some(v)
        }
    }
    fn grow(&mut self) {
        let new_cap = self.cap * 2;
        let new_data = alloc_array(new_cap);
        array_copy(new_data, self.data, self.len);
        array_free(self.data);
        self.data = new_data;
        self.cap = new_cap;
    }
    // 包含判断：是否存在元素与 `x` 相等（`==` 在泛型实例化后按具体类型比较：
    // i64 数值相等 / String 内容相等）。命中即置位标志，O(n) 遍历。
    fn contains(&self, x: T) -> bool {
        let mut result = false;
        let mut i = 0;
        while i < self.len {
            if self.data[i] == x {
                result = true;
            }
            i = i + 1;
        }
        result
    }
    // 删除并返回第 i 个元素：其后元素逐位前移（浅拷贝槽），len -= 1。
    // 越界（i >= len）MVP 无 panic，以 `loop {}` 充当崩溃替代。
    fn remove(&mut self, i: i64) -> T {
        if i >= self.len {
            loop {}
        }
        let v = self.data[i];
        let mut j = i;
        while j + 1 < self.len {
            self.data[j] = self.data[j + 1];
            j = j + 1;
        }
        self.len = self.len - 1;
        v
    }
    // 在第 i 位前插入 x：从尾部反向搬移元素（避免正向覆盖），len += 1。
    // 容量不足先扩容。i 越界（> len）行为未定义，调用方保证合法。
    fn insert(&mut self, i: i64, x: T) {
        if self.len >= self.cap {
            self.grow();
        }
        let mut j = self.len;
        while j > i {
            self.data[j] = self.data[j - 1];
            j = j - 1;
        }
        self.data[i] = x;
        self.len = self.len + 1;
    }
    // 清空：仅重置 len，槽区残留数据被后续 push 覆盖写，无需释放。
    fn clear(&mut self) {
        self.len = 0;
    }
    // 查找：返回第一个与 `x` 相等的元素下标，未命中返回 -1（对齐 String::find）。
    // result 变量模式返回；命中即 `i = self.len` 提前终止，保住首个下标。
    fn find(&self, x: T) -> i64 {
        let mut result = -1;
        let mut i = 0;
        while i < self.len {
            if self.data[i] == x {
                result = i;
                i = self.len;
            }
            i = i + 1;
        }
        result
    }
    // 排序：原地堆排序（升序，O(n log n)），`<` 在实例化后按具体类型比较
    // （i64 数值 / String 字典序）。非稳定排序；原地 O(1) 额外空间。
    // 算法：构建最大堆（从最后一个非叶子节点逐个下沉），再依次将堆顶
    // 交换到末尾并缩小堆（每次交换后对根重新下沉）。
    fn sort(&mut self) {
        let n = self.len;
        // 构建最大堆
        let mut start = n / 2 - 1;
        while start >= 0 {
            let mut root = start;
            loop {
                let mut child = root * 2 + 1;
                if child >= n { break; }
                let mut cv = self.data[child];
                if child + 1 < n {
                    let cv2 = self.data[child + 1];
                    if cv < cv2 {
                        child = child + 1;
                        cv = cv2;
                    }
                }
                if self.data[root] >= cv { break; }
                let t = self.data[root];
                self.data[root] = self.data[child];
                self.data[child] = t;
                root = child;
            }
            start = start - 1;
        }
        // 依次提取堆顶到已排序区
        let mut end = n - 1;
        while end > 0 {
            let t = self.data[0];
            self.data[0] = self.data[end];
            self.data[end] = t;
            let mut root = 0;
            loop {
                let mut child = root * 2 + 1;
                if child >= end { break; }
                let mut cv = self.data[child];
                if child + 1 < end {
                    let cv2 = self.data[child + 1];
                    if cv < cv2 {
                        child = child + 1;
                        cv = cv2;
                    }
                }
                if self.data[root] >= cv { break; }
                let t = self.data[root];
                self.data[root] = self.data[child];
                self.data[child] = t;
                root = child;
            }
            end = end - 1;
        }
    }
    // 首元素：空容器返回 None，否则返回槽 0 的值拷贝。
    fn first(&self) -> Option<T> {
        if self.len == 0 {
            Option::None
        } else {
            Option::Some(self.data[0])
        }
    }
    // 末元素：空容器返回 None，否则返回槽 len-1 的值拷贝。
    fn last(&self) -> Option<T> {
        if self.len == 0 {
            Option::None
        } else {
            Option::Some(self.data[self.len - 1])
        }
    }
    // 原地翻转：双指针从两端交换，O(n)。
    fn reverse(&mut self) {
        let mut i = 0;
        let mut j = self.len - 1;
        while i < j {
            let t = self.data[i];
            self.data[i] = self.data[j];
            self.data[j] = t;
            i = i + 1;
            j = j - 1;
        }
    }
    // 交换下标 i、j 的元素（越界以 `loop {}` 充当崩溃替代，MVP 无 panic）。
    fn swap(&mut self, i: i64, j: i64) {
        if i >= self.len {
            loop {}
        }
        if j >= self.len {
            loop {}
        }
        let t = self.data[i];
        self.data[i] = self.data[j];
        self.data[j] = t;
    }
    // 二分查找：要求已升序排列，返回 `x` 的下标，未命中返回 -1。
    // `self.data[mid] < x` 与 `x < self.data[mid]` 双比较区分三态，
    // `<` 在实例化后按具体类型比较（i64 数值 / String 字典序）。
    fn binary_search(&self, x: T) -> i64 {
        let mut result = -1;
        let mut lo = 0;
        let mut hi = self.len - 1;
        while lo <= hi {
            let mid = (lo + hi) / 2;
            if self.data[mid] < x {
                lo = mid + 1;
            } else if x < self.data[mid] {
                hi = mid - 1;
            } else {
                result = mid;
                hi = mid - 1;
            }
        }
        result
    }
    // 子切片：截取 [start, end) 区间元素（半开区间），返回全新缓冲，
    // 原 Vec 不受影响（元素按值拷贝）。
    // 边界越界 clamp 到 [0, len]；start >= end 返回空 Vec。
    // `v[lo..<hi]` / `v[lo...hi]` / `v[lo<..hi]` 在 typecheck 层 desugar 为此方法。
    fn slice(&self, start: i64, end: i64) -> Vec<T> {
        let mut sl: Vec<T> = Vec::new();
        let mut s = start;
        let mut e = end;
        if s < 0 {
            s = 0;
        }
        if s > self.len {
            s = self.len;
        }
        if e < 0 {
            e = 0;
        }
        if e > self.len {
            e = self.len;
        }
        while s < e {
            sl.push(self.data[s]);
            s = s + 1;
        }
        sl
    }
    // V1：瘦指针迭代器（2026-08）——返回 `Iter<T>` 零分配零拷贝迭代视图：data
    // 为首元素 `*const T` 裸指针（V1 `&arr[i]` 真实 GEP 地址支持），len 为剩余
    // 元素数（非缓冲长度）。元素按值拷贝读取（MVP 无 `Option<&T>` 引用返回——
    // 借用迭代器需生命周期标注的迭代状态类型，规划中）。接入 for 循环（检测
    // next() -> Option<T>）与 J3 适配器（map/filter/fold 等检测 next() 方法）。
    // 注意：迭代器持有原缓冲裸指针，迭代期间不得对 Vec 做结构性修改（push /
    // insert / remove 触发扩容重新分配会使指针悬垂）。
    fn iter(&self) -> Iter<T> {
        let p: *const T = &self.data[0];
        Iter::new(p, self.len)
    }
    // V1：可变瘦指针迭代器——返回 `IterMut<T>`（*mut T + 写回目标 cur + 剩余
    // 长度）。next() 返回元素值拷贝并推进；write(x) 经 DerefSet 写回"最近 next
    // 读取的元素"（真实原槽写回，非拷贝）。MVP 无 `Option<&mut T>` 引用返回
    // （规划），for 迭代中修改元素不写回。
    fn iter_mut(&mut self) -> IterMut<T> {
        let p: *mut T = &mut self.data[0];
        IterMut::new(p, p, self.len)
    }
    // V4：可变索引访问——`Option<&mut T>` 引用语义：越界返回 None，命中返回对原槽的
    // 可变引用，经 `match { Some(r) => *r = x }` 写回真实槽（非拷贝）。
    fn get_mut(&mut self, i: i64) -> Option<&mut T> {
        if i >= 0 && i < self.len {
            Option::Some(&mut self.data[i])
        } else {
            Option::None
        }
    }
    // T1a：比较器排序——cmp 返回三态 i64（负 / 零 / 正，对齐 Rust Ordering 语义，
    // Ordering 枚举规划；< 0 表示第一个参数应在前）。原地堆排序 O(n log n)，
    // 非稳定；比较器经函数指针调用（H1），H2 无捕获闭包可作实参。
    // 算法与 sort 相同（最大堆 + 堆顶交换），仅比较改用 cmp 三态判定。
    fn sort_by(&mut self, cmp: fn(T, T) -> i64) {
        let n = self.len;
        // 构建最大堆
        let mut start = n / 2 - 1;
        while start >= 0 {
            let mut root = start;
            loop {
                let mut child = root * 2 + 1;
                if child >= n { break; }
                let mut cv = self.data[child];
                if child + 1 < n {
                    let cv2 = self.data[child + 1];
                    if cmp(cv, cv2) < 0 {
                        child = child + 1;
                        cv = cv2;
                    }
                }
                if cmp(self.data[root], cv) >= 0 { break; }
                let t = self.data[root];
                self.data[root] = self.data[child];
                self.data[child] = t;
                root = child;
            }
            start = start - 1;
        }
        // 依次提取堆顶到已排序区
        let mut end = n - 1;
        while end > 0 {
            let t = self.data[0];
            self.data[0] = self.data[end];
            self.data[end] = t;
            let mut root = 0;
            loop {
                let mut child = root * 2 + 1;
                if child >= end { break; }
                let mut cv = self.data[child];
                if child + 1 < end {
                    let cv2 = self.data[child + 1];
                    if cmp(cv, cv2) < 0 {
                        child = child + 1;
                        cv = cv2;
                    }
                }
                if cmp(self.data[root], cv) >= 0 { break; }
                let t = self.data[root];
                self.data[root] = self.data[child];
                self.data[child] = t;
                root = child;
            }
            end = end - 1;
        }
    }
}

// ---------------------------------------------------------------------------
// V1 瘦指针迭代器（2026-08）：`Iter<T>` / `IterMut<T>` 零分配零拷贝迭代视图。
// 布局：data = 首元素裸指针（*const T / *mut T，V1 `&arr[i]` 真实 GEP 地址），
// len = 剩余元素数。元素按值拷贝读取（MVP 无 `Option<&T>` / `Option<&mut T>`
// 引用返回——借用迭代器需生命周期标注的迭代状态类型，规划中）。
// 接入：for 循环（check_for_iterator 检测 next() -> Option<T>）+ J3 适配器
// （map/filter/fold/collect/take/skip 检测 next() 方法 + Option 内项类型）。
// 约束：持有原缓冲裸指针，迭代期间不得对 Vec 做结构性修改（扩容 realloc 后
// 指针悬垂）。裸指针无生命周期关联，drop 不释放原缓冲（视图语义）。
// ---------------------------------------------------------------------------

struct Iter<T> {
    data: *const T,
    len: i64,
}

impl<T> Iter<T> {
    // 读当前元素（值拷贝）并推进：空迭代器返回 None。
    fn next(&mut self) -> Option<T> {
        if self.len == 0 {
            return Option::None;
        }
        let v = *self.data;
        self.data = self.data + 1;
        self.len = self.len - 1;
        Option::Some(v)
    }
    fn len(&self) -> i64 {
        self.len
    }
    fn is_empty(&self) -> bool {
        self.len == 0
    }
}

// V3-D2（2026-08-27）：Iter<T> 实现 Iterator trait（type Item = T），使其
// 能调用迁移后的惰性适配器默认方法（map/filter/take 等）。inherent next
// 优先于 trait next（方法解析），trait next 供 Iterator 语义/默认方法使用。
impl<T> Iterator for Iter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.len == 0 {
            return Option::None;
        }
        let v = *self.data;
        self.data = self.data + 1;
        self.len = self.len - 1;
        Option::Some(v)
    }
}

struct IterMut<T> {
    data: *mut T,   // 下一个待读元素地址
    cur: *mut T,    // 最近 next 读取的元素地址（write 写回目标）
    len: i64,       // 剩余元素数
}

impl<T> IterMut<T> {
    // 读当前元素（值拷贝）并推进：空迭代器返回 None。
    fn next(&mut self) -> Option<T> {
        if self.len == 0 {
            return Option::None;
        }
        let v = *self.data;
        self.cur = self.data;
        self.data = self.data + 1;
        self.len = self.len - 1;
        Option::Some(v)
    }
    // 写回最近 next 读取的元素（DerefSet 真实原槽写回）。next 未调用时写首元素。
    fn write(&mut self, x: T) {
        *self.cur = x;
    }
    fn len(&self) -> i64 {
        self.len
    }
    fn is_empty(&self) -> bool {
        self.len == 0
    }
}

// V3-D2（2026-08-27）：IterMut<T> 实现 Iterator trait（type Item = T）。
impl<T> Iterator for IterMut<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.len == 0 {
            return Option::None;
        }
        let v = *self.data;
        self.cur = self.data;
        self.data = self.data + 1;
        self.len = self.len - 1;
        Option::Some(v)
    }
}

// T2：Iterator trait（V3-A3，2026-08-27：引入 `type Item` 关联类型替代固定 i64，
// `next` 返回 `Option<Self::Item>`）。自定义迭代器经
// `impl Iterator for T { type Item = i64; fn next(&mut self) -> Option<i64> }`
// 接入 for 循环（check_for_iterator 检测 next() 方法，inherent 或 trait impl 均可）。
// 泛型元素迭代器（如 StdinLines 返回 Option<String>）仍走方法式接入。
trait Iterator {
    // 元素类型（V3-A3）：impl 提供 `type Item = <具体类型>`
    type Item;

    fn next(&mut self) -> Option<Self::Item>;

    // ===== V3 默认方法：基于 next() 的实现，impl 未显式实现时回退
    // （typecheck trait 默认方法机制）。MVP 默认方法仍按 i64 元素实现
    // （count/sum 数值累加、any/all 谓词），元素 i64 时与 `Self::Item` 一致。=====

    // 迭代器元素个数（耗尽剩余元素）
    fn count(&mut self) -> i64 {
        let mut n = 0;
        loop {
            match self.next() {
                Option::Some(_) => n = n + 1,
                Option::None => break,
            }
        }
        n
    }

    // 元素求和（i64）
    fn sum(&mut self) -> i64 {
        let mut s = 0;
        loop {
            match self.next() {
                Option::Some(v) => s = s + v,
                Option::None => break,
            }
        }
        s
    }

    // 任一元素满足谓词则 true（短路，遇 true 即返回）
    fn any(&mut self, pred: fn(i64) -> bool) -> bool {
        loop {
            match self.next() {
                Option::Some(v) => {
                    if pred(v) {
                        return true;
                    }
                }
                Option::None => return false,
            }
        }
    }

    // 所有元素满足谓词则 true（短路，遇 false 即返回）
    fn all(&mut self, pred: fn(i64) -> bool) -> bool {
        loop {
            match self.next() {
                Option::Some(v) => {
                    if !pred(v) {
                        return false;
                    }
                }
                Option::None => return true,
            }
        }
    }

    // V3-B 默认方法（2026-08-27）：find / fold（MVP 元素 i64，基于 next() 实现）。
    // `chain`/`enumerate` 需要消耗 `self` 泛型包装（`Chain<Self, U>`/`Enumerate<Self>`），
    // 待 trait 默认方法支持消耗式 `self` 后补（见 v3-b 叶子）。

    // 返回首个满足谓词的元素（未找到返回 -1；MVP 元素 i64 简化）
    fn find(&mut self, pred: fn(i64) -> bool) -> i64 {
        loop {
            match self.next() {
                Option::Some(v) => {
                    if pred(v) {
                        return v;
                    }
                }
                Option::None => return -1,
            }
        }
    }

    // 归约累加：`acc = f(acc, elem)` 逐元素折叠（MVP 累加器 i64）
    fn fold(&mut self, init: i64, f: fn(i64, i64) -> i64) -> i64 {
        let mut acc = init;
        loop {
            match self.next() {
                Option::Some(v) => acc = f(acc, v),
                Option::None => return acc,
            }
        }
    }

    // ===== V3-D1 惰性适配器（2026-08-27）：消耗 self 返回包装迭代器 =====
    // 依赖 V3-C 的 Filter/Take 包装 + V3-D 语言增强（默认方法返回泛型包装时
    // `Self` 实例化）。MVP 元素 i64。

    // filter：返回 Filter 包装（跳过不满足谓词的元素）
    fn filter(self, pred: fn(i64) -> bool) -> Filter<Self> {
        Filter<Self> { inner: self, pred: pred }
    }

    // take：返回 Take 包装（取前 n 个元素）
    fn take(self, n: i64) -> Take<Self> {
        Take<Self> { inner: self, remaining: n }
    }

    // skip：返回 Skip 包装（跳过前 n 个元素）
    fn skip(self, n: i64) -> Skip<Self> {
        Skip<Self> { inner: self, to_skip: n }
    }

    // collect：消耗迭代器并收集全部元素到 Vec<i64>（V3-D2）
    fn collect(self) -> Vec<i64> {
        let mut v: Vec<i64> = Vec::new();
        let mut s = self;
        loop {
            match s.next() {
                Option::Some(x) => v.push(x),
                Option::None => break,
            }
        }
        v
    }

    // map：消耗 self 返回 Map 包装（对元素应用变换函数）。
    // MVP：变换返回 i64（`Map<Self, i64>`）；方法级泛型 `<B>` 在默认方法中
    // 受限（见 v3-d2 叶子），B 固定 i64。
    fn map(self, f: fn(i64) -> i64) -> Map<Self, i64> {
        Map<Self, i64> { inner: self, f: f }
    }

    // enumerate：消耗 self 返回 Enumerate 包装（产出递增序号）。
    // 依赖 V3-C 的 Enumerate<I>（MVP 元素 i64，序号即元素索引）。
    fn enumerate(self) -> Enumerate<Self> {
        Enumerate<Self> { inner: self, idx: 0 }
    }

    // chain：消耗 self 与另一同类型迭代器，返回 Chain 包装（前者耗尽转后者）。
    // MVP：`other` 类型与 `Self` 相同（`Chain<Self, Self>`）；方法级泛型 `<U>`
    // 接异类型受限（见 v3-b 叶子）。
    fn chain(self, other: Self) -> Chain<Self, Self> {
        Chain<Self, Self> { a: self, b: other, on_a: true }
    }
}

// ===== V3-C 包装迭代器（2026-08-27）：惰性适配器基础设施 =====
// 泛型包装迭代器持底层迭代器 `I` + 参数槽，`next()` 实现变换逻辑。
// MVP 元素固定 i64（`type Item = i64`，与 V3-A3 默认方法一致）；泛型 `I::Item`
// 投影传播待 `where I: Iterator` 泛型约束完善后泛化。

// Filter<I>：跳过不满足谓词的元素
struct Filter<I> {
    inner: I,
    pred: fn(i64) -> bool,
}

impl<I> Iterator for Filter<I> {
    type Item = i64;
    fn next(&mut self) -> Option<i64> {
        loop {
            match self.inner.next() {
                Option::Some(v) => {
                    let p = self.pred;
                    if p(v) {
                        return Option::Some(v);
                    }
                }
                Option::None => return Option::None,
            }
        }
    }
}

// Take<I>：取前 n 个元素（计数归零返回 None）
struct Take<I> {
    inner: I,
    remaining: i64,
}

impl<I> Iterator for Take<I> {
    type Item = i64;
    fn next(&mut self) -> Option<i64> {
        if self.remaining <= 0 {
            return Option::None;
        }
        match self.inner.next() {
            Option::Some(v) => {
                self.remaining = self.remaining - 1;
                Option::Some(v)
            }
            Option::None => Option::None,
        }
    }
}

// Skip<I>：跳过前 n 个元素后再产出
struct Skip<I> {
    inner: I,
    to_skip: i64,
}

impl<I> Iterator for Skip<I> {
    type Item = i64;
    fn next(&mut self) -> Option<i64> {
        while self.to_skip > 0 {
            match self.inner.next() {
                Option::Some(_) => {
                    self.to_skip = self.to_skip - 1;
                }
                Option::None => return Option::None,
            }
        }
        self.inner.next()
    }
}

// Chain<A, B>：前迭代器耗尽后转后迭代器
struct Chain<A, B> {
    a: A,
    b: B,
    on_a: bool,
}

impl<A, B> Iterator for Chain<A, B> {
    type Item = i64;
    fn next(&mut self) -> Option<i64> {
        if self.on_a {
            match self.a.next() {
                Option::Some(v) => return Option::Some(v),
                Option::None => {
                    self.on_a = false;
                }
            }
        }
        self.b.next()
    }
}

// Enumerate<I>：产出递增序号（MVP 元素 i64，序号即元素索引）
struct Enumerate<I> {
    inner: I,
    idx: i64,
}

impl<I> Iterator for Enumerate<I> {
    type Item = i64;
    fn next(&mut self) -> Option<i64> {
        match self.inner.next() {
            Option::Some(_) => {
                let i = self.idx;
                self.idx = self.idx + 1;
                Option::Some(i)
            }
            Option::None => Option::None,
        }
    }
}

// Map<I, B>：对底层迭代器元素应用变换函数（V3-D2）。
// MVP：变换函数 `fn(i64) -> B`，`type Item = B`；map 默认方法按 B=i64 实例化
//（方法级泛型 `<B>` 在默认方法中受限于类型系统，见 v3-d2 叶子）。
struct Map<I, B> {
    inner: I,
    f: fn(i64) -> B,
}

impl<I, B> Iterator for Map<I, B> {
    type Item = B;
    fn next(&mut self) -> Option<B> {
        let f = self.f;
        match self.inner.next() {
            Option::Some(v) => Option::Some(f(v)),
            Option::None => Option::None,
        }
    }
}

// ---------------------------------------------------------------------------
// String：动态字符串（UTF-8 字节缓冲，按字符/字节索引步长 1）
// 布局与 Vec 相同：槽 0 = data 指针（[u8; 0] 动态缓冲），槽 1 = len，槽 2 = cap
// 构造器（new / with_capacity / from）由编译器特判展开（alloc_bytes / copy_bytes）
// （struct 定义已前移文件顶部，供 Option/Result 的 expect 引用）
// ---------------------------------------------------------------------------

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
    // T1b：字符列表——MVP 字节级：逐字节返回（字符 = 字节，与 to_upper / 索引
    // 步长 1 字节一致；目标 `Chars` 迭代器 + UTF-8 码点解码规划）。
    fn chars(&self) -> Vec<i64> {
        let mut cs: Vec<i64> = Vec::new();
        let mut i = 0;
        while i < self.len {
            cs.push(self.data[i]);
            i = i + 1;
        }
        cs
    }
    // V2：字符码点迭代器——返回 `Chars`（UTF-8 码点解码，`next() -> Option<char>`）。
    // 保留 `chars()`（字节级 Vec<i64>）兼容；`chars_iter` 为码点级迭代器。
    fn chars_iter(&self) -> Chars {
        Chars { s: self, pos: 0, len: self.len }
    }
    // T1b：行切分——按换行符（\n = 10）切分，返回 Vec<String>（复用 split 语义，
    // 连续换行产生空行段、尾随换行后有尾空行段）。目标 `Lines` 迭代器规划；
    // MVP 差异：\r\n 行尾的 \r 保留（字节语义，未剥除）。
    fn lines(&self) -> Vec<String> {
        let result = self.split("\n");
        result
    }
    // V2：行迭代器——返回 `Lines`（按 \n/\r\n 分行，剥 \r；next() -> Option<String>）。
    // 保留 `lines()`（Vec<String>）兼容；`lines_iter` 为惰性迭代器。
    fn lines_iter(&self) -> Lines {
        Lines { s: self, pos: 0, len: self.len }
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
    // V2：`next() -> Option<i64>`（码点值）。返回 UTF-8 码点数值而非 `char`，
    // 因 Rlyeh `char` 类型 codegen 仅支持 ASCII（非 ASCII `as char` 报错），
    // 用 `i64` 码点值可表达全部 Unicode 码点（含多字节）。
    fn next(&mut self) -> Option<i64> {
        if self.pos >= self.len {
            return Option::None;
        }
        let b0 = self.s.data[self.pos] as i64;
        if b0 < 0x80 {
            self.pos = self.pos + 1;
            return Option::Some(b0);
        } else if b0 >= 0xE0 {
            if b0 >= 0xF0 {
                // 4 字节
                if self.pos + 3 < self.len {
                    let b1 = self.s.data[self.pos + 1] as i64;
                    let b2 = self.s.data[self.pos + 2] as i64;
                    let b3 = self.s.data[self.pos + 3] as i64;
                    let cp = ((b0 & 0x07) << 18) | ((b1 & 0x3F) << 12) | ((b2 & 0x3F) << 6) | (b3 & 0x3F);
                    self.pos = self.pos + 4;
                    return Option::Some(cp);
                }
            } else {
                // 3 字节
                if self.pos + 2 < self.len {
                    let b1 = self.s.data[self.pos + 1] as i64;
                    let b2 = self.s.data[self.pos + 2] as i64;
                    let cp = ((b0 & 0x0F) << 12) | ((b1 & 0x3F) << 6) | (b2 & 0x3F);
                    self.pos = self.pos + 3;
                    return Option::Some(cp);
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
        Option::Some(b0)
    }
    fn is_empty(&self) -> bool {
        self.pos >= self.len
    }
}

// V2（2026-08-27）：Chars 实现 Iterator trait（`type Item = i64` 码点），使
// `for c in s.chars_iter()` 接入 V3 迭代器框架（目标签名 `chars() -> Chars`
// 的基础）。inherent next 优先于 trait next。
impl Iterator for Chars {
    type Item = i64;
    fn next(&mut self) -> Option<i64> {
        if self.pos >= self.len {
            return Option::None;
        }
        let b0 = self.s.data[self.pos] as i64;
        if b0 < 0x80 {
            self.pos = self.pos + 1;
            return Option::Some(b0);
        } else if b0 >= 0xE0 {
            if b0 >= 0xF0 {
                if self.pos + 3 < self.len {
                    let b1 = self.s.data[self.pos + 1] as i64;
                    let b2 = self.s.data[self.pos + 2] as i64;
                    let b3 = self.s.data[self.pos + 3] as i64;
                    let cp = ((b0 & 0x07) << 18) | ((b1 & 0x3F) << 12) | ((b2 & 0x3F) << 6) | (b3 & 0x3F);
                    self.pos = self.pos + 4;
                    return Option::Some(cp);
                }
            } else {
                if self.pos + 2 < self.len {
                    let b1 = self.s.data[self.pos + 1] as i64;
                    let b2 = self.s.data[self.pos + 2] as i64;
                    let cp = ((b0 & 0x0F) << 12) | ((b1 & 0x3F) << 6) | (b2 & 0x3F);
                    self.pos = self.pos + 3;
                    return Option::Some(cp);
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
        Option::Some(b0)
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

// V2（2026-08-27）：Lines 实现 Iterator trait（`type Item = String` 行），使
// `for l in s.lines_iter()` 接入 V3 迭代器框架（目标签名 `lines() -> Lines`
// 的基础）。inherent next 优先于 trait next。
impl Iterator for Lines {
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

// X2（2026-08-27）：引号感知分段——按分隔符分割，但跳过双引号字符串内的分隔符
//（值含逗号的 TOML 内联表/数组）。返回段（含原空格，调用方自行 trim）。
fn split_quoted(s: String, delim: i64) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut start = 0;
    let mut in_str = false;
    let mut i = 0;
    while i < s.len {
        let c = s.get(i);
        if c == 34 {
            // 双引号切换字符串状态
            in_str = !in_str;
        } else if c == delim && !in_str {
            let part = s.substring(start, i);
            parts.push(part);
            start = i + 1;
        }
        i = i + 1;
    }
    let last = s.substring(start, s.len);
    parts.push(last);
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

struct HashMap<K, V> {
    keys: [K; 0],
    vals: [V; 0],
    states: [i64; 0],
    len: i64,
    used: i64,
    cap: i64,
    dist: [i64; 0],
}

impl<K, V> HashMap<K, V> {
    // Robin Hood 线性探测查找：命中返回槽位索引，未找到返回 -1。
    // 提前终止双条件：遇空槽（s == 0）→ 不存在；遇槽内键探测距离 < 当前探测步数
    // → 不存在（Robin Hood 距离不变量：链上键距离非减，更远处不可能有本键）。
    // 槽内键距离预存于 dist 数组（insert/grow 维护，O(1) 读取，String 键零额外哈希）。
    // 墓碑（s == 2）跳过不终止——删除不破坏距离不变量（见 insert）。
    fn find(&self, k: K) -> i64 {
        let mask = self.cap - 1;
        let mut idx = hash_value(k) & mask;
        let mut d = 0;
        while d < self.cap {
            let s = self.states[idx];
            if s == 0 {
                return -1;
            }
            if s == 1 {
                if self.dist[idx] < d {
                    return -1;
                }
                if self.keys[idx] == k {
                    return idx;
                }
            }
            idx = (idx + 1) & mask;
            d = d + 1;
        }
        -1
    }
    fn len(&self) -> i64 {
        self.len
    }
    fn cap(&self) -> i64 {
        self.cap
    }
    fn is_empty(&self) -> bool {
        self.len == 0
    }
    fn contains_key(&self, k: K) -> bool {
        self.find(k) >= 0
    }
    fn get(&self, k: K) -> Option<V> {
        let idx = self.find(k);
        if idx < 0 {
            Option::None
        } else {
            Option::Some(self.vals[idx])
        }
    }
    // Robin Hood 插入：单次探测同时完成「查键」与「找插入位」，遇空槽即确认键不存在。
    // 距离均衡——探测中若槽内键距离 < 当前键已走距离（在位者更富/离家更近），则
    // 在位者让位、当前键就位、被挤出的键从下一位置继续探测（穷者靠近家，链上距离
    // 保持非减，支撑 find 的提前终止）。键距离写入 dist 数组（O(1)，String 键零额外
    // 哈希——与 i64 键同构）。
    // 墓碑（s == 2）跳过不占用（不参与交换、不作为插入位）：新键始终落真空槽，
    // 距离不变量与无删除时一致；墓碑堆积由负载因子触发的 grow 重哈希清除（丢弃）。
    // 负载因子 7/8（Robin Hood 高负载下距离均衡 + 提前终止控探测）——表小一半、
    // 扩容重哈希总量减半（vs 旧 1/2 负载）。
    fn insert(&mut self, k: K, v: V) {
        if self.used * 8 >= self.cap * 7 {
            self.grow();
        }
        let mask = self.cap - 1;
        let mut cur_k = k;
        let mut cur_v = v;
        let mut idx = hash_value(cur_k) & mask;
        let mut d = 0;
        while d < self.cap {
            let s = self.states[idx];
            if s == 0 {
                self.used = self.used + 1;
                self.keys[idx] = cur_k;
                self.vals[idx] = cur_v;
                self.states[idx] = 1;
                self.dist[idx] = d;
                self.len = self.len + 1;
                return;
            }
            if s == 1 && self.keys[idx] == cur_k {
                self.vals[idx] = cur_v;
                return;
            }
            if s == 1 {
                if self.dist[idx] < d {
                    let yk = self.keys[idx];
                    let yv = self.vals[idx];
                    let y_dist = self.dist[idx];
                    self.keys[idx] = cur_k;
                    self.vals[idx] = cur_v;
                    self.dist[idx] = d;
                    cur_k = yk;
                    cur_v = yv;
                    d = y_dist;
                }
            }
            idx = (idx + 1) & mask;
            d = d + 1;
        }
        // 表满兜底（负载 7/8 下不应发生）：扩容后重插
        self.grow();
        self.insert(cur_k, cur_v);
    }
    // 删除：命中槽位标记为墓碑（不破坏后续探测链），返回旧值
    fn remove(&mut self, k: K) -> Option<V> {
        let idx = self.find(k);
        if idx < 0 {
            Option::None
        } else {
            let v = self.vals[idx];
            self.states[idx] = 2;
            self.len = self.len - 1;
            Option::Some(v)
        }
    }
    // 翻倍扩容：新开四数组，重哈希所有占用槽位（墓碑丢弃），释放旧缓冲
    // 四个数组以类型注解显式定型（`alloc_array` 返回 `[Infer; 0]`，
    // 后续索引比较/赋值前必须先统一元素类型）。
    // 重插复用与 insert 同构的 Robin Hood 探测+交换：若只是纯线性探测，键按
    // 旧表槽位顺序重插会使链上距离出现下跳（如 h 的链被 home 槽键打断），
    // 破坏「链上距离非减」不变量 → find 的 dist < d 早退会误判（假阴性）。
    // 交换式重插在任意重插顺序下都维持不变量，早退保持可靠。
    fn grow(&mut self) {
        let new_cap = self.cap * 2;
        let new_keys: [K; 0] = alloc_array(new_cap);
        let new_vals: [V; 0] = alloc_array(new_cap);
        let new_states: [i64; 0] = alloc_array(new_cap);
        let new_dist: [i64; 0] = alloc_array(new_cap);
        let mask = new_cap - 1;
        let mut i = 0;
        while i < self.cap {
            if self.states[i] == 1 {
                // Robin Hood 落位（含交换），与 insert 循环同构
                let mut cur_k = self.keys[i];
                let mut cur_v = self.vals[i];
                let mut idx = hash_value(cur_k) & mask;
                let mut d = 0;
                let mut placed = 0;
                while placed == 0 && d < new_cap {
                    if new_states[idx] == 0 {
                        new_keys[idx] = cur_k;
                        new_vals[idx] = cur_v;
                        new_states[idx] = 1;
                        new_dist[idx] = d;
                        placed = 1;
                    } else if new_keys[idx] == cur_k {
                        // 旧表键唯一，理论不触发；防御性更新
                        new_vals[idx] = cur_v;
                        placed = 1;
                    } else if new_dist[idx] < d {
                        let yk = new_keys[idx];
                        let yv = new_vals[idx];
                        let y_dist = new_dist[idx];
                        new_keys[idx] = cur_k;
                        new_vals[idx] = cur_v;
                        new_dist[idx] = d;
                        cur_k = yk;
                        cur_v = yv;
                        d = y_dist;
                    }
                    idx = (idx + 1) & mask;
                    d = d + 1;
                }
            }
            i = i + 1;
        }
        array_free(self.keys);
        array_free(self.vals);
        array_free(self.states);
        array_free(self.dist);
        self.keys = new_keys;
        self.vals = new_vals;
        self.states = new_states;
        self.dist = new_dist;
        self.used = self.len;
        self.cap = new_cap;
    }
    // 清空：完全重置为容量不变的空表（重开四数组，墓碑/旧数据全部丢弃，
    // 避免墓碑堆积拖慢探测）。与 `remove` 的逐键墓碑不同，后续插入即全新探测链。
    fn clear(&mut self) {
        let k: [K; 0] = alloc_array(self.cap);
        let v: [V; 0] = alloc_array(self.cap);
        let s: [i64; 0] = alloc_array(self.cap);
        let d: [i64; 0] = alloc_array(self.cap);
        array_free(self.keys);
        array_free(self.vals);
        array_free(self.states);
        array_free(self.dist);
        self.keys = k;
        self.vals = v;
        self.states = s;
        self.dist = d;
        self.len = 0;
        self.used = 0;
    }
    // 键集合：稀疏遍历全部槽位，收集 states == 1（存活）的键为 Vec<K>。
    // 遍历顺序与插入无关（开放寻址），调用方断言需 sort 或求和。
    fn keys(&self) -> Vec<K> {
        let mut ks: Vec<K> = Vec::new();
        let mut i = 0;
        while i < self.cap {
            if self.states[i] == 1 {
                ks.push(self.keys[i]);
            }
            i = i + 1;
        }
        ks
    }
    // 值集合：稀疏遍历全部槽位，收集 states == 1（存活）的值为 Vec<V>。
    // 顺序与 keys 一致（同槽位遍历），顺序与插入无关。
    fn values(&self) -> Vec<V> {
        let mut vs: Vec<V> = Vec::new();
        let mut i = 0;
        while i < self.cap {
            if self.states[i] == 1 {
                vs.push(self.vals[i]);
            }
            i = i + 1;
        }
        vs
    }
    // T1c：迭代器——MVP 退化：返回键缓冲（目标 `Iter<'_, K, V>` 键值对迭代器
    // 规划——键值对需元组支持，MVP 未提供；与 keys 同构，可配 values() 配对使用）。
    fn iter(&self) -> Vec<K> {
        let result = self.keys();
        result
    }
    // V4：可变取值——`Option<&mut V>` 引用语义：键不存在返回 None，命中返回对原槽的
    // 可变引用，经 `match { Some(r) => *r = x }` 写回真实槽（非拷贝）。
    fn get_mut(&mut self, k: K) -> Option<&mut V> {
        let idx = self.find(k);
        if idx < 0 {
            Option::None
        } else {
            Option::Some(&mut self.vals[idx])
        }
    }
}

// ===== V5 新集合（2026-08-26）：VecDeque / HashSet / BTreeMap =====
// 三个集合目标 API 见 std-lib.md §1 目标架构目录（collections/hashset.rl、
// collections/btree.rl、collections/deque.rl）。泛型 impl 静态方法（`X::new`/
// `X::with_capacity`）MVP 不支持，构造器由 typecheck 特判展开（与 Vec/HashMap
// 同模式，见 rlyeh-typecheck check_expr.rs check_*_construct）。

// ===== VecDeque<T>：双端队列（V5） =====
// 内部用 `Vec<T>` 作底层数组 + 头索引 `front` + 长度 `len`。逻辑元素为
// `buf[front], buf[front+1], ..., buf[front+len-1]` 连续段。
// - `push_back`：写入逻辑尾部物理位置 `buf[front+len]`；若已在物理末尾则 `push`
//   扩展。`len++`。
// - `push_front`：`front > 0` 时直接前移写入 `buf[front-1]`；`front == 0` 时整体
//   右移腾出 `buf[0]`（先 `push` 扩展物理长度，再从尾向前搬移）。`len++`。
// - `pop_front`：读 `buf[front]` 并 `front++`、`len--`。
// - `pop_back`：读 `buf[front+len-1]` 并 `len--`。
// 布局 3 槽：槽 0 = buf（`Vec<T>` 对象指针，Ptr）、槽 1 = front（头索引，Int）、
// 槽 2 = len（元素个数，Int）。
struct VecDeque<T> {
    buf: Vec<T>,
    front: i64,
    len: i64,
}

impl<T> VecDeque<T> {
    // 元素个数
    fn len(&self) -> i64 {
        self.len
    }
    // 是否为空
    fn is_empty(&self) -> bool {
        self.len == 0
    }
    // 尾插：写入逻辑尾部物理位置；已在物理末尾则 push 扩展
    fn push_back(&mut self, x: T) {
        if self.front + self.len < self.buf.len() {
            self.buf[self.front + self.len] = x;
        } else {
            self.buf.push(x);
        }
        self.len = self.len + 1;
    }
    // 头插：front > 0 直接前移；front == 0 整体右移腾出 buf[0]
    fn push_front(&mut self, x: T) {
        if self.front > 0 {
            self.front = self.front - 1;
            self.buf[self.front] = x;
        } else {
            self.buf.push(x); // 先扩展物理长度（临时尾元素，稍后右移）
            let mut i = self.buf.len() - 1;
            while i > 0 {
                self.buf[i] = self.buf[i - 1];
                i = i - 1;
            }
            self.buf[0] = x;
        }
        self.len = self.len + 1;
    }
    // 头出：非空返回头元素并右移头索引，空返回 None
    fn pop_front(&mut self) -> Option<T> {
        if self.len == 0 {
            Option::None
        } else {
            let v = self.buf[self.front];
            self.front = self.front + 1;
            self.len = self.len - 1;
            Option::Some(v)
        }
    }
    // 尾出：返回逻辑末尾元素
    fn pop_back(&mut self) -> Option<T> {
        if self.len == 0 {
            Option::None
        } else {
            let v = self.buf[self.front + self.len - 1];
            self.len = self.len - 1;
            Option::Some(v)
        }
    }
    // 访问头元素（不移除）
    fn front(&self) -> Option<T> {
        if self.len == 0 {
            Option::None
        } else {
            Option::Some(self.buf[self.front])
        }
    }
    // 访问尾元素（不移除）
    fn back(&self) -> Option<T> {
        if self.len == 0 {
            Option::None
        } else {
            Option::Some(self.buf[self.front + self.len - 1])
        }
    }
}

// ===== HashSet<T>：开放寻址哈希集合（V5） =====
// 精简线性探测（无 Robin Hood 距离数组）：items 数组存元素、states 数组标
// 状态（0=空 1=占用 2=墓碑）。`hash_value` 内建（i64 直哈希 / String djb2
// 内容哈希，与 HashMap 键同构）。插入遇墓碑可复用槽位（贪心复用最早的墓碑），
// 负载因子 used/cap >= 7/8 时翻倍扩容重哈希。
// 布局 5 槽：槽 0 = items 指针、槽 1 = states 指针、槽 2 = len、槽 3 = used、
// 槽 4 = cap（2 的幂）。
struct HashSet<T> {
    items: [T; 0],
    states: [i64; 0],
    len: i64,
    used: i64,
    cap: i64,
}

impl<T> HashSet<T> {
    // 查找元素所在槽位，未找到返回 -1（线性探测，遇空槽终止）
    fn find(&self, x: T) -> i64 {
        let mask = self.cap - 1;
        let mut idx = hash_value(x) & mask;
        let mut d = 0;
        while d < self.cap {
            let s = self.states[idx];
            if s == 0 {
                return -1;
            }
            if s == 1 && self.items[idx] == x {
                return idx;
            }
            idx = (idx + 1) & mask;
            d = d + 1;
        }
        -1
    }
    fn len(&self) -> i64 {
        self.len
    }
    fn cap(&self) -> i64 {
        self.cap
    }
    fn is_empty(&self) -> bool {
        self.len == 0
    }
    fn contains(&self, x: T) -> bool {
        self.find(x) >= 0
    }
    // 插入：已存在则忽略（集合语义），否则落位（贪心复用最早墓碑，其次空槽）
    fn insert(&mut self, x: T) {
        if self.used * 8 >= self.cap * 7 {
            self.grow();
        }
        let mask = self.cap - 1;
        let mut idx = hash_value(x) & mask;
        let mut d = 0;
        let mut tomb = -1;
        while d < self.cap {
            let s = self.states[idx];
            if s == 0 {
                if tomb >= 0 {
                    // 复用墓碑槽
                    self.items[tomb] = x;
                    self.states[tomb] = 1;
                } else {
                    self.items[idx] = x;
                    self.states[idx] = 1;
                }
                self.used = self.used + 1;
                self.len = self.len + 1;
                return;
            }
            if s == 1 && self.items[idx] == x {
                return; // 已存在
            }
            if s == 2 && tomb < 0 {
                tomb = idx; // 记录首个墓碑（可复用）
            }
            idx = (idx + 1) & mask;
            d = d + 1;
        }
        // 表满兜底（负载 7/8 下不应发生）：扩容后重插
        self.grow();
        self.insert(x);
    }
    // 删除：命中槽位标记为墓碑，返回是否删除成功
    fn remove(&mut self, x: T) -> bool {
        let idx = self.find(x);
        if idx < 0 {
            false
        } else {
            self.states[idx] = 2;
            self.len = self.len - 1;
            true
        }
    }
    // 翻倍扩容：新开双数组，重哈希所有占用槽位（墓碑丢弃），释放旧缓冲
    fn grow(&mut self) {
        let new_cap = self.cap * 2;
        let new_items: [T; 0] = alloc_array(new_cap);
        let new_states: [i64; 0] = alloc_array(new_cap);
        let mask = new_cap - 1;
        let mut i = 0;
        while i < self.cap {
            if self.states[i] == 1 {
                let mut cur = self.items[i];
                let mut idx = hash_value(cur) & mask;
                let mut d = 0;
                let mut placed = 0;
                while placed == 0 && d < new_cap {
                    if new_states[idx] == 0 {
                        new_items[idx] = cur;
                        new_states[idx] = 1;
                        placed = 1;
                    }
                    idx = (idx + 1) & mask;
                    d = d + 1;
                }
            }
            i = i + 1;
        }
        array_free(self.items);
        array_free(self.states);
        self.items = new_items;
        self.states = new_states;
        self.used = self.len;
        self.cap = new_cap;
    }
    // 清空：完全重置为容量不变的空集（重开双数组，墓碑/旧数据全部丢弃）
    fn clear(&mut self) {
        let n: [T; 0] = alloc_array(self.cap);
        let s: [i64; 0] = alloc_array(self.cap);
        array_free(self.items);
        array_free(self.states);
        self.items = n;
        self.states = s;
        self.len = 0;
        self.used = 0;
    }
    // 元素集合：稀疏遍历全部槽位，收集 states == 1 的元素为 Vec<T>
    fn elements(&self) -> Vec<T> {
        let mut es: Vec<T> = Vec::new();
        let mut i = 0;
        while i < self.cap {
            if self.states[i] == 1 {
                es.push(self.items[i]);
            }
            i = i + 1;
        }
        es
    }
}

// ===== BTreeMap<K, V>：有序映射（数组二分 + 移动，V5） =====
// 键升序存于 keys 数组，vals 平行对齐。插入经二分查找定位 + 右移腾位
// （O(n) 移动，MVP 数组实现），命中键则覆盖值。MVP 限 i64 键（有序比较）。
// 布局 3 槽：槽 0 = keys 指针、槽 1 = vals 指针、槽 2 = len。
struct BTreeMap<K, V> {
    keys: [K; 0],
    vals: [V; 0],
    len: i64,
}

impl<K, V> BTreeMap<K, V> {
    // 二分查找键：命中返回槽位索引，未命中返回插入位（负 = -pos-1）
    fn find(&self, k: K) -> i64 {
        let mut lo = 0;
        let mut hi = self.len;
        while lo < hi {
            let mid = (lo + hi) / 2;
            let kv = self.keys[mid];
            if kv == k {
                return mid;
            }
            if kv < k {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        -lo - 1
    }
    fn len(&self) -> i64 {
        self.len
    }
    fn is_empty(&self) -> bool {
        self.len == 0
    }
    fn contains_key(&self, k: K) -> bool {
        self.find(k) >= 0
    }
    fn get(&self, k: K) -> Option<V> {
        let idx = self.find(k);
        if idx < 0 {
            Option::None
        } else {
            Option::Some(self.vals[idx])
        }
    }
    // 插入（保持有序）：命中覆盖值；未命中右移腾位插入新键值对
    fn insert(&mut self, k: K, v: V) {
        let idx = self.find(k);
        if idx >= 0 {
            self.vals[idx] = v;
            return;
        }
        let pos = -idx - 1;
        let mut i = self.len;
        while i > pos {
            self.keys[i] = self.keys[i - 1];
            self.vals[i] = self.vals[i - 1];
            i = i - 1;
        }
        self.keys[pos] = k;
        self.vals[pos] = v;
        self.len = self.len + 1;
    }
    // 删除：命中右移覆盖移除，返回是否成功
    fn remove(&mut self, k: K) -> bool {
        let idx = self.find(k);
        if idx < 0 {
            false
        } else {
            let mut i = idx;
            while i < self.len - 1 {
                self.keys[i] = self.keys[i + 1];
                self.vals[i] = self.vals[i + 1];
                i = i + 1;
            }
            self.len = self.len - 1;
            true
        }
    }
    // 最小键
    fn first(&self) -> Option<K> {
        if self.len == 0 {
            Option::None
        } else {
            Option::Some(self.keys[0])
        }
    }
    // 最大键
    fn last(&self) -> Option<K> {
        if self.len == 0 {
            Option::None
        } else {
            Option::Some(self.keys[self.len - 1])
        }
    }
    // 键集合（有序）
    fn keys(&self) -> Vec<K> {
        let mut ks: Vec<K> = Vec::new();
        let mut i = 0;
        while i < self.len {
            ks.push(self.keys[i]);
            i = i + 1;
        }
        ks
    }
    // 值集合（与键顺序对齐）
    fn values(&self) -> Vec<V> {
        let mut vs: Vec<V> = Vec::new();
        let mut i = 0;
        while i < self.len {
            vs.push(self.vals[i]);
            i = i + 1;
        }
        vs
    }
}

// ===== extern 声明集中区（模块化拆分，2026-08-22）=====
// 编译器对 extern 按声明符号名生成 LLVM `declare`（`__rlyeh_` 前缀者由 driver
// 注入 define，跳过 declare）。符号名必须与 libc 一致，故不能放入子模块
// （模块前缀会改变 LLVM 符号名，链接器将无法解析 libc 符号）。
// 使用方见各子模块文件头注释（time.rl / io.rl / net.rl / sync.rl）。

// --- time.rl：进程 CPU 时钟 + 墙钟 ---
extern fn clock() -> i64;
// S2b：墙钟（clock_gettime CLOCK_MONOTONIC，返回微秒）。driver 注入 define
// （time_builtin_ir，见 rlyeh-driver lib.rs）；不支持的平台返回 -1，
// 语言侧 `Instant::now/elapsed` 退回 clock()（CPU 时钟）。
extern fn __rlyeh_clock_monotonic() -> i64;
// X1：系统时间（clock_gettime CLOCK_REALTIME，返回微秒）。driver 注入 define
// （time_builtin_ir，见 rlyeh-driver lib.rs）；不支持的平台返回 -1，
// 语言侧 `SystemTime::now` 退回 UNIX 纪元。
extern fn __rlyeh_clock_realtime() -> i64;

// --- io.rl：libc stdio + POSIX read ---
extern fn fopen(path: String, mode: String) -> i64;
extern fn fread(buf: String, size: i64, nmemb: i64, f: i64) -> i64;
extern fn fwrite(buf: String, size: i64, nmemb: i64, f: i64) -> i64;
extern fn fclose(f: i64) -> i64;
extern fn fseek(f: i64, offset: i64, whence: i64) -> i64;
extern fn ftell(f: i64) -> i64;
extern fn fflush(f: i64) -> i32;   // N1c：File::flush（stdio 缓冲刷盘）
extern fn read(fd: i64, buf: String, count: i64) -> i64;   // 控制台读取（fd 0 = stdin）
// Y1（2026-08）：文件元数据（stat 平台差异由 driver 注入 define，见
// rlyeh-driver lib.rs file_stat_builtin_ir）：统一签名 (path) → 字段值；
// 失败（路径不存在等）返回 -1；非 Linux/macOS 平台注入 -1 stub。
// st_mode & S_IFMT 掩码：S_IFREG = 0x8000（普通文件），S_IFDIR = 0x4000。
extern fn __rlyeh_file_size(path: String) -> i64;
extern fn __rlyeh_file_mtime(path: String) -> i64;
extern fn __rlyeh_file_mode(path: String) -> i64;

// --- io/nio.rl + io/sendfile.rl：非阻塞 IO + 零拷贝传输（R 阶段，2026-08）---
extern fn fcntl(fd: i64, cmd: i64, arg: i64) -> i32;   // F_GETFL=3 / F_SETFL=4（O_NONBLOCK=0x4）；i32 返回 → extern_ret32 清洗
extern fn poll(fds: String, nfds: i64, timeout: i64) -> i32;   // pollfd 缓冲（8 字节/项）
// Y2a（2026-08-28）：kqueue/kevent（macOS/BSD 高性能事件后端）。kevent 结构体
// 32 字节缓冲（ident uintptr 8B + filter int16 2B + flags uint16 2B + fflags
// uint32 4B + data intptr 8B + udata ptr 8B），经 String 承载传 data 指针；
// timeout 为 `{timespec.tv_sec i64, tv_nsec i64}` 16 字节缓冲（String）或空。
extern fn __rlyeh_kqueue() -> i32;
extern fn __rlyeh_kevent(kq: i64, changelist: String, nchanges: i64, eventlist: String, nevents: i64, timeout: String) -> i32;
extern fn fileno(f: i64) -> i32;   // FILE* → 底层 fd（File::sendfile_to）
// sendfile(2) 平台差异由 driver 注入 define（见 rlyeh-driver lib.rs platform_builtin_ir）：
// 统一签名 (out_fd, in_fd, off_ptr, count)；WASI 下注入返回 -1 的 stub。
extern fn __rlyeh_sendfile(out_fd: i64, in_fd: i64, offset: String, count: i64) -> i64;

// --- thread（S 阶段，2026-08）：线程支持 ---
// pthread 差异由 driver 注入 define（thread_builtin_ir）：
// __rlyeh_thread_spawn(entry, arg) → 线程 tid 或 -1（pthread_create 封装）；
// entry 为函数指针值（语言侧经 extern i64 形参传地址整数，codegen ptrtoint）；
// __rlyeh_thread_join(tid) → 线程返回值槽内容或 -1；
// __rlyeh_thread_self() → 当前线程 id。
// __rlyeh_thread_sleep(micros) → 0 成功 / -1 失败（S2a，usleep 绑定）。
extern fn __rlyeh_thread_spawn(entry: i64, arg: i64) -> i64;
extern fn __rlyeh_thread_join(tid: i64) -> i64;
extern fn __rlyeh_thread_self() -> i64;
extern fn __rlyeh_thread_sleep(micros: i64) -> i64;
extern fn __rlyeh_thread_spawn_stack(entry: i64, arg: i64, stack_size: i64) -> i64;  // Y8：Builder::stack_size 定制线程栈（<=0 → 默认栈）

// --- fs.rl：路径 + 文件系统 ---
extern fn access(path: String, mode: i64) -> i32;   // N3a：F_OK=0 存在性
extern fn unlink(path: String) -> i32;              // N3c：remove_file
extern fn r#rename(from: String, to: String) -> i32; // N3c：重命名/移动
extern fn mkdir(path: String, mode: i64) -> i32;    // N3c：create_dir
extern fn rmdir(path: String) -> i32;               // N3c：remove_dir（空目录）
extern fn popen(command: String, mode: String) -> i64;  // N3c：read_dir / remove_dir_all（ls / rm）
extern fn pclose(f: i64) -> i32;                    // N3c：popen 句柄关闭
extern fn write(fd: i64, buf: String, count: i64) -> i64;  // 控制台写入（N2a：fd 1 stdout / 2 stderr）

// --- net.rl：套接字 ---
// AF_UNIX=1, SOCK_STREAM=1（macOS/Linux 一致）
extern fn socketpair(domain: i64, type_: i64, protocol: i64, fds: String) -> i32;
extern fn socket(domain: i64, type_: i64, protocol: i64) -> i32;
extern fn connect(fd: i64, addr: String, len: i64) -> i32;
extern fn close(fd: i64) -> i32;
// O 阶段（2026-08）：TCP 服务器/HTTP 所需——bind/listen/accept/setsockopt/
// getsockname/shutdown。addr/len 传 String（codegen 取 data 指针）：
//  - `bind`/`connect` 的 addr 为 sockaddr_in 缓冲（sockaddr_in4 构造），len 定值 16
//  - `accept`/`getsockname` 的 addr/len 为 out 缓冲（内核回填），len 初值须为
//    socklen_t 小端 16（int_buf4），addr 容量 ≥16
extern fn bind(fd: i64, addr: String, len: i64) -> i32;
extern fn listen(fd: i64, backlog: i64) -> i32;
extern fn accept(fd: i64, addr: String, len: String) -> i32;
extern fn getsockname(fd: i64, addr: String, len: String) -> i32;
extern fn setsockopt(fd: i64, level: i64, optname: i64, optval: String, optlen: i64) -> i32;
extern fn shutdown(fd: i64, how: i64) -> i32;
// `send`/`recv` 为 actor 保留字，用原始标识符 `r#` 绕开（lexer 解为 Ident）
extern fn r#send(fd: i64, buf: String, len: i64, flags: i64) -> i64;
extern fn r#recv(fd: i64, buf: String, len: i64, flags: i64) -> i64;
// Y7（2026-08）：UDP——sendto/recvfrom（ssize_t i64 承载；recvfrom 的
// addr/addrlen 为输出回填缓冲 String，codegen 取 data 指针，与 accept/getsockname 一致）
extern fn sendto(fd: i64, buf: String, len: i64, flags: i64, addr: String, addrlen: i64) -> i64;
extern fn recvfrom(fd: i64, buf: String, len: i64, flags: i64, addr: String, addrlen: String) -> i64;
// 编译器注入的平台内建：返回当前目标 OS 码（0=未知 1=linux 2=macos 3=windows 4=freebsd）。
// 由 rlyeh-driver 在汇编阶段注入 `define internal i32 @__rlyeh_target_os()`；
// codegen 对 `__rlyeh_` 前缀 extern 不生成 declare（避免同符号 declare+define 冲突）。
extern fn __rlyeh_target_os() -> i32;
extern fn gethostname(name: String, len: i64) -> i64;

// --- sync.rl：pthread 互斥锁 / 读写锁 ---
// 分配用 `calloc` 而非 `malloc`：内建 alloc_array/alloc_bytes 已按
// `i8* @malloc(i64)` 声明 malloc，再以 i64 返回声明会触发 LLVM
// "invalid redefinition of function 'malloc'"；calloc 符号无内建冲突。
extern fn calloc(n: i64, size: i64) -> i64;
extern fn pthread_mutex_init(m: i64, attr: i64) -> i32;
extern fn pthread_mutex_lock(m: i64) -> i32;
extern fn pthread_mutex_unlock(m: i64) -> i32;
extern fn pthread_mutex_trylock(m: i64) -> i32;
extern fn pthread_rwlock_init(r: i64, attr: i64) -> i32;
extern fn pthread_rwlock_rdlock(r: i64) -> i32;
extern fn pthread_rwlock_wrlock(r: i64) -> i32;
extern fn pthread_rwlock_unlock(r: i64) -> i32;
extern fn pthread_rwlock_tryrdlock(r: i64) -> i32;
extern fn pthread_rwlock_trywrlock(r: i64) -> i32;
// P 阶段（2026-08）：条件变量 / 屏障（pthread_cond_* / pthread_barrier_*）
extern fn pthread_cond_init(c: i64, attr: i64) -> i32;
extern fn pthread_cond_destroy(c: i64) -> i32;
extern fn pthread_cond_wait(c: i64, m: i64) -> i32;
extern fn pthread_cond_signal(c: i64) -> i32;
extern fn pthread_cond_broadcast(c: i64) -> i32;
extern fn pthread_barrier_init(b: i64, attr: i64, count: i64) -> i32;
extern fn pthread_barrier_wait(b: i64) -> i32;

// ===== 子模块拆分（2026-08-22）=====
// time/io/net/sync 的实现拆分为独立文件（rlyeh-std/rlyeh/<name>.rl），
// 由 driver 加载标准库时经模块展开（module foo; → module foo { ... }）合入。
// 声明置于文件末尾：类型符号顺序解析，根类型（String/Vec/Option/Result/
// HashMap 及编译器特判函数）必须先于子模块签名注册。
// 拆分理由：io/net/sync 基于 extern FFI 绑定 libc，模块化便于独立演进；
// String/Vec/Option/Result/HashMap 因编译器按全名特判（构造器展开、比较、
// 数组切片等），必须留在根命名空间。
module time;
module io;
module future;
// W5：future 符号在 net/sync 之前 import，使 net/http 与 sync 模块（`impl Future`
// 自建 future 类型）收集阶段能经 use_aliases 解析裸名 Future/Poll/Context
// （跨模块类型，非自建；future 不依赖 net，故可前置于 net）。
import future::Future;
import future::Poll;
import future::Context;
module net;
module sync;
module fs;
module thread;
module serde;
module fmt;

// 重新导出到根命名空间，保持用户 API 不变（裸名即用，无需前缀）。
// 目录化（2026-08）：子模块按 std-lib.md §1 目标架构拆分为目录形式，
// import 路径指向类型文件完整路径（io::error::IoError 等）。
import serde::Serialize;
import time::Duration;
import time::Instant;
import time::system::SystemTime;
import io::error::IoErrorKind;
import io::error::IoError;
import io::error::Error;
import io::OpenMode;
import io::file::File;
import io::console::Stdout;
import io::console::Stderr;
import io::console::stdout;
import io::console::stderr;
import io::console::read_to_string;
import io::console::lines;
import io::c_str;
import io::file::read_file;
import io::file::write_file;
import io::file::append_file;
import io::console::read_line;
import net::byteorder::htons;
import net::socketpair_stream;
import net::fd_at;
import net::send_all;
import net::recv_some;
import net::byteorder::sockaddr_in4_with_layout;
import net::byteorder::sockaddr_in4;
import net::tcp_connect;
import net::hostname;
import net::addr::Ipv4Octets;
import net::addr::ipv4_octets;
import net::addr::SocketAddr;
import net::tcp::TcpListener;
import net::tcp::TcpStream;
import net::udp::UdpSocket;
import net::udp::UdpPacket;
import net::addr::Shutdown;
import net::http::HttpClient;
import net::http::Response;
import net::http::parse_url;
import sync::Mutex;
import sync::RwLock;
import sync::MutexGuard;
import sync::Condvar;
import sync::Barrier;
import sync::Sender;
import sync::Receiver;
import sync::ChannelPair;
import sync::RecvAsync;
import sync::channel;
import fs::path::Path;
import io::nio::Interest;
import io::nio::Event;
import io::nio::Poller;
import io::nio::set_nonblocking;
import io::nio::is_nonblocking;
import io::sendfile::sendfile;
import thread::Thread;
import thread::Builder;   // Y8：线程栈定制构建器
import thread::sleep;
import thread::join_all;
import future::Future;
import future::Poll;
import future::Context;
import future::block_on;
import future::timeout;
import future::TimeoutError;
import fmt::Display;
import fmt::Debug;
import fmt::Formatter;

