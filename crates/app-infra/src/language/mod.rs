//! A bounded process boundary: an unresponsive native parser is killable.
use app_core::contracts::{LanguageRequest, LanguageResponse, LexicalToken, ResourceInfo};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    process::Stdio,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex,
};
const MAX_TEXT_BYTES: usize = 65_536;
const MAX_FRAME_BYTES: usize = 1_048_576;
const DEADLINE: Duration = Duration::from_secs(15);
const STARTUP_DEADLINE: Duration = Duration::from_secs(60);
struct Worker {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    primed: bool,
}
pub struct LanguageClient {
    resources: PathBuf,
    worker: Mutex<Option<Worker>>,
    counter: AtomicU64,
    queue: tokio::sync::Semaphore,
}
impl LanguageClient {
    pub fn new(resource_dir: PathBuf) -> Self {
        Self {
            resources: resource_dir,
            worker: Mutex::new(None),
            counter: AtomicU64::new(1),
            queue: tokio::sync::Semaphore::new(8),
        }
    }
    pub fn resource_info(&self) -> ResourceInfo {
        let bytes = std::fs::read(self.resources.join("language/manifest.json"));
        match bytes.and_then(|bytes| {
            serde_json::from_slice::<serde_json::Value>(&bytes)
                .map(|value| (bytes, value))
                .map_err(std::io::Error::other)
        }) {
            Ok((bytes, value)) => ResourceInfo {
                kind: "language".into(),
                version: value["resource_version"]
                    .as_str()
                    .unwrap_or("invalid")
                    .into(),
                manifest_sha256: format!("{:x}", Sha256::digest(&bytes)),
                available: self.resources.join("language-worker").is_file()
                    && self.resources.join("language/unidic/sys.dic").is_file(),
                detail: "离线 MeCab 0.996 / UniDic 2025.12；工作进程启动时逐文件核验".into(),
            },
            Err(error) => ResourceInfo {
                kind: "language".into(),
                version: "unavailable".into(),
                manifest_sha256: String::new(),
                available: false,
                detail: error.to_string(),
            },
        }
    }
    pub async fn analyze(&self, text: &str) -> Result<Vec<LexicalToken>, String> {
        if text.len() > MAX_TEXT_BYTES || text.contains('\0') {
            return Err("文本超过65536 UTF-8字节或包含NUL".into());
        }
        if text.is_empty() {
            return Ok(Vec::new());
        }
        let _queue_slot = self
            .queue
            .try_acquire()
            .map_err(|_| "语言分析队列已满，请稍后重试")?;
        let request = LanguageRequest {
            v: 1,
            request_id: format!("language-{}", self.counter.fetch_add(1, Ordering::Relaxed)),
            text: text.into(),
        };
        let mut worker = tokio::time::timeout(DEADLINE, self.worker.lock())
            .await
            .map_err(|_| "等待语言工作进程超过15秒")?;
        if worker.is_none() {
            *worker = Some(self.spawn()?);
        }
        let budget = if worker.as_ref().is_some_and(|worker| worker.primed) {
            DEADLINE
        } else {
            STARTUP_DEADLINE
        };
        let operation = async {
            let active = worker.as_mut().ok_or("language worker missing")?;
            let mut bytes = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
            bytes.push(b'\n');
            if bytes.len() > MAX_FRAME_BYTES {
                return Err("language request frame too large".into());
            }
            active
                .input
                .write_all(&bytes)
                .await
                .map_err(|e| e.to_string())?;
            active.input.flush().await.map_err(|e| e.to_string())?;
            let mut frame = Vec::new();
            (&mut active.output)
                .take((MAX_FRAME_BYTES + 1) as u64)
                .read_until(b'\n', &mut frame)
                .await
                .map_err(|e| e.to_string())?;
            if frame.len() > MAX_FRAME_BYTES || frame.last() != Some(&b'\n') {
                return Err("language result truncated, oversized or worker exited".into());
            }
            let response: LanguageResponse =
                serde_json::from_slice(&frame).map_err(|e| e.to_string())?;
            if response.v != 1
                || response.request_id != request.request_id
                || response.resource_version != self.resource_info().version
            {
                return Err("language response identity/version mismatch".into());
            }
            if let Some(error) = response.error {
                return Err(error);
            }
            validate_spans(text, &response.tokens)?;
            active.primed = true;
            Ok(response.tokens)
        };
        match tokio::time::timeout(budget, operation).await {
            Ok(Ok(tokens)) => Ok(tokens),
            result => {
                if let Some(mut owned) = worker.take() {
                    let _ = owned.child.kill().await;
                    let _ = owned.child.wait().await;
                }
                Err(match result {
                    Ok(Err(error)) => format!("语言工作进程已停止，下次请求将重新启动：{error}"),
                    _ => format!(
                        "语言工作进程超过{}秒，已终止本客户端持有的进程；可重试",
                        budget.as_secs()
                    ),
                })
            }
        }
    }
    pub async fn stop(&self) {
        if let Some(mut owned) = self.worker.lock().await.take() {
            let _ = owned.child.kill().await;
            let _ = owned.child.wait().await;
        }
    }
    fn spawn(&self) -> Result<Worker, String> {
        let executable = self.resources.join("language-worker");
        if !executable.is_file() {
            return Err("随包语言工作进程缺失".into());
        }
        let mut child = Command::new(executable)
            .arg("--resources")
            .arg(&self.resources)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("启动语言工作进程失败：{e}"))?;
        Ok(Worker {
            input: child.stdin.take().ok_or("worker stdin missing")?,
            output: BufReader::new(child.stdout.take().ok_or("worker stdout missing")?),
            child,
            primed: false,
        })
    }
}
fn validate_spans(text: &str, tokens: &[LexicalToken]) -> Result<(), String> {
    let mut end = 0;
    let mut utf16_end = 0;
    for token in tokens {
        let start = token.byte_start as usize;
        let finish = token.byte_end as usize;
        if start < end || start >= finish || text.get(start..finish) != Some(token.surface.as_str())
        {
            return Err("language worker returned invalid UTF-8 source span".into());
        }
        let expected_start = utf16_end + text[end..start].encode_utf16().count() as u32;
        let expected_end = expected_start + token.surface.encode_utf16().count() as u32;
        if expected_start != token.utf16_start || expected_end != token.utf16_end {
            return Err("language worker returned invalid UTF-16 span".into());
        }
        end = finish;
        utf16_end = expected_end;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
