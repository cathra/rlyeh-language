# 8. 泛型与单态化

泛型通过单态化实现：每个具体类型实例化生成独立代码。

```rlyeh
struct Wrapper<T> {
    value: T,
}
impl<T> Wrapper<T> {
    fn new(v: T) -> Wrapper<T> { Wrapper { value: v } }
}

trait Area { fn area(&self) -> f64; }
impl Area for Shape { fn area(&self) -> f64 { /* ... */ } }

// 泛型 trait 约束（bound / where，✅）
trait Convert<T> {
    fn convert(&self) -> T;
}
impl Convert<i64> for f64 {
    fn convert(&self) -> i64 { *self as i64 }
}
```

- 泛型 impl 单态化后 `Self` 替换为具体类型（`Wrapper<i64>::new(v) -> Self` → `Wrapper<i64>`）。
- MVP 限制：`Self` 仅支持**返回位置**——参数位置（关联返回）保持禁止；dyn 场景含 `Self` 签名方法不可经 vtable 调用（H4 既有限制）；`From::from(v) -> Self` / `Into::into() -> Self` / `Deserialize::from_json(s) -> Self` 落地待 std trait 声明。
- 类型联合 `A | B`（U1/U2 ✅）：成员两两不相交，`match` 用类型臂解构（`i64 => ...`）。
- 枚举显式判别式（U3 ✅）：`enum Code { Ok = 200, NotFound = 404 }`，判别值即变体 tag。

---

[← 上一章：模块与可见性](./07-modules.md) | [返回手册目录](./index.md) | [下一章：内存模型 →](./09-memory.md)
