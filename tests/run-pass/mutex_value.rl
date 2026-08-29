// Y4b-2 final check: std Mutex<T> carries a value and exposes it via get()/get_mut().
// g.get() is &self (cross-module OK); g.get_mut() is &mut self (explicit (&mut g) borrow
// due to lang-defects #10). Bare blocks need a trailing semicolon.
fn main() {
    let mut m = Mutex::new(42);
    {
        let g = m.lock_guard();
        println(g.get());
    };
    {
        let mut g = m.lock_guard();
        let p = (&mut g).get_mut();
        *p = 100;
    };
    {
        let g = m.lock_guard();
        println(g.get());
    };
    println(0);
}
