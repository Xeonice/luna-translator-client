//! Bounded observation wire; intentionally no generic parent writer/Execute sender.
use super::{OfflineErrorCode, OfflineFailure, OfflineResult};
use app_core::domain::offline_runtime::{
    ChildFrameV2, MAX_V2_FRAME_BYTES, OfflineWorkerHelloV2, OfflineWorkerReferenceV2,
    ParentFrameV2, decode_child_v2, encode_parent_v2,
};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};
fn bad(stage: &str) -> OfflineFailure {
    OfflineFailure::new(OfflineErrorCode::ProtocolViolation, stage)
}
pub async fn read_child_v2<R: AsyncBufRead + Unpin>(
    reader: &mut R,
) -> OfflineResult<Option<ChildFrameV2>> {
    let mut bytes = Vec::new();
    loop {
        let chunk = reader.fill_buf().await.map_err(|_| bad("v2_stdout"))?;
        if chunk.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err(bad("v2_incomplete_frame"))
            };
        }
        let end = chunk.iter().position(|b| *b == b'\n').map(|n| n + 1);
        let take = end.unwrap_or(chunk.len());
        if bytes.len().saturating_add(take) > MAX_V2_FRAME_BYTES {
            return Err(bad("v2_frame_limit"));
        }
        bytes.extend_from_slice(&chunk[..take]);
        reader.consume(take);
        if end.is_some() {
            return decode_child_v2(&bytes)
                .map(Some)
                .map_err(|_| bad("v2_child_frame"));
        }
    }
}
pub async fn write_hello_v2<W: AsyncWrite + Unpin>(
    writer: &mut W,
    hello: &OfflineWorkerHelloV2,
) -> OfflineResult<()> {
    let bytes = encode_parent_v2(&ParentFrameV2::Hello {
        hello: Box::new(hello.clone()),
    })
    .map_err(|_| bad("v2_hello_shape"))?;
    writer
        .write_all(&bytes)
        .await
        .map_err(|_| bad("v2_stdin"))?;
    writer.flush().await.map_err(|_| bad("v2_stdin"))
}
pub async fn write_cancel_v2<W: AsyncWrite + Unpin>(
    writer: &mut W,
    reference: &OfflineWorkerReferenceV2,
) -> OfflineResult<()> {
    let bytes = encode_parent_v2(&ParentFrameV2::Cancel {
        reference: Box::new(reference.clone()),
    })
    .map_err(|_| bad("v2_cancel_shape"))?;
    writer
        .write_all(&bytes)
        .await
        .map_err(|_| bad("v2_stdin"))?;
    writer.flush().await.map_err(|_| bad("v2_stdin"))
}
