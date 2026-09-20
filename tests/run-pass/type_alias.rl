// 类型别名回归测试：非泛型 / 链式 / pub / 泛型别名 + 别名套用泛型别名
type Int = i64;
type Num = Int;
pub type PubInt = i64;
type Name = String;
type Pair<T> = (T, T);
type PairI64 = Pair<i64>;

fn get_id() -> Int { 7 }
fn name_len(n: Name) -> i64 { n.len() }

fn main() {
    let x: Int = 42;
    println(x);                       // 42
    let y: Num = 100;
    println(y);                       // 100
    let s: Name = String::from("hello");
    println(s.len());                 // 5
    println(name_len(s));             // 5
    println(get_id());                // 7

    let p: Pair<String> = (String::from("a"), String::from("b"));
    println(p.f0.len() + p.f1.len()); // 2

    let q: PairI64 = (1, 2);
    println(q.f0 + q.f1);             // 3
}
