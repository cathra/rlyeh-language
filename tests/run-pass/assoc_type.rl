// U2 protocol 关联类型（assoc type）：
// `protocol T { type Item; ... }` 声明 + `impl T for X { type Item = Concrete; }`
// 定义；impl 方法签名中 `Self::Item` 在收集期替换为具体类型。
protocol Container {
    type Item;
    type Label;
    fn get(&self) -> Self::Item;
    fn set(&mut self, v: Self::Item);
    fn label_of(&self, v: Self::Item) -> Self::Label;
    fn label(&self) -> String;
}

struct Box { value: i64 }
impl Box: Container {
    type Item = i64;
    type Label = String;
    fn get(&self) -> Self::Item { self.value }
    fn set(&mut self, v: Self::Item) { self.value = v; }
    fn label_of(&self, v: Self::Item) -> Self::Label {
        if v > 10 {
            String::from("big")
        } else {
            String::from("small")
        }
    }
    fn label(&self) -> String { String::from("box") }
}

struct Bag { name: String }
impl Bag: Container {
    type Item = String;
    type Label = String;
    fn get(&self) -> Self::Item { self.name.clone() }
    fn set(&mut self, v: Self::Item) { self.name = v; }
    fn label_of(&self, v: Self::Item) -> Self::Label {
        if v.len() > 3 {
            String::from("long")
        } else {
            String::from("short")
        }
    }
    fn label(&self) -> String { String::from("bag") }
}

fn main() {
    // Item = i64：返回 / 参数位置均替换为具体类型
    let mut b = Box { value: 42 };
    println(b.get());              // 42
    b.set(99);
    println(b.get());              // 99
    println(b.label_of(7));        // small
    println(b.label_of(100));      // big
    println(b.label());            // box

    // Item = String：关联类型替换为 String
    let mut g = Bag { name: String::from("hello") };
    println(g.get());              // hello
    g.set(String::from("zeta"));
    println(g.get());              // zeta
    println(g.label_of(String::from("abc")));   // short
    println(g.label_of(String::from("abcdef"))); // long
    println(g.label());            // bag

    // H4 dyn 去虚拟化：dyn 局部变量绑定源具体类型已知时静态分派，
    // 关联类型签名方法可用（vtable 路径 MVP 报 Unsupported）
    let d: dyn Container = &b;
    println(d.get());              // 99
}
