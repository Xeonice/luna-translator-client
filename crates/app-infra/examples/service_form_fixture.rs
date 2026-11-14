//! Test-only JSONL transport for the real immutable catalogue and pure Rust form projection.
//! It never accesses a profile, credentials, a provider endpoint, native APIs or resources.
use app_core::contracts::ServiceFormRequest;
use app_infra::providers::builtin_registry;
use serde::Deserialize;
use serde_json::json;
use std::io::{self, BufRead, Read, Write};

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Catalog {},
    Form { request: ServiceFormRequest },
}
fn main() -> io::Result<()> {
    let registry = builtin_registry().expect("reviewed builtin registry");
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = io::stdout().lock();
    loop {
        let mut line = Vec::new();
        let read = input.by_ref().take(65_537).read_until(b'\n', &mut line)?;
        if read == 0 { break; }
        if read > 65_536 { writeln!(output, "{{\"ok\":false,\"code\":\"fixture_request_too_large\"}}")?; output.flush()?; break; }
        let response = match serde_json::from_slice::<Request>(&line) {
            Ok(Request::Catalog {}) => json!({"ok":true,"value":registry.catalog()}),
            Ok(Request::Form { request }) => match registry.project_form(&request) {
                Ok(value) => json!({"ok":true,"value":value}),
                Err(error) => json!({"ok":false,"code":error.code}),
            },
            Err(_) => json!({"ok":false,"code":"invalid_fixture_request"}),
        };
        serde_json::to_writer(&mut output, &response)?;
        writeln!(output)?;
        output.flush()?;
    }
    Ok(())
}
