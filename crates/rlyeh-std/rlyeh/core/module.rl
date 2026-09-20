// Rlyeh 标准库 core：根命名空间类型定义（纯 Rlyeh 实现）
//
// 位置：`rlyeh-std/rlyeh/core/module.rl`（core 入口，2026-09-18 由单文件 `core.rl` 重整）。
// - `rlyeh build/run` 文件入口自动注入标准库（rlyeh-driver/src/stdlib.rs 定位：
//   环境变量 `RLYEH_STD_PATH` 优先，默认 <workspace>/crates/rlyeh-std/rlyeh/）。
// - 预置加载顺序：**`core`（本文件，根命名空间类型）→ 与 core 平级的平铺单元
//   （str_ext / convert / collections / externs）→ `module.rl`（子模块声明 + 根重导出）**。
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

// SH-P1-2（0.2.0-C）：derive 宏所依赖的 protocol 声明。
//
// 编译器在 `#[derive(Clone/PartialEq)]` 处自动合成对应 `impl`（见
// `rlyeh-typecheck` 的 derive 展开）；用户亦可直接手写
// `impl Clone / PartialEq for T`。`Debug` protocol 已在 `fmt` 模块声明
// （`fmt::Debug`，供 `dbg!` / `{:?}` 引擎接入）。
protocol Clone {
    fn clone(&self) -> Self;
}
protocol PartialEq {
    fn eq(&self, other: &Self) -> bool;
}
protocol PartialOrd {
    fn lt(&self, other: &Self) -> bool;
    fn le(&self, other: &Self) -> bool;
    fn gt(&self, other: &Self) -> bool;
    fn ge(&self, other: &Self) -> bool;
}

// SH-P1-5（0.2.0-S，2026-09-04）：`Copy` 标记 protocol（无方法）。值语义标记：
// 标记为 Copy 的类型在赋值 / 传参时按位拷贝。Rlyeh 默认聚合即按值拷贝（无 move
// 语义），故 `Copy` 主要作为泛型约束 `T: Copy` 与 `#[derive(Copy)]` 的标记，与
// Rust `Copy` 语义对齐。`#[derive(Copy)]` 由 typecheck 展开为 `impl Copy for T {}`
// （见 `rlyeh-typecheck/src/check_item/derive.rs`）。
protocol Copy {
}

