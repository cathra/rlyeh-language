// T-7：HIR 字段内引用悬垂——引用经聚合字段传播后，referent 被 transfer 移出，
// 内嵌引用 `w.inner` 指向已失效的槽位 → DanglingReference（BC005）。
// expect: `x` does not live long enough
struct Wrapper 'a { inner: &'a i64 }

fn main() {
    region 'r {
        let x = 5 in 'r;
        let w = Wrapper { inner: &x } in 'r;
        transfer x out of 'r;
        println(*w.inner);
    }
    println(0);
}
