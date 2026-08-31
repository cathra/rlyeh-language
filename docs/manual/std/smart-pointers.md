# 智能指针（Box / Rc / Arc / Gc）

堆分配与所有权管理（K2/K3/K4 ✅）。`*` 解引用、字段/方法/索引自动剥层、引用计数（clone/强弱计数/弱引用/`try_unwrap`）。

## Box\<T\>（K2，独占堆分配）

```rlyeh
let b: Box<i64> = Box::new(42);
println(*b);                      // 42（解引用）
let p = Box::new(Point { x: 1, y: 2 });
println(p.x);                     // 字段访问自动剥层
```

- `Box::new(x: T) -> Box<T>`：在堆上分配并移动值。
- `*` 解引用；字段/方法/索引访问自动剥引用层。

## Rc\<T\>（K3，单线程引用计数）

```rlyeh
let a: Rc<i64> = Rc::new(42);
let b = a.clone();                // 强引用 +1
println(*a);                      // 42
let weak = Rc::downgrade(a);      // 弱引用
match Rc::upgrade(weak) {
    Some(v) => println(*v),
    None => println(0),
}
```

- `Rc::new(x: T) -> Rc<T>`
- `clone() -> Rc<T>`：强引用计数 +1
- `Rc::downgrade(rc) -> Weak<T>`：生成弱引用
- `Rc::upgrade(weak) -> Option<Rc<T>>`：尝试升级（原对象已释放则 `None`）
- `Rc::try_unwrap(rc) -> Option<T>`：引用计数为 1 时取出所有权，否则 `None`

## Arc\<T\>（K3，原子引用计数，多线程安全）

API 与 `Rc` 对称，但计数操作为原子指令，可跨线程共享。
```rlyeh
let a: Arc<i64> = Arc::new(42);
let b = a.clone();                // 原子强引用 +1
```

## Gc\<T\>（K4，可选 GC）

保守标记-清除垃圾回收，配合 `gc_region` 块与逃逸 root 使用（`rlyeh-gc-runtime`）。
```rlyeh
gc_region {
    let g: Gc<Node> = Gc::new(Node { value: 1 });
    // 块结束时由 GC 回收（逃逸分析 + 标记-清除）
}
```

- `Gc::new(x: T) -> Gc<T>`
- 与 `Box`/`Rc` 一致的 `*` 解引用、字段/方法自动剥层。

## 完整示例

```rlyeh
fn main() {
    let b: Box<i64> = Box::new(10);
    println(*b);

    let r: Rc<i64> = Rc::new(20);
    let r2 = r.clone();
    println(*r + *r2);

    gc_region {
        let g: Gc<i64> = Gc::new(30);
        println(*g);
    }
}
```

---

[← 返回标准库详述索引](./index.md)
