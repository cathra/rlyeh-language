// U1 块级变量遮蔽（block-scoped shadowing）：
// 块内 `let x` 覆盖外层同名变量，块内引用指向新绑定，块结束后原变量恢复。
fn main() {
    // 1. 基础块遮蔽：块内 let 遮蔽外层，块后恢复
    let x = 10;
    {
        let x = 20;
        println(x);   // 20
    };
    println(x);       // 10

    // 2. 多层嵌套遮蔽
    let y = 1;
    {
        let y = 2;
        {
            let y = 3;
            println(y);   // 3
        };
        println(y);       // 2
    };
    println(y);           // 1

    // 3. 遮蔽初始化引用外层同名变量
    let a = 5;
    {
        let a = a + 1;
        println(a);       // 6
    };
    println(a);           // 5

    // 4. match 臂内遮蔽
    let m = 7;
    match m {
        7 => {
            let m = 8;
            println(m);   // 8
        }
        _ => println(0),
    }
    println(m);           // 7

    // 5. while 循环体内遮蔽外层变量，循环外恢复
    let i = 100;
    let mut n = 0;
    while n < 2 {
        let i = n * 10;
        println(i);   // 0 10
        n = n + 1;
    }
    println(i);       // 100
    println(n);       // 2

    // 6. for 循环变量遮蔽外层同名变量
    let j = 99;
    for j in 0..<3 {
        println(j);   // 0 1 2
    }
    println(j);       // 99

    // 7. 遮蔽 + 闭包捕获（块内遮蔽的变量被闭包捕获）
    let base = 10;
    let r = {
        let base = 20;
        (|x| x + base)(1)   // 21：捕获块内遮蔽的 base
    };
    println(r);       // 21
    println(base);    // 10
}
