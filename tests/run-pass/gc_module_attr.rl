// B-6（P3）：模块级 GC 逃逸舱——`#[memory(gc)]` 使模块内 `&T` 默认映射为 `Gc<T>`，
// 指针图 / 树结构免写 `&`/`&mut`。本例在 gc 模块内：
//   - 结构体字段 `target: &i64` 等价于 `target: Gc<i64>`（构造用 `Gc::new`）；
//   - 函数形参 `h: &i64` 等价于 `h: Gc<i64>`（自动解引用取值）。
// 字段读取经 Gc 自动剥层，证明 parse→typecheck→codegen→run 全链路生效。

#[memory(gc)]
module graph {
    pub struct Handle {
        target: &i64,
    }

    pub fn read(h: &i64) -> i64 {
        *h
    }
}

fn main() -> i64 {
    let h = graph::Handle { target: Gc::new(42) };
    graph::read(h.target) + *(h.target)
}
