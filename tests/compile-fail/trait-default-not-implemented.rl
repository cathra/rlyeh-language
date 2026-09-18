// V3：trait 无默认方法（抽象方法）且 impl 未实现时，调用必须报错（不回退）
protocol Worker {
    fn work(&self) -> i64;
}

struct Machine {
    x: i64,
}

impl Machine: Worker {
    // 未实现 work（抽象方法必须实现）
}

fn main() {
    let m = Machine { x: 7 };
    println(m.work()); // 报错：FunctionNotFound（trait 无默认实现，不可回退）
}
