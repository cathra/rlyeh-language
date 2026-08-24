// 基准目录共享 module：各子目录 .go 文件独立 `go build <file>.go` 单文件编译，
// 无外部依赖（go 1.21+ GO111MODULE=on 上下文必需）。
module zeta-benchmarks

go 1.21
