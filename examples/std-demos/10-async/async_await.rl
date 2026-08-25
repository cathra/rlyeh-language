// async_await.rl：S1c 定位——手写 __Fut_f 与 desugar 生成完全同构
async fn g(x: i64) -> i64 {
    x + 1
}

struct __Fut_f {
    state: i64,
    __fut_0: __Fut_g,
    v: i64,
}

impl Future for __Fut_f {
    fn poll(&mut self) -> Poll<i64> {
        if self.state == 0 {
            self.__fut_0 = g(41);
            match self.__fut_0.poll() {
                Poll::Ready(__v) => {
                    self.v = __v;
                    self.state = 2;
                }
                Poll::Pending => {
                    self.state = 1;
                    return Poll::Pending;
                }
            }
        }
        if self.state == 1 {
            match self.__fut_0.poll() {
                Poll::Ready(__v) => {
                    self.v = __v;
                    self.state = 2;
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }
        if self.state == 2 {
            return Poll::Ready(self.v + 1);
        }
        Poll::Ready(0)
    }
}

fn f() -> __Fut_f {
    __Fut_f { state: 0, __fut_0: g(0), v: 0 }
}

fn main() {
    let mut fut = f();
    let v = block_on(&mut fut);
    println(v);
}
