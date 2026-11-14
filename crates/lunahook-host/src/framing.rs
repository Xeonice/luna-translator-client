//! Bounded JSONL input framing. An oversized line is drained without allocation.

use hook_contracts::MAX_REQUEST_BYTES;
use std::io::{self, BufRead};

pub enum Frame {
    Json(Vec<u8>),
    TooLong,
}

pub fn read_frame(reader: &mut impl BufRead) -> io::Result<Option<Frame>> {
    let mut bytes = Vec::new();
    let mut oversized = false;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if oversized {
                Ok(Some(Frame::TooLong))
            } else if bytes.is_empty() {
                Ok(None)
            } else {
                Ok(Some(Frame::Json(bytes)))
            };
        }
        let end = available.iter().position(|byte| *byte == b'\n');
        let count = end.unwrap_or(available.len());
        if !oversized {
            if bytes.len().saturating_add(count) > MAX_REQUEST_BYTES {
                bytes.clear();
                oversized = true;
            } else {
                bytes.extend_from_slice(&available[..count]);
            }
        }
        reader.consume(count + usize::from(end.is_some()));
        if end.is_some() {
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
            return Ok(Some(if oversized {
                Frame::TooLong
            } else {
                Frame::Json(bytes)
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    #[test]
    fn oversized_frame_does_not_consume_next_request() {
        let mut input = vec![b'x'; MAX_REQUEST_BYTES + 1024];
        input.extend_from_slice(b"\n{}\r\n");
        let mut reader = BufReader::with_capacity(17, Cursor::new(input));
        assert!(matches!(
            read_frame(&mut reader).expect("read"),
            Some(Frame::TooLong)
        ));
        let Some(Frame::Json(bytes)) = read_frame(&mut reader).expect("second frame") else {
            panic!("expected JSON frame")
        };
        assert_eq!(bytes, b"{}");
        assert!(read_frame(&mut reader).expect("eof").is_none());
    }
}
