// Y4a：泛型 struct 字面量构造推断（语言级增强，construct.rs）
// `MutexGuard<T> { value: T }` 无 turbofish 时从字段实参推断 T（T = i64/String）。
// 此前报 `expects T, found i64`（lang-defects.md #1）；Y4a 修复后直接推断。

struct MutexGuard<T> { value: T }

impl<T> MutexGuard<T> {
    fn get(&self) -> T {
        let g: T = self.value;
        g
    }
}

fn main() {
    // 1. 泛型推断：i64
    let g1 = MutexGuard { value: 42 };
    println(g1.get()); // 42
    // 2. 泛型推断：String
    let g2 = MutexGuard { value: String::from("hi") };
    println(g2.get()); // hi
    // 3. 显式 turbofish 仍可用
    let g3: MutexGuard<i64> = MutexGuard { value: 7 };
    println(g3.get()); // 7
}
