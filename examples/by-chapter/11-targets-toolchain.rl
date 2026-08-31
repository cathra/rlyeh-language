// 对应 docs/guide/11-targets-toolchain.md —— 编译目标与工具链
// 本章重点是工具链命令；此文件用于演示 rlyeh build --target 跨平台编译。
// 原生编译运行：rlyeh run 11-targets-toolchain.rl
// 交叉编译（按平台替换 triple）：
//   rlyeh build 11-targets-toolchain.rl --target x86_64-apple-darwin -o app
//   rlyeh build 11-targets-toolchain.rl --target wasm32-wasip1 -o app.wasm
fn main() {
    println("cross-compile me with --target");
}
