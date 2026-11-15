// 无堆密码学子集示例（GM/T 0028 风格的内存约束 PoC）：
// 仅使用标量 / 定长数组 / 栈上 struct，无 String/Vec/Box/format!/region。
// flag: --no-heap
struct State {
    a: i64,
    b: i64,
    c: i64,
    d: i64,
}

fn mix(x: i64, k: i64) -> i64 {
    (x << 5) ^ (x >> 3) ^ k
}

fn round(s: &mut State, k: &[i64; 4]) {
    let t0 = s.a + mix(s.b, k[0]);
    let t1 = s.b ^ mix(s.c, k[1]);
    let t2 = s.c + mix(s.d, k[2]);
    let t3 = s.d ^ mix(s.a, k[3]);
    s.a = t0;
    s.b = t1;
    s.c = t2;
    s.d = t3;
}

fn main() {
    let mut key: [i64; 4] = [0x1234, 0x5678, 0x9abc, 0xdef0];
    let mut s = State { a: 1, b: 2, c: 3, d: 4 };
    let mut i = 0;
    while i < 64 {
        round(&mut s, &key);
        key[0] = key[0] + 1;
        i = i + 1;
    }
    println(s.a);
    println(s.b);
    println(s.c);
    println(s.d);
}
