extern fn fread(buf: String, size: i64, nmemb: i64, f: i64) -> i64;

fn try_ptr(xs: &[u8]) -> i64 {
  fread(xs.as_ptr(), 1, xs.len(), 0)
}

fn main() {
  let mut v: Vec<u8> = Vec::new();
  v.push(65);
  println(try_ptr(v.as_slice()));
}
