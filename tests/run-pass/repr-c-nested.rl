// SH-P0-1 E2：repr(C) 嵌套结构体字段内联打包（嵌套聚合按 C 规则内联）。
// 布局：Point{x:i32,y:i32} size=8 align=4；
//       Line{ a:Point, b:i32, tag:u8 } => a@0(8) b@8(4) tag@12(1) => size 16 (align 4)
//       Scene{ line:Line, extra:i32 } => line@0(16) extra@16(4) => size 20 (align 4)
#[repr(C)]
struct Point { x: i32, y: i32 }

#[repr(C)]
struct Line {
    a: Point,
    b: i32,
    tag: u8,
}

#[repr(C)]
struct Scene {
    line: Line,
    extra: i32,
}

fn main() {
    let mut l = Line { a: Point { x: 1, y: 2 }, b: 3, tag: 4 };
    println(l.a.x);   // 1
    println(l.a.y);   // 2
    println(l.b);     // 3
    println(l.tag);   // 4
    // 整体赋值嵌套字段（memcpy 内联）
    l.a = Point { x: 7, y: 8 };
    println(l.a.x);   // 7
    println(l.a.y);   // 8
    // 子对象取子指针后访问
    let q = l.a;
    println(q.x);     // 7
    println(q.y);     // 8
    // 三层嵌套下降 Scene -> Line -> Point
    let s = Scene { line: Line { a: Point { x: 11, y: 22 }, b: 33, tag: 44 }, extra: 55 };
    println(s.line.a.x);  // 11
    println(s.line.a.y);  // 22
    println(s.line.b);    // 33
    println(s.line.tag);  // 44
    println(s.extra);     // 55
    unsafe {
        let p = &l as *const Line as *const u8;
        println(*(p + 0) as u32);   // a.x = 7
        println(*(p + 4) as u32);   // a.y = 8（证明嵌套内联、y 在偏移 4）
        println(*(p + 8) as u32);   // b = 3
        println(*(p + 12) as u32);  // tag = 4
        let sp = &s as *const Scene as *const u8;
        println(*(sp + 0) as u32);    // line.a.x = 11
        println(*(sp + 4) as u32);    // line.a.y = 22
        println(*(sp + 8) as u32);    // line.b = 33
        println(*(sp + 12) as u32);   // line.tag = 44
        println(*(sp + 16) as u32);   // extra = 55
    }
}
