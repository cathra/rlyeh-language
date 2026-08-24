# wordfreq — 文本词频统计（Zeta）

纯 Zeta 实现的词频统计器：读取文本文件 → 切分单词 → 统计频次 →
按频次排序 → 终端排行表 + 写回 `freq.txt`。

## 构建

```sh
cd examples/projects/wordfreq
scripts/build.sh        # 产出 wordfreq
```

（需先在仓库根 `cargo build --release` 构建编译器。）

## 运行

```sh
scripts/run.sh          # 读取 ./sample.txt，输出排行并写 ./freq.txt
```

输出示例（`sample.txt`，可自行替换文本）：

```
=== wordfreq: sample.txt ===
total words: 59
unique words: 39
  rank  word                  count
   1  the                      8
   2  of                       4
   3  hello                    3
   4  words                    3
   5  zeta                     3
   6  and                      2
   7  brown                    2
   8  fox                      2
   9  quick                    2
  10  a                        1
  ...
```

## 架构

```
tokenizer.zeta  tokenize：按非字母字符切分，ASCII 大小写归一（转小写）
stats.zeta      count_freq：词 → 频次表；ranked：频次降序排行条目
main.zeta       入口：读 sample.txt → 统计 → 表格输出 → 写 freq.txt
```

## Zeta 语言亮点（本项目用到）

- **文件 IO**：`read_file` / `write_file`（io::file 内建，fopen/fread 封装）
- **泛型集合**：`HashMap<String, i64>` 统计频次，`keys()` + `get`（值拷贝）遍历
- **泛型排序**：`Vec::sort_by(cmp: fn(T, T) -> i64)` —— 比较器为函数指针
  （`WordFreq` 自定义结构体按值传参，降序 + 同频字典序二级排序）
- **字符串方法**：`chars()`（字节级）、`len()`、`push_byte`、`clone`、`+` 拼接、
  `String::from` / `int_to_string`（i64 → 十进制）
- **比较链**：`if 65 <= c <= 90`（ASCII 字母范围判断）
- **模块化**：三个平铺模块 + 逐条 `use` 导入（结构体 `WordFreq`、函数均跨模块引用）

## 已知 MVP 限制

- **无命令行参数 API**：输入文件硬编码为 `sample.txt`（相对运行时 cwd），
  修改后重新编译；换文本直接覆盖该文件即可
- **ASCII 字符集**：切词仅识别 ASCII 字母，UTF-8 非 ASCII 视为分隔符
- **`read_file` 单次读满**：普通文件一次 `fread` 读入（fseek+ftell 得大小）
