// ===== wordfreq 入口：文本词频统计 =====
//
// 读取 sample.txt → 切词 → 统计频次 → 按频次排序 → 终端表格 + 写 freq.txt。
// MVP 无命令行参数 API：输入文件硬编码为 "sample.txt"（相对运行时 cwd）。

module tokenizer;
module stats;

import tokenizer::tokenize;
import stats::count_freq;
import stats::ranked;
import stats::WordFreq;

fn main() {
    let text = match read_file(String::from("sample.txt")) {
        Result::Ok(t) => t,
        Result::Err(e) => {
            println("cannot read sample.txt (run in examples/projects/wordfreq)");
            return
        }
    };

    let words = tokenize(text);
    let total = words.len();
    let freq = count_freq(words);
    let entries = ranked(&freq);
    let unique = entries.len();

    let title = String::from("=== wordfreq: sample.txt ===");
    println(title);
    let tline = String::from("total words: ") + int_to_string(total);
    println(tline);
    let uline = String::from("unique words: ") + int_to_string(unique);
    println(uline);
    println(String::from("  rank  word                  count"));

    let mut body = String::new();
    body.push_str(title.clone() + "\n");
    body.push_str(tline.clone() + "\n");
    body.push_str(uline.clone() + "\n");
    body.push_str(String::from("  rank  word                  count\n"));

    let m = entries.len();
    let mut i = 0;
    while i < m {
        let e = entries[i];
        let rank = i + 1;
        let line = pad(int_to_string(rank), 4) + String::from("  ") + pad_right(e.word.clone(), 20) + String::from("  ") + pad(int_to_string(e.count), 4);
        println(line.clone());
        body.push_str(line + "\n");
        i = i + 1;
    }

    let _ = write_file(String::from("freq.txt"), body);
    println(String::from("wrote freq.txt"));
}

// 左填充（右对齐）：宽度不足补空格，超宽原样返回
fn pad(s: String, w: i64) -> String {
    let n = s.len();
    let mut buf = String::new();
    let mut sp = 0;
    while sp < w - n {
        buf.push_byte(32);
        sp = sp + 1;
    }
    buf + s
}

// 右填充（左对齐）：文本后补空格到宽度
fn pad_right(s: String, w: i64) -> String {
    let n = s.len();
    let mut buf = s;
    let mut sp = 0;
    while sp < w - n {
        buf.push_byte(32);
        sp = sp + 1;
    }
    buf
}
