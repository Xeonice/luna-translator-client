//! Actual native client → Wine broker → pinned Core → owned Windows GDI fixture.
//! Successful output is not a real-game or graphical-product acceptance result.
use hook_client::{Client, LaunchSettings, NativeEvent, SubscriptionError};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 5 {
        return Err(
            "usage: core_client_probe <resources> <wine> <bottle> <fixture.exe> <logs>".into(),
        );
    }
    let resources = PathBuf::from(&args[0]);
    let logs = PathBuf::from(&args[4]);
    std::fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
    let settings = LaunchSettings {
        wine_path: args[1].to_str().ok_or("invalid wine path")?.into(),
        bottle: args[2].to_str().ok_or("invalid bottle")?.into(),
    };
    let mut fixture = Command::new(&args[1])
        .args(["--bottle", &settings.bottle, "--no-update", "--no-gui"])
        .arg(&args[3])
        .arg("--target")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let client = Client::new(resources, logs.join("sessions"));
    let subscription = client.subscribe(512)?;
    let started = Instant::now();
    let target = loop {
        let matches = client
            .discover_targets(&settings)?
            .into_iter()
            .filter(|target| target.exe.eq_ignore_ascii_case("core_fixture_probe.exe"))
            .collect::<Vec<_>>();
        if matches.len() == 1 {
            break matches.into_iter().next().unwrap();
        }
        if matches.len() > 1 {
            return Err("multiple fixture identities; refusing to guess a target".into());
        }
        if started.elapsed() > Duration::from_secs(5) {
            return Err("owned fixture did not become discoverable".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    client.connect(settings, target)?;
    let mut distinct = BTreeSet::new();
    let mut received = Vec::new();
    let mut failure = None;
    while started.elapsed() < Duration::from_secs(15) {
        match subscription.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => {
                if let Some(NativeEvent::Text { text, .. }) = &event.payload
                    && text.starts_with("[rust-core-fixture:")
                {
                    distinct.insert(text.trim().to_owned());
                }
                if event.integrity_lost {
                    failure = Some("client integrity lost".to_string());
                    break;
                }
                received.push(event);
                if distinct.len() == 3 {
                    break;
                }
            }
            Err(SubscriptionError::Timeout) => {}
            Err(error) => {
                failure = Some(format!("subscription: {error:?}"));
                break;
            }
        }
    }
    let captured = client.snapshot();
    client.disconnect()?;
    let stop = Instant::now();
    while client.has_active_session() && stop.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let fixture_status = fixture.wait().map_err(|e| e.to_string())?;
    let final_state = client.snapshot();
    let passed = failure.is_none()
        && distinct.len() == 3
        && captured.status == "connected"
        && !captured.integrity_lost
        && !client.has_active_session()
        && final_state.errors.is_empty()
        && fixture_status.success();
    let receipt = serde_json::json!({"scope":"native macOS Rust ClientSubscription and pinned Core; controlled owned GDI fixture, not game/UI acceptance","passed":passed,"texts":distinct,"captured":captured,"final":final_state,"failure":failure,"events":received});
    std::fs::write(
        logs.join("result.json"),
        serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!(
        "{}",
        serde_json::json!({"passed":passed,"distinct_texts":distinct.len(),"receipt":logs.join("result.json")})
    );
    if passed {
        Ok(())
    } else {
        Err("native client Core round trip failed".into())
    }
}
