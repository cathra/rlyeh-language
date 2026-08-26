// W4：Future 版 join_all（并发轮询多个 future 直至全部 Ready）
struct MyFut { n: i64 }
impl Future for MyFut {
    type Output = i64;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        self.n = self.n + 1;
        if self.n >= 3 {
            Poll::Ready(self.n)
        } else {
            Poll::Pending
        }
    }
}

fn main() {
    let fs: Vec<MyFut> = vec![MyFut { n: 0 }, MyFut { n: 0 }];
    let r = future::join_all(fs);
    for x in r {
        println(x);
    }
}
