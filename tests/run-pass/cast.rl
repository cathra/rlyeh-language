// U6 Cast IR：数值→数值转换（`as`）——输出与 cast.out 精确对比
// 覆盖：f64↔i64、整型截断/扩展（i8/u8 语义位宽）、整↔bool、整↔char
fn main() {
    // f64 → i64：向零截断（fptosi）
    println(3.7 as i64);        // 3
    println((-3.7) as i64);     // -3
    // i64 → f64（sitofp；%f 显示 6 位小数）
    println(5 as f64);          // 5.000000
    // i64 → i8：trunc + sext（符号扩展回 64 位槽）
    println(300 as i8);         // 44
    println((-300) as i8);      // -44
    // i64 → u8：trunc + zext（零扩展）
    println(300 as u8);         // 44
    println((-1) as u8);        // 255
    // i8 → i64：窄到宽透传（槽值已符号扩展）
    let a: i8 = -1;
    println(a as i64);          // -1
    // 整 ↔ bool
    println(5 as bool);         // true
    println(0 as bool);         // false
    println(true as i64);       // 1
    // 整 ↔ char
    println('a' as i64);        // 97
    let c = 66 as char;         // 'B'
    println(c);                 // B
    // 同类型恒等（typecheck 擦除，无指令）
    println(42 as i64);         // 42
}
