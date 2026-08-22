// K4 追踪 GC：Gc::new / 解引用 / 自动剥层 / gc_region 生命周期 / 逃逸对象 / 嵌套块
struct Point {
    x: i64,
    y: i64,
}

struct Holder {
    v: i64,
    g: Gc<i64>,
}

fn main() {
    // 1. gc_region 内分配 + 解引用（块结束触发 GC 周期，未逃逸对象回收）
    gc_region {
        let a = Gc::new(42);
        println(*a); // 42
        let b = Gc::new(7);
        println(*a + *b); // 49
    }

    // 2. 逃逸对象：块返回值 Gc<T> 存活到块外
    let g = gc_region {
        let inner = Gc::new(100);
        println(*inner); // 100
        inner
    };
    println(*g); // 100

    // 3. 字段剥层（Gc<Point>）
    let gp = gc_region { Gc::new(Point { x: 5, y: 6 }) };
    println(gp.x + gp.y); // 11

    // 4. 方法 / 索引剥层（Gc<String>）
    let gs = gc_region { Gc::new(String::from("hello")) };
    println(gs.len()); // 5
    println(gs[0]); // 104

    // 5. 嵌套 gc_region：内层逃逸对象在外层块内仍活跃（存活链式提升）
    let outer = gc_region {
        let o = Gc::new(1000);
        let saved = gc_region {
            let ii = Gc::new(2000); // 注意：不能与后续 `for i` 不同类型的遮蔽（LIR 限制）
            let o2 = o; // 外层对象指针拷贝（epoch 旧，内层收集不回收）
            ii
        };
        println(*saved); // 2000
        o
    };
    println(*outer); // 1000
    println(*g); // 100（早前逃逸对象仍活跃）

    // 6. 重复收集：循环内块分配 + 使用（回收不崩溃）
    let mut sum = 0;
    for i in 0..<3 {
        gc_region {
            let x = Gc::new(i);
            let y = Gc::new(i * 10);
            sum += *x + *y;
        }
    }
    println(sum); // 0 + 11 + 22 = 33

    // 7. Gc 字段剥层（块内聚合引用 Gc 字段，未逃逸一并回收）
    gc_region {
        let g1 = Gc::new(7);
        let h = Holder { v: 1, g: g1 };
        println(h.v + *h.g); // 8
    }

    // 8. 块外分配（epoch 0，永不回收——MVP 泄漏语义，仍可用）
    let leaky = Gc::new(999);
    println(*leaky); // 999
    gc_region {
        let t = Gc::new(1);
        println(*leaky + *t); // 1000
    }
    println(*leaky); // 999（后续块收集不影响块外对象）
}
