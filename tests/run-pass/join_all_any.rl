// W4 补全：join_all 返回 Vec<F::Output>，F::Output 投影支持非 i64 类型
struct StrFut { s: String, n: i64 }
impl Future for StrFut {
    type Output = String;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        self.n = self.n + 1;
        if self.n >= 2 {
            Poll::Ready(self.s)
        } else {
            Poll::Pending
        }
    }
}

fn main() {
    // join_all 收集 Vec<String>
    let fs: Vec<StrFut> = vec![
        StrFut { s: String::from("hi"), n: 0 },
        StrFut { s: String::from("yo"), n: 0 },
    ];
    let r = future::join_all(fs);
    for x in r {
        println(x);
    }
}
