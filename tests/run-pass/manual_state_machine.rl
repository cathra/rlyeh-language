// manual_state_machine.rl：手写状态机对照（S1c 排查用）
// 与 desugar 输出完全同构：&mut Self 签名 + 函数构造器 + 字段赋值。
struct G {
    state: i64,
    x: i64,
}

fn mk_g(x: i64) -> G {
    G { state: 0, x: x }
}

impl G: Future {
    type Output = i64;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        if self.state == 0 {
            return Poll::Ready(self.x + 1);
        }
        Poll::Ready(0)
    }
}

struct F {
    state: i64,
    __fut_0: G,
    v: i64,
}

fn mk_f() -> F {
    F { state: 0, __fut_0: mk_g(0), v: 0 }
}

impl F: Future {
    type Output = i64;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        if self.state == 0 {
            self.__fut_0 = mk_g(41);
            match self.__fut_0.poll(&mut *cx) {
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
            match self.__fut_0.poll(&mut *cx) {
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

fn main() {
    let mut fut = mk_f();
    let v = block_on(&mut fut);
    println(v);
}
