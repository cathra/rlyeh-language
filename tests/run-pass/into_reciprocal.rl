// P6c-1/2：`Into::into` 经 blanket 语义实现（约束求解确认 `From` 存在后改写 `From::from`）。
// `Into::<IoError>::into(kind)` ≡ `From::from(kind)`，验证二者输出一致。

fn main() {
    let kind = io::error::IoErrorKind::TimedOut;

    // 1. Into::into（前缀 turbofish 指定目标类型）
    let e1 = Into::<io::error::IoError>::into(kind);
    println(e1.message());

    // 2. 与 From::from 等价（同一 From impl）
    let e2 = From::from(kind);
    println(e2.message());

    // 3. Into::into::<U>(x) 后置 turbofish 形式
    let e3 = Into::into::<io::error::IoError>(kind);
    println(e3.message());
}
