//! JSON-RPC 2.0 over stdio 帧编解码。
//!
//! LSP 在 stdin/stdout 上使用 HTTP 风格的头帧：
//!
//! ```text
//! Content-Length: <N>\r\n\r\n<JSON body 恰好 N 字节>
//! ```
//!
//! 每次读写一条消息体（`serde_json::Value`）。

use std::io::{self, Read, Write};

/// 从 `reader` 读取一帧，返回消息体字节。
///
/// EOF（流关闭且无未完成帧）返回 `Ok(None)`。
pub fn read_frame(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut headers = Vec::new();
    let mut buf = [0u8; 1];
    // 读取头（到空行 `\r\n\r\n`）。
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            // 流关闭：若还没有任何头数据则视为正常结束
            if headers.is_empty() {
                return Ok(None);
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "stream closed inside frame headers",
            ));
        }
        headers.push(buf[0]);
        let len = headers.len();
        if len >= 4 && &headers[len - 4..] == b"\r\n\r\n" {
            break;
        }
        if len > 16 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "frame header too large",
            ));
        }
    }

    let head = String::from_utf8_lossy(&headers);
    let content_length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.trim().eq_ignore_ascii_case("Content-Length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length"))?;

    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

/// 向 `writer` 写出一帧。
pub fn write_frame(writer: &mut impl Write, body: &[u8]) -> io::Result<()> {
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrip() {
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let mut buf = Vec::new();
        write_frame(&mut buf, body).unwrap();
        let expect = format!("Content-Length: {}\r\n\r\n", body.len());
        assert!(buf.starts_with(expect.as_bytes()));
        assert!(buf.ends_with(body));

        let mut cursor = &buf[..];
        let out = read_frame(&mut cursor).unwrap().unwrap();
        assert_eq!(out, body);
    }

    #[test]
    fn frame_multiple_in_stream() {
        let mut buf = Vec::new();
        write_frame(&mut buf, br#"{"a":1}"#).unwrap();
        write_frame(&mut buf, br#"{"b":2}"#).unwrap();

        let mut cursor = &buf[..];
        assert_eq!(read_frame(&mut cursor).unwrap().unwrap(), br#"{"a":1}"#);
        assert_eq!(read_frame(&mut cursor).unwrap().unwrap(), br#"{"b":2}"#);
        assert!(read_frame(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn eof_mid_header_is_error() {
        // "Content-Length: 2\r\n\r\n" 后只有 1 字节 body
        let mut buf = br#"Content-Length: 2"#.to_vec();
        buf.extend_from_slice(b"\r\n\r\n");
        buf.push(b'{');
        let mut cursor = &buf[..];
        assert!(read_frame(&mut cursor).unwrap_err().kind() == io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn empty_stream_is_none() {
        let empty: &[u8] = b"";
        assert!(read_frame(&mut &empty[..]).unwrap().is_none());
    }
}
