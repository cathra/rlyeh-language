// collections/module.rl：HashMap / HashSet / VecDeque / BTreeMap 及迭代器（2026-09-18 由 core.rl 拆分）。

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
    // V1-b：键值对引用迭代器——`iter_pairs() -> HashMapIter<K,V>`，`next()` 返回
    // `Option<KVRef<K,V>>`（`key`/`val` 两裸指针指向原 keys/vals 真实槽，零拷贝）。
    // 遍历跳过空/墓碑槽（states != 1）。因 Rlyeh 元组运行时未就绪，键值对以
    // `KVRef` 结构体承载，等价 Rust `(&K, &V)` 引用语义。迭代期间不得对 HashMap
    // 做结构性修改（push/remove/grow 触发重哈希会使指针悬垂）。
    fn iter_pairs(&self) -> HashMapIter<K, V> {
        let sp: *const i64 = &self.states[0];
        let kp: *const K = &self.keys[0];
        let vp: *const V = &self.vals[0];
        HashMapIter { states: sp, keys: kp, vals: vp, idx: 0, cap: self.cap }
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
    // ===== 按键集合运算（V5d+，2026-09-02）：对标 Python dict 合并 =====
    // 并集 `a | b`：键集合 A ∪ B（所有键来自 a 与 b）；冲突键取右操作数 b 的值
    // （b 胜，与 Python `dict | dict` 语义一致，b 在 a 之后插入自然覆盖）。返回新 map。
    fn union(&self, other: &HashMap<K, V>) -> HashMap<K, V> {
        let mut r: HashMap<K, V> = HashMap::new();
        let ka = self.keys();
        let mut i = 0;
        while i < ka.len() {
            r.insert(ka[i], self.get(ka[i]).unwrap());
            i = i + 1;
        }
        let kb = other.keys();
        let mut j = 0;
        while j < kb.len() {
            r.insert(kb[j], other.get(kb[j]).unwrap());
            j = j + 1;
        }
        r
    }
    // 交集 `a & b`：键集合 A ∩ B（仅保留同时存在于 a、b 的键）；值取左操作数 a
    // （结果 ⊆ a，自然保留 a 的值）。返回新 map。
    fn intersection(&self, other: &HashMap<K, V>) -> HashMap<K, V> {
        let mut r: HashMap<K, V> = HashMap::new();
        let ka = self.keys();
        let mut i = 0;
        while i < ka.len() {
            if other.contains_key(ka[i]) {
                r.insert(ka[i], self.get(ka[i]).unwrap());
            }
            i = i + 1;
        }
        r
    }
    // 差集 `a - b`：键集合 A - B（仅保留存在于 a 但不存在于 b 的键）；值取左操作数
    // a（结果 ⊆ a，自然保留 a 的值）。返回新 map。
    fn difference(&self, other: &HashMap<K, V>) -> HashMap<K, V> {
        let mut r: HashMap<K, V> = HashMap::new();
        let ka = self.keys();
        let mut i = 0;
        while i < ka.len() {
            if !other.contains_key(ka[i]) {
                r.insert(ka[i], self.get(ka[i]).unwrap());
            }
            i = i + 1;
        }
        r
    }
    // 对称差 `a ^ b`：键集合 (A - B) ∪ (B - A)（仅存在于一方、不共享的键）；a 独有键
    // 取 a 的值，b 独有键取 b 的值。返回新 map。
    fn symmetric_difference(&self, other: &HashMap<K, V>) -> HashMap<K, V> {
        let mut r: HashMap<K, V> = HashMap::new();
        let ka = self.keys();
        let mut i = 0;
        while i < ka.len() {
            if !other.contains_key(ka[i]) {
                r.insert(ka[i], self.get(ka[i]).unwrap());
            }
            i = i + 1;
        }
        let kb = other.keys();
        let mut j = 0;
        while j < kb.len() {
            if !self.contains_key(kb[j]) {
                r.insert(kb[j], other.get(kb[j]).unwrap());
            }
            j = j + 1;
        }
        r
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

// V1-b：HashMap 键值对引用迭代器（零拷贝视图）。`HashMapIter` 持有原表三数组指针
// （states/keys/vals）+ 游标 + 容量；`next()` 跳过 states != 1 的空/墓碑槽，返回
// `KVRef`（两裸指针指向原 keys/vals 真实槽）。等价 Rust `(&K, &V)` 引用语义，但用
// 结构体承载（Rlyeh 元组运行时未就绪）。接入 for 循环（inherent next 检测，无需
// Iterator protocol——`type Item = KVRef<K,V>` 对适配器框架不友好，for 循环仅需
// inherent next）。
struct HashMapIter<K, V> {
    states: *const i64,
    keys: *const K,
    vals: *const V,
    idx: i64,
    cap: i64,
}

struct KVRef<K, V> {
    key: *const K,
    val: *const V,
}

impl<K, V> HashMapIter<K, V> {
    // 取下一存活槽的键值引用对并推进；耗尽（无更多 states==1 槽）返回 None。
    fn next(&mut self) -> Option<KVRef<K, V>> {
        while self.idx < self.cap {
            if self.states[self.idx] == 1 {
                let k: *const K = &self.keys[self.idx];
                let v: *const V = &self.vals[self.idx];
                self.idx = self.idx + 1;
                return Option::Some(KVRef { key: k, val: v });
            }
            self.idx = self.idx + 1;
        }
        Option::None
    }
}

// ===== HashSetIter<T>：HashSet 只读引用迭代器（V5c） =====
// `iter() -> HashSetIter<T>`，`next()` 返回 `Option<&T>`（指向原 items 真实槽，
// 零拷贝）。遍历跳过空/墓碑槽（states != 1）。复用 HashMapIter 同式裸指针视图；
// 迭代期间不得对 HashSet 做结构性修改（insert/remove/grow 触发重哈希会使指针悬垂）。
// 接入 for 循环（inherent next 检测，无需 Iterator protocol——`type Item = &T` 引用
// 类型对适配器框架不友好，for 循环仅需 inherent next）。
struct HashSetIter<T> {
    states: *const i64,
    items: *const T,
    idx: i64,
    cap: i64,
}

impl<T> HashSetIter<T> {
    // 取下一存活槽元素的引用（指向原 items 真实槽）并推进；耗尽返回 None。
    fn next(&mut self) -> Option<&T> {
        while self.idx < self.cap {
            if self.states[self.idx] == 1 {
                let r: &T = &self.items[self.idx];
                self.idx = self.idx + 1;
                return Option::Some(r);
            }
            self.idx = self.idx + 1;
        }
        Option::None
    }
}

// ===== V5d HashSet 运算符糖（2026-09-02） =====
// `|`/`&`/`-`/`^` 经运算符重载降级为集合代数方法（V5b）。`self` 按值消费（与
// Rust std::ops 一致），内部 `union`/`intersection`/`difference`/`symmetric_difference`
// 经 `&self` 自动借用读原集、返回全新集合。关系运算符 `<`/`>`（子集/超集）经
// 比较链独立路径 + 运算符重载降级为命名方法（V5d+，2026-09-02）：`<`=真子集
// `<=`=子集 `>`=真超集 `>=`=超集；见下方 `impl PartialOrd for HashSet<T>`。
impl<T> HashSet<T>: BitOr {
    type Output = HashSet<T>;
    fn bitor(self, other: HashSet<T>) -> HashSet<T> { self.union(&other) }
}
impl<T> HashSet<T>: BitAnd {
    type Output = HashSet<T>;
    fn bitand(self, other: HashSet<T>) -> HashSet<T> { self.intersection(&other) }
}
impl<T> HashSet<T>: Sub {
    type Output = HashSet<T>;
    fn sub(self, other: HashSet<T>) -> HashSet<T> { self.difference(&other) }
}
impl<T> HashSet<T>: BitXor {
    type Output = HashSet<T>;
    fn bitxor(self, other: HashSet<T>) -> HashSet<T> { self.symmetric_difference(&other) }
}
// HashMap 按键集合运算符糖（V5d+，2026-09-02）：`|`=并集 `&`=交集，降级到上方
// `union`/`intersection` 命名方法（冲突键语义见方法注释）。
impl<K, V> HashMap<K, V>: BitOr {
    type Output = HashMap<K, V>;
    fn bitor(self, other: HashMap<K, V>) -> HashMap<K, V> { self.union(&other) }
}
impl<K, V> HashMap<K, V>: BitAnd {
    type Output = HashMap<K, V>;
    fn bitand(self, other: HashMap<K, V>) -> HashMap<K, V> { self.intersection(&other) }
}
impl<K, V> HashMap<K, V>: Sub {
    type Output = HashMap<K, V>;
    fn sub(self, other: HashMap<K, V>) -> HashMap<K, V> { self.difference(&other) }
}
impl<K, V> HashMap<K, V>: BitXor {
    type Output = HashMap<K, V>;
    fn bitxor(self, other: HashMap<K, V>) -> HashMap<K, V> { self.symmetric_difference(&other) }
}
// 关系运算符 `<`/`>` 子集/超集（V5d+，2026-09-02）：走比较链独立路径，经运算符
// 重载降级为集合关系命名方法（与 Python `set` 语义一致：`<`=真子集 `<=`=子集
// `>`=真超集 `>=`=超集）。方法按引用（`&self`/`&other`）以避免 `a < b < c` 链式
// 复用操作数时的二次 move。
impl<T> HashSet<T>: PartialOrd {
    fn lt(&self, other: &HashSet<T>) -> bool { self.is_proper_subset(other) }
    fn le(&self, other: &HashSet<T>) -> bool { self.is_subset(other) }
    fn gt(&self, other: &HashSet<T>) -> bool { self.is_proper_superset(other) }
    fn ge(&self, other: &HashSet<T>) -> bool { self.is_superset(other) }
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

    // V5c：只读引用迭代器——返回 `HashSetIter<T>`（零拷贝视图，next() -> Option<&T>）。
    // 迭代期间不得对集合做结构性修改（指针悬垂）。
    fn iter(&self) -> HashSetIter<T> {
        let sp: *const i64 = &self.states[0];
        let ip: *const T = &self.items[0];
        HashSetIter { states: sp, items: ip, idx: 0, cap: self.cap }
    }

    // ===== V5b 集合运算（对标 Python set，零新增语言特性） =====
    // 返回新集合：并集 A ∪ B
    fn union(&self, other: &HashSet<T>) -> HashSet<T> {
        let mut r: HashSet<T> = HashSet::new();
        let a = self.elements();
        let mut i = 0;
        while i < a.len() {
            r.insert(a[i]);
            i = i + 1;
        }
        let b = other.elements();
        let mut j = 0;
        while j < b.len() {
            r.insert(b[j]);
            j = j + 1;
        }
        r
    }
    // 返回新集合：交集 A ∩ B
    fn intersection(&self, other: &HashSet<T>) -> HashSet<T> {
        let mut r: HashSet<T> = HashSet::new();
        let a = self.elements();
        let mut i = 0;
        while i < a.len() {
            if other.contains(a[i]) {
                r.insert(a[i]);
            }
            i = i + 1;
        }
        r
    }
    // 返回新集合：差集 A - B
    fn difference(&self, other: &HashSet<T>) -> HashSet<T> {
        let mut r: HashSet<T> = HashSet::new();
        let a = self.elements();
        let mut i = 0;
        while i < a.len() {
            if !other.contains(a[i]) {
                r.insert(a[i]);
            }
            i = i + 1;
        }
        r
    }
    // 返回新集合：对称差 A Δ B = (A-B) ∪ (B-A)
    fn symmetric_difference(&self, other: &HashSet<T>) -> HashSet<T> {
        let mut r: HashSet<T> = HashSet::new();
        let a = self.elements();
        let mut i = 0;
        while i < a.len() {
            if !other.contains(a[i]) {
                r.insert(a[i]);
            }
            i = i + 1;
        }
        let b = other.elements();
        let mut j = 0;
        while j < b.len() {
            if !self.contains(b[j]) {
                r.insert(b[j]);
            }
            j = j + 1;
        }
        r
    }
    // 关系：A ⊆ B
    fn is_subset(&self, other: &HashSet<T>) -> bool {
        let a = self.elements();
        let mut i = 0;
        while i < a.len() {
            if !other.contains(a[i]) {
                return false;
            }
            i = i + 1;
        }
        true
    }
    // 关系：A ⊇ B
    fn is_superset(&self, other: &HashSet<T>) -> bool {
        other.is_subset(self)
    }
    // 关系：A ⊂ B（真子集：A⊆B 且 |A|<|B|）
    fn is_proper_subset(&self, other: &HashSet<T>) -> bool {
        if self.len() < other.len() {
            self.is_subset(other)
        } else {
            false
        }
    }
    // 关系：A ⊃ B（真超集：A⊇B 且 |A|>|B|）
    fn is_proper_superset(&self, other: &HashSet<T>) -> bool {
        if self.len() > other.len() {
            self.is_superset(other)
        } else {
            false
        }
    }
    // 关系：A ∩ B = ∅
    fn is_disjoint(&self, other: &HashSet<T>) -> bool {
        let a = self.elements();
        let mut i = 0;
        while i < a.len() {
            if other.contains(a[i]) {
                return false;
            }
            i = i + 1;
        }
        true
    }
    // 原地：A |= B
    fn union_with(&mut self, other: &HashSet<T>) {
        let b = other.elements();
        let mut j = 0;
        while j < b.len() {
            self.insert(b[j]);
            j = j + 1;
        }
    }
    // 原地：A &= B
    fn intersect_with(&mut self, other: &HashSet<T>) {
        let a = self.elements();
        self.clear();
        let mut i = 0;
        while i < a.len() {
            if other.contains(a[i]) {
                self.insert(a[i]);
            }
            i = i + 1;
        }
    }
    // 原地：A -= B
    fn difference_with(&mut self, other: &HashSet<T>) {
        let b = other.elements();
        let mut j = 0;
        while j < b.len() {
            self.remove(b[j]);
            j = j + 1;
        }
    }
    // 原地：A ^= B
    fn symmetric_with(&mut self, other: &HashSet<T>) {
        let r = self.symmetric_difference(other);
        let elems = r.elements();
        self.clear();
        let mut i = 0;
        while i < elems.len() {
            self.insert(elems[i]);
            i = i + 1;
        }
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
