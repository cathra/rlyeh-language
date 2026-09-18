// fs/path.rl：Path 对象（std-lib.md §4.3）。
// 目录化（2026-08）：由原 fs.rl 顶部拆分。符号完整路径 fs::path::Path，
// core.rl 经 `import fs::path::Path;` 重导出到根命名空间（裸名 Path 即用）。
// 实现说明：
// - `Path` 为纯字符串封装（无规范化/解析）；exists 用 POSIX access(F_OK)；
//   is_file 用 fopen 试探（无读权限文件误报，MVP）；is_dir = exists && !is_file 近似。

// Path 对象：路径字符串封装。
struct Path {
    path: String,
}

impl Path {
    // 构造 Path。
    fn new(path: String) -> fs::path::Path {
        fs::path::Path { path: path }
    }
    // 底层路径字符串。
    fn as_string(self) -> String {
        self.path
    }
    // 拼接路径段（自动处理 base 尾部斜杠）。
    fn join(self, other: String) -> fs::path::Path {
        let base = self.path;
        if base.len == 0 {
            return fs::path::Path { path: other };
        }
        if base.data[base.len - 1] == 47 {   // '/'
            return fs::path::Path { path: base + other };
        }
        fs::path::Path { path: base + String::from("/") + other }
    }
    // 父路径（去掉最后一段；无 '/' 返回 "."）。
    fn parent(self) -> fs::path::Path {
        let p = self.path;
        let mut i = p.len;
        // 跳过末尾斜杠
        while i > 0 && p.data[i - 1] == 47 {
            i = i - 1;
        }
        // 找上一个 '/'
        while i > 0 && p.data[i - 1] != 47 {
            i = i - 1;
        }
        if i == 0 {
            return fs::path::Path { path: String::from(".") };
        }
        // [0, i-1)（去掉末尾斜杠）
        let mut res = String::with_capacity(i - 1);
        let mut j = 0;
        while j < i - 1 {
            res.push_byte(p.data[j]);
            j = j + 1;
        }
        fs::path::Path { path: res }
    }
    // 文件名（最后一段；无 '/' 返回自身）。
    fn file_name(self) -> fs::path::Path {
        let p = self.path;
        let mut i = p.len;
        while i > 0 && p.data[i - 1] == 47 {
            i = i - 1;
        }
        let mut start = 0;
        let mut j = 0;
        while j < i {
            if p.data[j] == 47 {
                start = j + 1;
            }
            j = j + 1;
        }
        let mut res = String::with_capacity(i - start);
        let mut k = start;
        while k < i {
            res.push_byte(p.data[k]);
            k = k + 1;
        }
        fs::path::Path { path: res }
    }
    // 扩展名（文件名最后一个 '.' 之后；无 '.' 返回空串）。
    fn path_extension(self) -> String {
        let name = self.file_name();
        let n = name.path;
        let mut dot = n.len;
        let mut i = 0;
        while i < n.len {
            if n.data[i] == 46 {   // '.'
                dot = i;
            }
            i = i + 1;
        }
        if dot == n.len {
            return String::from("");
        }
        let mut res = String::with_capacity(n.len - dot - 1);
        let mut k = dot + 1;
        while k < n.len {
            res.push_byte(n.data[k]);
            k = k + 1;
        }
        res
    }
    // 路径是否存在（POSIX access F_OK=0）。
    fn exists(self) -> i64 {
        let r = access(c_str(self.path), 0);
        if r == 0 {
            return 1;
        }
        0
    }
    // 是普通文件（`test -f <path> && echo y`：POSIX test 正确区分文件/目录；
    // fopen 试探在 macOS 下目录可打开（目录流），不可用）。
    // MVP：依赖 /bin/sh，路径不含空格/特殊字符；WASI/Windows 不支持。
    fn is_file(self) -> i64 {
        let cmd = String::from("test -f ") + self.path + String::from(" && echo y");
        let f = popen(c_str(cmd), String::from("r"));
        if f == 0 {
            return 0;
        }
        let mut tmp = String::with_capacity(16);
        let n = fread(tmp, 1, 16, f);
        let _ = pclose(f);
        if n >= 1 && tmp.data[0] == 121 {   // 'y'
            return 1;
        }
        0
    }
    // 是目录（`test -d <path> && echo y`，同上依赖 shell）。
    fn is_dir(self) -> i64 {
        let cmd = String::from("test -d ") + self.path + String::from(" && echo y");
        let f = popen(c_str(cmd), String::from("r"));
        if f == 0 {
            return 0;
        }
        let mut tmp = String::with_capacity(16);
        let n = fread(tmp, 1, 16, f);
        let _ = pclose(f);
        if n >= 1 && tmp.data[0] == 121 {   // 'y'
            return 1;
        }
        0
    }
}
