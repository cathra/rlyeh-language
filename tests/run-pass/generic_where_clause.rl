// SH-P1-1（0.2.0-A）A4：泛型约束收尾——**函数 / 方法级 `where` 子句**。
//
// 此前 `where` 仅 impl 块支持（`parse_impl` 末尾调用 `parse_where_clause`），
// 函数声明遇到 `where` 直接报「expected ';' or '{', found Where」——函数级约束
// 只能写成内联 `fn f<T: Bound>(..)`。本项把 `parse_where_clause` 接入
// `parse_fn`（返回类型之后、函数体之前），约束按参数名合并进 `generics`，
// 复用既有 bound 校验路径（与内联 bound 同一套诊断）。

protocol Speak {
    fn speak(&self) -> i64;
}

protocol Named {
    fn name_id(&self) -> i64;
}

struct Dog { id: i64 }

impl Dog: Speak {
    fn speak(&self) -> i64 { self.id }
}

impl Dog: Named {
    fn name_id(&self) -> i64 { 100 }
}

// 1) 函数级 where（单约束）
fn loud<T>(x: T) -> i64 where T: Speak { x.speak() }

// 2) 函数级 where（多约束）
fn both<T>(x: T) -> i64 where T: Speak + Named { x.speak() + x.name_id() }

// 3) 内联 bound 与 where 混用
fn mixed<T: Speak, U>(x: T, y: U) -> i64 where U: Named { x.speak() + y.name_id() }

// 4) impl 块方法上的 where（同走 parse_fn）
struct Helper { k: i64 }

impl Helper {
    fn use_it<T>(&self, x: T) -> i64 where T: Speak { x.speak() + self.k }
}

// 5) impl 块级 where（既有能力回归）：约束 impl 的泛型参数。
//    注：impl 的类型参数由**接收者类型** unify 推导（`Pair<T>`），
//    protocol 类型实参不参与推导，故此处用无类型参数的 protocol。
struct Pair<T> { a: T }

protocol Wrap {
    fn wrap(&self) -> i64;
}

impl<T> Pair<T>: Wrap where T: Speak {
    fn wrap(&self) -> i64 { self.a.speak() }
}

fn main() {
    let d = Dog { id: 7 };
    println(loud(d));                        // 7
    println(both(d));                        // 107
    println(mixed(d, d));                    // 107

    let h = Helper { k: 1000 };
    println(h.use_it(d));                    // 1007

    let p: Pair<Dog> = Pair<Dog> { a: Dog { id: 5 } };
    println(p.wrap());                       // 5
}
