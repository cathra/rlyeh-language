// L3 region 选项接线：bump 分配 / with_size / strategy (bump) / adaptive /
// exact / allow_growth(growth_factor) / transfer / 匿名区域
struct Big {
    a: i64,
    b: i64,
    c: i64,
    d: i64,
}

fn main() {
    // 1. 默认区域：bump 分配 + 区域退出批量释放
    region 'r {
        let x = Big { a: 1, b: 2, c: 3, d: 4 } in 'r;
        println(x.a + x.b + x.c + x.d); // 10
    }

    // 2. with_size 显式初始容量
    region 'r2 with_size (64) {
        let y = Big { a: 10, b: 20, c: 30, d: 40 } in 'r2;
        println(y.a + y.b); // 30
    }

    // 3. strategy (bump) 显式 bump 策略
    region 'r3 strategy (bump) {
        let z = Big { a: 1, b: 1, c: 1, d: 1 } in 'r3;
        println(z.d); // 1
    }

    // 4. adaptive 自适应（EWMA 预测扩容）
    region 'r4 adaptive {
        let w = Big { a: 5, b: 5, c: 5, d: 5 } in 'r4;
        println(w.c); // 5
    }

    // 5. exact 精确容量（不扩容）
    region 'r5 with_size (64) exact {
        let v = Big { a: 7, b: 8, c: 9, d: 10 } in 'r5;
        println(v.a * v.d); // 70
    }

    // 6. allow_growth 倍率扩容
    region 'r6 with_size (16) allow_growth (growth_factor = 4.0) {
        let u = Big { a: 2, b: 3, c: 4, d: 5 } in 'r6;
        println(u.a + u.b + u.c + u.d); // 14
    }

    // 7. transfer 所有权移出区域（transfer 后该对象不可再访问）
    region 'r7 {
        let t = Big { a: 3, b: 3, c: 3, d: 3 } in 'r7;
        println(t.a); // 3（transfer 前使用）
        transfer t out of 'r7;
    }

    // 8. 匿名区域（块作用域，无 in）
    region {
        let q = Big { a: 1, b: 2, c: 3, d: 4 };
        println(q.a + q.b + q.c + q.d); // 10
    }

    // 9. 循环内多区域（bump 扩容路径）
    let mut sum = 0;
    for i in 0..<3 {
        region 'r9 {
            let o = Big { a: i, b: i * 2, c: i * 3, d: i * 4 } in 'r9;
            sum += o.a + o.b + o.c + o.d;
        }
    }
    println(sum); // 0 + 10 + 20 = 30
}
