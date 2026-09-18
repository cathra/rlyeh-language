// SH-P0-8：`Drop` trait / 析构 / RAII（0.2.0-Q）。
//   Q-M1 `impl Drop for T` 识别（`Drop` 为编译器内置 trait，无需显式声明）
//   Q-M2 块尾自动插入 `x.drop()`——仅拥有所有权的绑定，按**逆声明序**

struct Resource { id: i64 }

impl Resource: Drop {
    fn drop(&mut self) {
        println(self.id);
    }
}

// 函数体局部变量在返回前析构
fn scoped() {
    let r = Resource { id: 7 };
    println(70);
}

// 块值先计算、后析构：尾表达式的值在析构前已存入临时
fn value_then_drop() -> i64 {
    let r = Resource { id: 8 };
    r.id
}

// Q-M3 字段级递归：自身无 Drop 的 struct 按**逆字段序**析构其拥有字段
struct Outer { a: Resource, b: Resource }

// Q-M3 自身有 Drop 的 struct：先 `drop()`，再析构字段（drop glue，同 Rust）
struct Both { tag: i64, r: Resource }

impl Both: Drop {
    fn drop(&mut self) {
        println(self.tag);
    }
}

// Q-M3 多层嵌套：字段递归逐层下探
struct Mid { o: Outer }

fn main() {
    // 1) 同一块内逆声明序析构（后声明者先析构）
    {
        let a = Resource { id: 1 };
        let b = Resource { id: 2 };
        println(100);                  // 100
    }                                  // 2, 1

    // 2) 嵌套块各自在块尾析构（词法作用域，与 Rust 一致）
    {
        let c = Resource { id: 3 };
        {
            let d = Resource { id: 4 };
        }                              // 4
    }                                  // 3

    // 3) 块值先求，再析构，最后以保存的值作为块结果
    let v = {
        let e = Resource { id: 5 };
        e.id
    };
    println(v);                        // 5

    // 4) 函数体局部变量在返回前析构
    scoped();                          // 70, 7

    // 5) 引用绑定不拥有所有权，不触发析构（只有 `f` 析构一次）
    {
        let f = Resource { id: 6 };
        let g = &f;
        println(g.id);                 // 6
    }                                  // 6（来自 f，g 跳过）

    // 6) 带返回值函数的析构时序
    println(value_then_drop());        // 8, 8

    // 7) Q-M3 逆字段序析构（Outer 自身无 Drop）
    {
        let o = Outer { a: Resource { id: 11 }, b: Resource { id: 12 } };
        println(1100);                 // 1100
    }                                  // 12, 11

    // 8) Q-M3 先 drop() 再析构字段（drop glue）
    {
        let b = Both { tag: 13, r: Resource { id: 14 } };
        println(1300);                 // 1300
    }                                  // 13, 14

    // 9) Q-M3 多层嵌套字段递归
    {
        let m = Mid { o: Outer { a: Resource { id: 15 }, b: Resource { id: 16 } } };
        println(1500);                 // 1500
    }                                  // 16, 15
}
