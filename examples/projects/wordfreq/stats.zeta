// ===== 词频统计 =====
//
// 单词列表 → 频次表（HashMap<String, i64>）→ 排行条目（Vec<WordFreq>）。

pub struct WordFreq {
    pub word: String,
    pub count: i64,
}

// 词 → 频次 表（遍历计数，get 返回 Option<i64> 值拷贝）
pub fn count_freq(words: Vec<String>) -> HashMap<String, i64> {
    let mut freq: HashMap<String, i64> = HashMap::new();
    let n = words.len();
    let mut i = 0;
    while i < n {
        let w = words[i];
        let g = freq.get(w.clone());
        let c = match g {
            Option::Some(v) => v,
            Option::None => 0,
        };
        freq.insert(w.clone(), c + 1);
        i = i + 1;
    }
    freq
}

// 频次表 → 排行条目（频次降序、同频按单词字典序；sort_by 比较器为函数指针）
pub fn ranked(freq: &HashMap<String, i64>) -> Vec<WordFreq> {
    let keys = freq.keys();
    let mut entries: Vec<WordFreq> = Vec::new();
    let n = keys.len();
    let mut i = 0;
    while i < n {
        let w = keys[i].clone();
        let g = freq.get(w.clone());
        let mut c = 0;
        match g {
            Option::Some(v) => {
                c = v;
            }
            Option::None => {}
        }
        entries.push(WordFreq { word: w, count: c });
        i = i + 1;
    }
    entries.sort_by(cmp_freq_desc);
    entries
}

// 排序比较器：频次降序（b - a < 0 表示 a 在前）；同频按单词升序（字典序）
pub fn cmp_freq_desc(a: WordFreq, b: WordFreq) -> i64 {
    if b.count != a.count {
        return b.count - a.count;
    }
    if a.word < b.word {
        return 0 - 1;
    }
    if a.word == b.word {
        return 0;
    }
    1
}
