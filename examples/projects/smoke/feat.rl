// smoke4：项目依赖特性综合验证
// dyn Trait / region adaptive / ? 运算符 / HashMap / sort_by / 闭包 / 数值区间

protocol Shape {
    fn area(&self) -> f64;
}
struct Circle { radius: f64 }
impl Circle: Shape {
    fn area(&self) -> f64 {
        let r = self.radius;
        let r2 = r * r;
        let pi = 3.14;
        pi * r2
    }
}

fn load(path: String) -> Result<String, IoError> {
    read_file(path)
}

fn main() {
    // dyn Trait 分派（局部绑定）
    let c = Circle { radius: 2.0 };
    let d: dyn Shape = &c;
    println(d.area());                         // 12.56

    // region adaptive 批量分配
    region 'r adaptive {
        let mut total = 0;
        let mut i = 0;
        while i < 100 {
            let v1 = Circle { radius: 1.0 } in 'r;
            total = total + 1;
            i = i + 1;
        }
        println(total);                        // 100
    }

    // ? 运算符 + 文件读取
    let content = match load(String::from("examples/projects/smoke/main.rl")) {
        Result::Ok(text) => text,
        Result::Err(e) => String::from("ERR"),
    };
    println(content.len() > 0);                // true

    // HashMap + 遍历
    let mut m: HashMap<String, i64> = HashMap::new();
    m.insert(String::from("b"), 2);
    m.insert(String::from("a"), 1);
    m.insert(String::from("c"), 3);
    println(m.len());                          // 3
    let mut sum = 0;
    for (k, val2) in m {
        sum = sum + val2;
    }
    println(sum);                              // 6

    // Vec<String> sort + sort_by（闭包）
    let mut vs: Vec<String> = vec![String::from("pear"), String::from("apple"), String::from("banana")];
    vs.sort();
    println(vs[0]);                            // apple
    let mut nums: Vec<i64> = vec![3, 1, 2];
    nums.sort();
    println(nums[0]);                          // 1

    // 数值区间 for
    let mut s = 0;
    for i in 0..<10 {
        s = s + i;
    }
    println(s);                                // 45
}
