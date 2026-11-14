//! Private length-delimited collector transport; no native handles or globals.
use serde::{Serialize, de::DeserializeOwned};
use std::io::{self, Read, Write};
pub(super) const VERSION: u16 = 4;
pub(super) const MAX_HEADER: usize = 65_536;
pub(super) const MAX_FRAME: usize = 524_288;
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid collector packet")
}
fn read_bytes(reader: &mut impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let mut length = [0u8; 4];
    reader.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > limit {
        return Err(invalid());
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}
fn write_bytes(writer: &mut impl Write, bytes: &[u8], limit: usize) -> io::Result<()> {
    if bytes.is_empty() || bytes.len() > limit {
        return Err(invalid());
    }
    writer.write_all(&(bytes.len() as u32).to_be_bytes())?;
    writer.write_all(bytes)
}
pub(super) fn read_json<T: DeserializeOwned>(reader: &mut impl Read) -> io::Result<T> {
    serde_json::from_slice(&read_bytes(reader, MAX_HEADER)?).map_err(|_| invalid())
}
pub(super) fn write_json<T: Serialize>(writer: &mut impl Write, value: &T) -> io::Result<()> {
    struct Bounded(Vec<u8>);
    impl Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.0.len().saturating_add(bytes.len()) > MAX_HEADER {
                return Err(invalid());
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut bytes = Bounded(Vec::new());
    serde_json::to_writer(&mut bytes, value).map_err(|_| invalid())?;
    write_bytes(writer, &bytes.0, MAX_HEADER)
}
pub(super) fn read_frame<T: DeserializeOwned>(reader: &mut impl Read) -> io::Result<(T, Vec<u8>)> {
    Ok((read_json(reader)?, read_bytes(reader, MAX_FRAME)?))
}
pub(super) fn write_frame<T: Serialize>(
    writer: &mut impl Write,
    header: &T,
    frame: &[u8],
) -> io::Result<()> {
    if frame.is_empty() || frame.len() > MAX_FRAME {
        return Err(invalid());
    }
    write_json(writer, header)?;
    write_bytes(writer, frame, MAX_FRAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Header {
        v: u16,
        sequence: u64,
    }
    struct Partial<T>(T);
    impl<T: Read> Read for Partial<T> {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            let n = bytes.len().min(3);
            self.0.read(&mut bytes[..n])
        }
    }
    impl<T: Write> Write for Partial<T> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.write(&bytes[..bytes.len().min(2)])
        }
        fn flush(&mut self) -> io::Result<()> {
            self.0.flush()
        }
    }
    #[test]
    fn framed_binary_round_trip_handles_actual_partial_reads_and_writes() {
        let header = Header {
            v: VERSION,
            sequence: 9,
        };
        let mut wire = Partial(Vec::new());
        write_frame(&mut wire, &header, &[1, 2, 3, 4, 5]).unwrap();
        let (received, bytes): (Header, _) = read_frame(&mut Partial(Cursor::new(wire.0))).unwrap();
        assert_eq!(received, header);
        assert_eq!(bytes, [1, 2, 3, 4, 5]);
    }
    #[test]
    fn untrusted_lengths_and_truncation_reject_before_payload_allocation() {
        for length in [0, (MAX_HEADER + 1) as u32, u32::MAX] {
            let result: io::Result<Header> = read_json(&mut Cursor::new(length.to_be_bytes()));
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
        }
        let mut wire = Vec::new();
        write_json(
            &mut wire,
            &Header {
                v: VERSION,
                sequence: 0,
            },
        )
        .unwrap();
        for length in [0, (MAX_FRAME + 1) as u32, u32::MAX] {
            let mut bytes = wire.clone();
            bytes.extend(length.to_be_bytes());
            assert_eq!(
                read_frame::<Header>(&mut Cursor::new(bytes))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }
        let mut bytes = wire;
        bytes.extend(8u32.to_be_bytes());
        bytes.extend([1, 2]);
        assert_eq!(
            read_frame::<Header>(&mut Cursor::new(bytes))
                .unwrap_err()
                .kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
    #[test]
    fn private_v4_ready_and_frame_validation_rejects_actual_v2_and_v3_packets() {
        use super::super::{FrameHeader, FrameStreamGuard, Ready, valid_ready};
        assert_eq!(VERSION, 4);
        let ready = super::super::tests::ready();
        let worker = ready.worker.clone();
        for version in [2, 3, VERSION] {
            let mut sent = ready.clone();
            sent.v = version;
            let mut bytes = Vec::new();
            write_json(&mut bytes, &sent).unwrap();
            let received: Ready = read_json(&mut Cursor::new(bytes)).unwrap();
            assert_eq!(received.v, version);
            assert_eq!(
                valid_ready(&received, &ready.nonce, &worker, &worker),
                version == VERSION
            );
            let mut frame = super::super::tests::frame(3, 10);
            frame.header.v = version;
            let mut bytes = Vec::new();
            write_frame(&mut bytes, &frame.header, &frame.bytes).unwrap();
            let (header, payload): (FrameHeader, _) = read_frame(&mut Cursor::new(bytes)).unwrap();
            let mut guard = FrameStreamGuard::default();
            assert_eq!(
                guard.accept(
                    "instance",
                    &super::super::tests::source(),
                    &header,
                    &payload
                ),
                version == VERSION
            );
            if version != VERSION {
                assert_eq!(guard.sequence, 0);
                assert!(guard.native.is_none());
                assert!(guard.capture.is_none());
            }
        }
    }
    #[test]
    fn private_v4_worker_rejects_actual_v2_and_v3_hello_before_native_dispatch() {
        use super::super::{Hello, worker};
        use std::os::unix::net::UnixStream;
        assert_eq!(VERSION, 4);
        for version in [2, 3] {
            let (mut parent, control) = UnixStream::pair().unwrap();
            let (_parent_frames, frames) = UnixStream::pair().unwrap();
            let hello = Hello {
                v: version,
                nonce: "valid-nonce-for-owned-test-session-0000".into(),
                parent: super::super::tests::ready().worker,
            };
            write_json(&mut parent, &hello).unwrap();
            // The production handshake rejects old versions before any Native
            // parent inspection or RunLoop dispatch. Core/JSONL are unrelated.
            let rejected = worker::run(control, frames).unwrap_err();
            assert_eq!(rejected.code, "collector_handshake");
            let mut byte = [0];
            assert_eq!(parent.read(&mut byte).unwrap(), 0);
        }
    }
    #[test]
    fn private_v4_prepare_wire_requires_frozen_image_path_and_strict_schema() {
        use super::super::{Control, ControlOp};
        let control = Control {
            v: VERSION,
            instance: "instance".into(),
            request: 1,
            op: ControlOp::Prepare {
                source: Box::new(super::super::tests::source()),
                select: false,
            },
        };
        let expected = serde_json::to_value(&control).unwrap();
        let mut packet = Vec::new();
        write_json(&mut packet, &expected).unwrap();
        let received: Control = read_json(&mut Cursor::new(packet)).unwrap();
        assert_eq!(received.v, 4);
        assert_eq!(serde_json::to_value(&received).unwrap(), expected);
        assert_eq!(
            expected["op"]["source"]["target"]["image_path"],
            "/fixture/drive_c/Fixture.exe"
        );
        for version in [2, 3, VERSION] {
            let mut missing_path = expected.clone();
            missing_path["v"] = version.into();
            missing_path["op"]["source"]["target"]
                .as_object_mut()
                .unwrap()
                .remove("image_path");
            let mut packet = Vec::new();
            write_json(&mut packet, &missing_path).unwrap();
            assert!(read_json::<Control>(&mut Cursor::new(packet)).is_err());
        }
        for path in [
            "/unrecognized",
            "/op/unrecognized",
            "/op/source/unrecognized",
            "/op/source/target/unrecognized",
            "/op/source/target/candidate/unrecognized",
        ] {
            let mut unknown = expected.clone();
            let (parent, field) = path.rsplit_once('/').unwrap();
            unknown
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(field.into(), true.into());
            let mut packet = Vec::new();
            write_json(&mut packet, &unknown).unwrap();
            assert!(
                read_json::<Control>(&mut Cursor::new(packet)).is_err(),
                "{path}"
            );
        }
    }
    #[test]
    fn unknown_json_fields_and_oversized_output_are_not_forwarded() {
        let mut wire = Vec::new();
        write_json(
            &mut wire,
            &serde_json::json!({"v":1,"sequence":0,"unexpected":true}),
        )
        .unwrap();
        assert!(read_json::<Header>(&mut Cursor::new(wire)).is_err());
        let mut rejected = Vec::new();
        assert!(write_json(&mut rejected, &"x".repeat(MAX_HEADER + 1)).is_err());
        assert!(rejected.is_empty());
        let mut wire = Vec::new();
        assert!(
            write_frame(
                &mut wire,
                &Header { v: 1, sequence: 0 },
                &vec![0; MAX_FRAME + 1]
            )
            .is_err()
        );
        assert!(wire.is_empty());
    }
    #[test]
    fn private_v4_actual_status_wire_requires_checked_typed_geometry_fields() {
        use super::super::{Status, valid_status_metadata};
        use std::os::unix::net::UnixStream;
        let mut status = Status::new(
            "instance",
            &super::super::tests::source(),
            "capture",
            "Unavailable",
            Some("preview_budget_exceeded".into()),
        );
        status.diagnostic = Some(super::super::tests::recovery_diagnostic_fixture());
        let expected = serde_json::to_value(&status).unwrap();
        let (mut sender, mut receiver) = UnixStream::pair().unwrap();
        write_json(&mut sender, &expected).unwrap();
        let received: Status = read_json(&mut receiver).unwrap();
        assert_eq!(received.v, 4);
        assert!(valid_status_metadata(&received));
        for field in ["geometry", "failure_kind"] {
            let mut missing = expected.clone();
            missing["diagnostic"].as_object_mut().unwrap().remove(field);
            write_json(&mut sender, &missing).unwrap();
            let received: Status = read_json(&mut receiver).unwrap();
            assert!(!valid_status_metadata(&received), "missing {field}");
        }
        let mut unknown = expected.clone();
        unknown["diagnostic"]["geometry"]["unrecognized"] = true.into();
        write_json(&mut sender, &unknown).unwrap();
        assert!(read_json::<Status>(&mut receiver).is_err());
        let mut missing = expected.clone();
        missing["diagnostic"]["geometry"]
            .as_object_mut()
            .unwrap()
            .remove("sc_window_frame_query");
        write_json(&mut sender, &missing).unwrap();
        assert!(read_json::<Status>(&mut receiver).is_err());
    }
}
