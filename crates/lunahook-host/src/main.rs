use hook_contracts::{Envelope, ErrorCode, Event};
use std::{
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

fn main() -> ExitCode {
    let directory = match arguments() {
        Ok(Some(path)) => path,
        Ok(None) => return ExitCode::SUCCESS,
        Err(message) => {
            emit_error(ErrorCode::InvalidRequest, message);
            return ExitCode::from(2);
        }
    };
    #[cfg(all(windows, any(target_arch = "x86_64", target_arch = "x86")))]
    {
        ExitCode::from(lunahook_host::windows::run(&directory))
    }
    #[cfg(not(all(windows, any(target_arch = "x86_64", target_arch = "x86"))))]
    {
        let _ = directory;
        emit_error(
            ErrorCode::UnsupportedPlatform,
            "lunahook-host requires a Windows x86 or x64 process; run it inside the selected CrossOver bottle on macOS".into(),
        );
        ExitCode::from(2)
    }
}

fn arguments() -> Result<Option<PathBuf>, String> {
    let mut args = std::env::args_os().skip(1);
    let Some(first) = args.next() else {
        return Err("usage: lunahook-host --runtime-dir <absolute local runtime directory>".into());
    };
    if first == "--help" || first == "-h" {
        eprintln!(
            "lunahook-host --runtime-dir <runtime directory>\nReads versioned JSONL on stdin; writes only JSONL on stdout. No GUI or translation service."
        );
        return Ok(None);
    }
    if first != "--runtime-dir" {
        return Err("expected --runtime-dir".into());
    }
    let directory = args.next().ok_or("missing runtime directory")?;
    if args.next().is_some() {
        return Err("unexpected extra command-line argument".into());
    }
    Ok(Some(PathBuf::from(directory)))
}

fn emit_error(code: ErrorCode, message: String) {
    let event = Envelope::new(Event::Error {
        id: None,
        code,
        message,
    });
    let mut output = io::stdout().lock();
    if serde_json::to_writer(&mut output, &event).is_ok() {
        let _ = writeln!(output);
        let _ = output.flush();
    }
}
