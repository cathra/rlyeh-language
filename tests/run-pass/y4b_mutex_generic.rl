// Y4b-2：泛型 Mutex<T> + guard Deref 解引用（std sync 泛型化原型）
// 语言级能力（2026-08-28 完成）：
//   - Y4a：泛型 struct 字面量构造推断（Mutex { value: 42 } → T = i64）
//   - Y4b-1：* 解引用支持自定义 Deref<T> protocol（guard.deref() 分派）
//   - Y4b-2：泛型 impl 静态方法 new 从实参推断 T（MyMutex::new(42)）
// std sync/module.rl 的 Mutex 泛型化为破坏性改动（波及 core.rl/guard/driver），
// 本测试用独立原型验证能力，std 迁移待专项。

protocol Deref<T> {
    fn deref(&self) -> T;
}

struct MutexGuard<T> { p: i64, value: T }

impl<T> MutexGuard<T>: Deref<T> {
    fn deref(&self) -> T {
        let g: T = self.value;
        g
    }
}

struct MyMutex<T> { p: i64, value: T }

impl<T> MyMutex<T> {
    fn new(v: T) -> MyMutex<T> {
        MyMutex { p: 0, value: v }
    }
    fn lock(&self) -> MutexGuard<T> {
        let val: T = self.value;
        MutexGuard { p: self.p, value: val }
    }
}

fn main() {
    // 1. 泛型 Mutex::new + lock + *guard 解引用（i64）
    let m1 = MyMutex::new(42);
    println(*m1.lock()); // 42
    // 2. String 数据
    let m2 = MyMutex::new(String::from("hi"));
    println(*m2.lock()); // hi
}
