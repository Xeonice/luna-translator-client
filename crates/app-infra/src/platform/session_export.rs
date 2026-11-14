//! Bounded, cancellable JSONL delivery of an already pinned application snapshot.
use app_core::{application::runtime::ApplicationRuntime, contracts::*};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};
/// Cancellation wins until the atomic delivery claim. Once rename is claimed,
/// callers must wait for its receipt rather than promise a cancelled delivery.
#[derive(Default)]
pub struct ExportControl(AtomicU8);
impl ExportControl {
    pub fn request_cancel(&self) -> bool {
        self.0
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
            || self.0.load(Ordering::Acquire) == 1
    }
    fn cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire) == 1
    }
    fn claim_delivery(&self) -> bool {
        self.0
            .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}

fn error(code: &str, text: &str) -> AppError {
    AppError::new(code, "session_export", text)
}
pub fn deliver(
    runtime: &Arc<ApplicationRuntime>,
    plan: &SessionExportPlan,
    destination: &Path,
    cancelled: &ExportControl,
) -> AppResult<SessionExportReceipt> {
    let parent = destination
        .parent()
        .filter(|p| p.is_dir())
        .ok_or_else(|| error("export_destination", "请选择有效的保存目录"))?;
    if !destination.is_absolute() || destination.is_dir() || destination.is_symlink() {
        return Err(error("export_destination", "保存目标无效，请重新选择"));
    }
    let initial = std::fs::metadata(destination)
        .ok()
        .map(|m| (m.len(), m.modified().ok()));
    let pending = parent.join(format!(".luna-export-{}.pending", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&pending)
            .map_err(|_| error("export_create", "无法创建导出临时文件"))?;
        let mut hash = Sha256::new();
        let count = {
            let mut write = |value: &serde_json::Value| -> AppResult<()> {
                let mut line = serde_json::to_vec(value)
                    .map_err(|_| error("export_encoding", "导出记录无法编码"))?;
                line.push(b'\n');
                file.write_all(&line)
                    .map_err(|_| error("export_write", "导出写入失败，未交付不完整文件"))?;
                hash.update(&line);
                Ok(())
            };
            write(&serde_json::json!({"format":"luna-session-jsonl","version":1,"plan":plan}))?;
            let mut cursor = None;
            let mut count = 0_u64;
            loop {
                if cancelled.cancelled() {
                    return Err(error("export_cancelled", "导出已取消"));
                }
                let page =
                    runtime.session_export_page(plan.export_id.clone(), cursor.clone(), 100)?;
                if page.snapshot.high_watermark != plan.snapshot.high_watermark {
                    return Err(error(
                        "export_range_changed",
                        "导出冻结范围发生变化，未交付文件",
                    ));
                }
                for record in &page.records {
                    write(
                        &serde_json::to_value(record)
                            .map_err(|_| error("export_encoding", "导出记录无法编码"))?,
                    )?;
                    count = count
                        .checked_add(1)
                        .ok_or_else(|| error("export_count", "导出记录数量超出支持范围"))?;
                }
                match page.next_cursor {
                    Some(next) if Some(&next) != cursor.as_ref() => cursor = Some(next),
                    Some(_) => return Err(error("export_cursor", "导出分页未向前推进")),
                    None => break,
                }
            }
            count
        };
        if cancelled.cancelled() {
            return Err(error("export_cancelled", "导出已取消"));
        }
        file.sync_all()
            .map_err(|_| error("export_flush", "导出文件尚未安全写入"))?;
        drop(file);
        let current = std::fs::metadata(destination)
            .ok()
            .map(|m| (m.len(), m.modified().ok()));
        if current != initial || destination.is_symlink() {
            return Err(error(
                "export_destination_changed",
                "保存目标已变化，请重新确认覆盖",
            ));
        }
        if !cancelled.claim_delivery() {
            return Err(error("export_cancelled", "导出已取消"));
        }
        std::fs::rename(&pending, destination)
            .map_err(|_| error("export_delivery", "导出未完成交付，请重新选择保存位置"))?;
        File::open(parent).and_then(|f| f.sync_all()).map_err(|_| {
            error(
                "export_directory_flush",
                "文件已交付，但保存目录尚未确认安全写入",
            )
        })?;
        Ok(SessionExportReceipt {
            export_id: plan.export_id.clone(),
            status: SessionExportStatus::Completed,
            record_count: count.to_string(),
            destination_label: destination
                .file_name()
                .map(|n| n.to_string_lossy().into_owned()),
            content_sha256: Some(format!("{:x}", hash.finalize())),
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&pending);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancel_and_delivery_have_one_authoritative_winner() {
        let cancelled = ExportControl::default();
        assert!(cancelled.request_cancel());
        assert!(cancelled.request_cancel());
        assert!(!cancelled.claim_delivery());
        let delivered = ExportControl::default();
        assert!(delivered.claim_delivery());
        assert!(!delivered.request_cancel());
        assert!(!delivered.claim_delivery());
        for _ in 0..64 {
            let control = Arc::new(ExportControl::default());
            let other = control.clone();
            let worker = std::thread::spawn(move || other.claim_delivery());
            let cancellation_won = control.request_cancel();
            let delivery_won = worker.join().unwrap();
            assert_ne!(cancellation_won, delivery_won);
        }
    }
}
