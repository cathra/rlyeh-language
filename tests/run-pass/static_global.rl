// 0.2.0-V M2：static / static mut 全局变量（data 段符号读写）
static VERSION: i64 = 42;
static GREETING: i64 = 40 + 8;   // 编译期常量表达式（字面量算术）
static mut COUNT: i64 = 0;

fn main() {
    println(VERSION);          // 42
    println(GREETING);         // 50
    unsafe {
        COUNT = 7;
        println(COUNT);        // 7
        COUNT = COUNT + 3;
        println(COUNT);        // 10
    };
}
