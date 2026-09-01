// SH-P0-1 E3：extern 函数调用须在 `unsafe` 块内。
// std 预置的 `calloc` 在 `unsafe` 块中调用放行（prelude 内部 FFI 豁免）。
fn main() {
    unsafe {
        let _p = calloc(1, 8);
    }
    println("ok");
}
