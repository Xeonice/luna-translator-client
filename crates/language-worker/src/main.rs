use app_core::contracts::{LanguageRequest, LanguageResponse};
use language_worker::{MAX_FRAME_BYTES, Mecab};
use std::{
    io::{self, BufRead, Read, Write},
    path::PathBuf,
};
fn main() {
    if let Err(error) = run() {
        eprintln!("language-worker: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--resources")) {
        return Err("usage: language-worker --resources <app resources directory>".into());
    }
    let root = PathBuf::from(args.next().ok_or("missing resources directory")?);
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    let mut engine = Mecab::open(&root)?;
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    loop {
        let mut frame = Vec::new();
        let count = (&mut input)
            .take((MAX_FRAME_BYTES + 1) as u64)
            .read_until(b'\n', &mut frame)
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Ok(());
        }
        if frame.len() > MAX_FRAME_BYTES || frame.last() != Some(&b'\n') {
            return Err("language frame exceeds limit or is truncated".into());
        }
        let request: LanguageRequest = serde_json::from_slice(&frame).map_err(|e| e.to_string())?;
        if request.v != 1 || request.request_id.is_empty() || request.request_id.len() > 128 {
            return Err("unsupported language protocol or request identity".into());
        }
        let result = engine.analyze(&request.text);
        let response = LanguageResponse {
            v: 1,
            request_id: request.request_id,
            resource_version: engine.resource_version.clone(),
            tokens: result.as_ref().cloned().unwrap_or_default(),
            error: result.err(),
        };
        let mut bytes = serde_json::to_vec(&response).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        if bytes.len() > MAX_FRAME_BYTES {
            return Err("language result exceeds bounded frame".into());
        }
        output
            .write_all(&bytes)
            .and_then(|_| output.flush())
            .map_err(|e| e.to_string())?;
    }
}
