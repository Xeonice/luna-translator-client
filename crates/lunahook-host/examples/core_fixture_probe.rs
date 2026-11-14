//! Controlled real GDI → pinned Core → Rust broker round trip. This is not a game test.
#[cfg(not(windows))]
fn main() {
    eprintln!("This controlled native probe must run inside Windows/CrossOver");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    if let Err(error) = probe::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
mod probe {
    use hook_contracts::{CORE_COMMIT, Envelope, Event, PROTOCOL_VERSION};
    use std::{
        io::{BufRead, BufReader, Write},
        os::windows::io::AsRawHandle,
        process::{Command, Stdio},
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{FILETIME, HANDLE},
        Graphics::Gdi::{CreateCompatibleDC, DeleteDC, TextOutW},
        System::Threading::GetProcessTimes,
    };

    pub fn run() -> Result<(), String> {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        if args.first().is_some_and(|arg| arg == "--target") {
            // A real Windows GDI call, repeatedly changing the source text.
            let dc = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
            if dc.is_null() {
                return Err("CreateCompatibleDC failed".into());
            }
            for index in 0..40 {
                thread::sleep(Duration::from_millis(250));
                let text: Vec<u16> =
                    format!("[rust-core-fixture:{}] 原文を受け取ります。", index % 3 + 1)
                        .encode_utf16()
                        .collect();
                if unsafe { TextOutW(dc, 0, 0, text.as_ptr(), text.len() as i32) } == 0 {
                    unsafe { DeleteDC(dc) };
                    return Err("TextOutW failed".into());
                }
            }
            unsafe { DeleteDC(dc) };
            return Ok(());
        }
        if args.len() != 2 {
            return Err(
                "usage: core_fixture_probe.exe <broker.exe> <Core runtime directory>".into(),
            );
        }
        let self_path = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut target = Command::new(&self_path)
            .arg("--target")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let handle = target.as_raw_handle() as HANDLE;
        let mut creation = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit = creation;
        let mut kernel = creation;
        let mut user = creation;
        if unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) } == 0
        {
            let _ = target.kill();
            let _ = target.wait();
            return Err("GetProcessTimes failed".into());
        }
        let filetime =
            (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
        let mut broker = Command::new(&args[0])
            .arg("--runtime-dir")
            .arg(&args[1])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut stdin = broker.stdin.take().ok_or("broker stdin missing")?;
        let stdout = broker.stdout.take().ok_or("broker stdout missing")?;
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let _ = sender.send(line);
            }
        });
        let started = Instant::now();
        let mut connected = false;
        let mut injected = false;
        let mut distinct = std::collections::BTreeSet::new();
        let mut failure = None;
        while started.elapsed() < Duration::from_secs(18) {
            let line = match receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(Ok(line)) => line,
                Ok(Err(error)) => {
                    failure = Some(error.to_string());
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => {
                    failure = Some("broker stdout ended".into());
                    break;
                }
            };
            println!("{line}");
            let envelope: Envelope = serde_json::from_str(&line).map_err(|e| e.to_string())?;
            if envelope.v != PROTOCOL_VERSION {
                failure = Some("wrong protocol version".into());
                break;
            }
            match envelope.payload {
                Event::Ready {
                    abi_version: 1,
                    upstream_commit,
                    ..
                } if upstream_commit == CORE_COMMIT => {
                    writeln!(stdin,"{}",serde_json::json!({"v":PROTOCOL_VERSION,"id":"fixture-attach","method":"attach","params":{
                        "pid":target.id(),"expected_image_path":self_path.to_string_lossy(),"expected_creation_time":filetime.to_string()}})).map_err(|e|e.to_string())?;
                    stdin.flush().map_err(|e| e.to_string())?;
                }
                Event::ProcessConnected { pid } if pid == target.id() => connected = true,
                Event::InjectionFinished {
                    pid,
                    exit_code: Some(0),
                    accepted_by_helper: true,
                    ..
                } if pid == target.id() => injected = true,
                Event::Text { stream, text, .. }
                    if stream.pid == target.id() && text.starts_with("[rust-core-fixture:") =>
                {
                    distinct.insert(text.trim().to_owned());
                }
                Event::Error { message, .. } => {
                    failure = Some(message);
                    break;
                }
                Event::Overflow { .. } => {
                    failure = Some("event integrity lost".into());
                    break;
                }
                _ => {}
            }
            if connected && injected && distinct.len() == 3 {
                break;
            }
        }
        // Terminate only this explicitly-owned fixture, never any user process.
        let _ = target.kill();
        let _ = target.wait();
        let _ = writeln!(
            stdin,
            "{}",
            serde_json::json!({"v":PROTOCOL_VERSION,"id":"fixture-shutdown","method":"shutdown","params":{}})
        );
        let _ = stdin.flush();
        drop(stdin);
        let stop = Instant::now();
        let broker_status = loop {
            match broker.try_wait().map_err(|e| e.to_string())? {
                Some(status) => break status,
                None if stop.elapsed() < Duration::from_secs(5) => {
                    thread::sleep(Duration::from_millis(20))
                }
                None => {
                    let _ = broker.kill();
                    break broker.wait().map_err(|e| e.to_string())?;
                }
            }
        };
        let passed = failure.is_none()
            && connected
            && injected
            && distinct.len() == 3
            && broker_status.success();
        println!(
            "{}",
            serde_json::json!({"probe":"rust_core_gdi","passed":passed,"target_pid":target.id(),"broker_exit":broker_status.code(),"core_commit":CORE_COMMIT,"abi":1,"protocol":PROTOCOL_VERSION,"connected":connected,"injected":injected,"texts":distinct,"error":failure,"scope":"controlled owned GDI fixture; not a game or GUI acceptance"})
        );
        if passed {
            Ok(())
        } else {
            Err("controlled Core round trip failed".into())
        }
    }
}
