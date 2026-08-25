// J1 数组迭代：for x in arr（索引遍历循环，长度编译期已知）
fn main() {
    // 1. 数组字面量迭代
    let mut sum = 0;
    for x in [10, 20, 30] {
        sum += x;
    }
    println(sum);   // 60

    // 2. 数组变量迭代（类型注解 [i64; 4]）
    let arr: [i64; 4] = [1, 2, 3, 4];
    let mut prod = 1;
    for v in arr {
        prod *= v;
    }
    println(prod);  // 24

    // 3. 求最大值（元素聚合）
    let data = [3, 7, 2, 9, 5];
    let mut max = 0;
    for d in data {
        if d > max {
            max = d;
        }
    }
    println(max);   // 9

    // 4. break 提前退出
    let nums = [10, 20, 30, 40];
    let mut total = 0;
    for n in nums {
        if n >= 30 {
            break;
        }
        total += n;
    }
    println(total); // 30

    // 5. 嵌套数组循环
    let grid = [[1, 2], [3, 4]];
    let mut s = 0;
    for row in grid {
        for cell in row {
            s += cell;
        }
    }
    println(s);     // 10
}
