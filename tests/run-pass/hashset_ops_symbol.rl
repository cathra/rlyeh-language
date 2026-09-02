// V5d HashSet 运算符糖（2026-09-02）：| & - ^ 经运算符重载降级为集合方法
struct Point { x: i64, y: i64 }
impl Add for Point {
    type Output = Point;
    fn add(self, other: Point) -> Point { Point { x: self.x + other.x, y: self.y + other.y } }
}

fn main() {
    // 并集 A ∪ B = {1,2,3,4}
    let mut a1: HashSet<i64> = HashSet::new();
    a1.insert(1); a1.insert(2); a1.insert(3);
    let mut b1: HashSet<i64> = HashSet::new();
    b1.insert(3); b1.insert(4);
    let u = a1 | b1;
    println(u.len());   // 4

    // 交集 A ∩ B = {3}
    let mut a2: HashSet<i64> = HashSet::new();
    a2.insert(1); a2.insert(2); a2.insert(3);
    let mut b2: HashSet<i64> = HashSet::new();
    b2.insert(3); b2.insert(4);
    let i = a2 & b2;
    println(i.len());   // 1

    // 差集 A \ B = {1,2}
    let mut a3: HashSet<i64> = HashSet::new();
    a3.insert(1); a3.insert(2); a3.insert(3);
    let mut b3: HashSet<i64> = HashSet::new();
    b3.insert(3); b3.insert(4);
    let d = a3 - b3;
    println(d.len());   // 2

    // 对称差 A △ B = {1,2,4}
    let mut a4: HashSet<i64> = HashSet::new();
    a4.insert(1); a4.insert(2); a4.insert(3);
    let mut b4: HashSet<i64> = HashSet::new();
    b4.insert(3); b4.insert(4);
    let x = a4 ^ b4;
    println(x.len());   // 3

    // 自定义类型算术重载（+）：Point 分量相加
    let p = Point { x: 1, y: 2 } + Point { x: 10, y: 20 };
    println(p.x);   // 11
    println(p.y);   // 22
}