// SH-P1-4（0.2.0-R，2026-09-04 起步）：`Deref` / `DerefMut` 用户类型自动解引用
// 强制所需的核心 protocol 声明（关联类型 `Target` + `deref` / `deref_mut`）。具体
// 自动解引用强制（字段/方法/索引访问失败时插入 `*(x.deref())` 递归，限深度）见
// `rlyeh-typecheck` 解析回退（M2）；此处先落地 protocol + 关联类型（M1）。
protocol Deref {
    type Target;
    fn deref(&self) -> &Self::Target;
}
protocol DerefMut {
    type Target;
    fn deref_mut(&mut self) -> &mut Self::Target;
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
    // V1：只读引用迭代器——返回 `IterRef<T>`（*const T + 剩余长度），next() 返回
    // 元素 `Option<&T>` 引用（零拷贝，指向原缓冲真实槽）。可用于零拷贝读取与
    // 原地写回（`*r = x`）。迭代期间不得对 Vec 做结构性修改（指针悬垂）。
    fn iter_ref(&self) -> IterRef<T> {
        let p: *const T = &self.data[0];
        IterRef::new(p, self.len)
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

// V3-D2（2026-08-27）：Iter<T> 实现 Iterator protocol（type Item = T），使其
// 能调用迁移后的惰性适配器默认方法（map/filter/take 等）。inherent next
// 优先于 protocol next（方法解析），protocol next 供 Iterator 语义/默认方法使用。
impl<T> Iter<T>: Iterator {
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

// V3-D2（2026-08-27）：IterMut<T> 实现 Iterator protocol（type Item = T）。
impl<T> IterMut<T>: Iterator {
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

// V1：只读引用迭代器——`IterRef<T>` 零分配引用视图（next() 返回 `Option<&T>`）。
// 与 Iter<T> 同布局（*const T + 剩余长度），但 next 返回元素引用而非值拷贝，
// 支持零拷贝读取与写回原缓冲。接入 for 循环（inherent next 检测，无需 Iterator
// protocol——`type Item = &T` 引用类型对适配器框架不友好，for 循环仅需 inherent next）。
struct IterRef<T> {
    data: *const T,
    len: i64,
}

impl<T> IterRef<T> {
    // 取当前元素引用（指向原缓冲真实槽）并推进：耗尽返回 None。
    fn next(&mut self) -> Option<&T> {
        if self.len == 0 {
            return Option::None;
        }
        let r: &T = &self.data[0];
        self.data = self.data + 1;
        self.len = self.len - 1;
        Option::Some(r)
    }
    fn len(&self) -> i64 {
        self.len
    }
    fn is_empty(&self) -> bool {
        self.len == 0
    }
}

// T2：Iterator protocol（V3-A3，2026-08-27：引入 `type Item` 关联类型替代固定 i64，
// `next` 返回 `Option<Self::Item>`）。自定义迭代器经
// `impl Iterator for T { type Item = i64; fn next(&mut self) -> Option<i64> }`
// 接入 for 循环（check_for_iterator 检测 next() 方法，inherent 或 protocol impl 均可）。
// 泛型元素迭代器（如 StdinLines 返回 Option<String>）仍走方法式接入。
protocol Iterator {
    // 元素类型（V3-A3）：impl 提供 `type Item = <具体类型>`
    type Item;

    fn next(&mut self) -> Option<Self::Item>;

    // ===== V3 默认方法：基于 next() 的实现，impl 未显式实现时回退
    // （typecheck protocol 默认方法机制）。MVP 默认方法仍按 i64 元素实现
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
    // 待 protocol 默认方法支持消耗式 `self` 后补（见 v3-b 叶子）。

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

// ===== V5d 运算符重载 protocols（2026-09-02） =====
// 对标 P009 设计稿：每个可重载二元运算符对应一个 protocol + `type Output` 关联类型
// + `fn <op>(self, other: Self) -> Self::Output`。`a OP b` 由 typecheck 在内建
// 路径失败后降级为 `a.<op>(b)` 方法调用（复用既有 method-call 全链路，codegen
// 无需改动）。仅 BinaryOp 运算符可重载（逻辑 &&/|| 短路、比较链 < > 等走独立
// 路径，本期不重载）。方法名对齐 Rust std::ops（add/sub/mul/div/rem/bitand/bitor/
// bitxor/shl/shr）。
protocol Add { type Output; fn add(self, other: Self) -> Self::Output; }
protocol Sub { type Output; fn sub(self, other: Self) -> Self::Output; }
protocol Mul { type Output; fn mul(self, other: Self) -> Self::Output; }
protocol Div { type Output; fn div(self, other: Self) -> Self::Output; }
protocol Rem { type Output; fn rem(self, other: Self) -> Self::Output; }
protocol BitAnd { type Output; fn bitand(self, other: Self) -> Self::Output; }
protocol BitOr { type Output; fn bitor(self, other: Self) -> Self::Output; }
protocol BitXor { type Output; fn bitxor(self, other: Self) -> Self::Output; }
protocol Shl { type Output; fn shl(self, other: Self) -> Self::Output; }
protocol Shr { type Output; fn shr(self, other: Self) -> Self::Output; }

// ===== V3-C 包装迭代器（2026-08-27）：惰性适配器基础设施 =====
// 泛型包装迭代器持底层迭代器 `I` + 参数槽，`next()` 实现变换逻辑。
// MVP 元素固定 i64（`type Item = i64`，与 V3-A3 默认方法一致）；泛型 `I::Item`
// 投影传播待 `where I: Iterator` 泛型约束完善后泛化。

// Filter<I>：跳过不满足谓词的元素
struct Filter<I> {
    inner: I,
    pred: fn(i64) -> bool,
}

impl<I> Filter<I>: Iterator {
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

impl<I> Take<I>: Iterator {
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

impl<I> Skip<I>: Iterator {
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

impl<A, B> Chain<A, B>: Iterator {
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

impl<I> Enumerate<I>: Iterator {
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

impl<I, B> Map<I, B>: Iterator {
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


// ===== 其余内容已拆分至与 core 平级的平铺单元（2026-09-18）=====
// 拼接顺序（rlyeh-driver/src/stdlib.rs `FLAT_UNITS`，与原 core.rl 行序一致）：
//   str_ext/module.rl → convert/module.rl → collections/module.rl
//   → externs/module.rl → 最后 `module.rl`（子模块声明 + 根命名空间重导出）。
// 这些单元**平铺**（不经 `module x;` 展开、不包 module 壳）以保持全名与符号顺序不变
// （编译器按全名特判 String/Vec/HashMap 等；extern 符号名须与 libc 一致）。
