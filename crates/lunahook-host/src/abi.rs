//! Audited value-only C ABI. No C++ object layout crosses this boundary.

use hook_contracts::{DecimalU64, StreamId};
use std::ffi::c_char;

#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct ThreadParam {
    pub process_id: u32,
    pub addr: u64,
    pub ctx: u64,
    pub ctx2: u64,
}

impl From<ThreadParam> for StreamId {
    fn from(value: ThreadParam) -> Self {
        Self {
            pid: value.process_id,
            address: DecimalU64(value.addr),
            context: DecimalU64(value.ctx),
            context2: DecimalU64(value.ctx2),
        }
    }
}

pub type ProcessCallback = unsafe extern "C" fn(u32);
pub type CreateCallback = unsafe extern "C" fn(*const u16, *const c_char, ThreadParam);
pub type DestroyCallback = unsafe extern "C" fn(*const u16, *const c_char, ThreadParam);
pub type OutputCallback = unsafe extern "C" fn(*const u16, *const c_char, ThreadParam, *const u16);
pub type HostInfoCallback = unsafe extern "C" fn(i32, *const u16);
pub type HookInsertCallback = unsafe extern "C" fn(u32, u64, *const u16);
pub type AbiVersionFn = unsafe extern "C" fn() -> u32;
pub type StartFn = unsafe extern "C" fn(
    ProcessCallback,
    ProcessCallback,
    CreateCallback,
    DestroyCallback,
    OutputCallback,
    HostInfoCallback,
    HookInsertCallback,
);
pub type SettingsFn = unsafe extern "C" fn(i32, i32, i32, i32, bool);
pub type ProcessFn = unsafe extern "C" fn(u32);
pub type CheckInjectFn = unsafe extern "C" fn(u32) -> bool;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn upstream_thread_param_layout_is_explicit() {
        assert_eq!(size_of::<ThreadParam>(), 32);
        assert_eq!(align_of::<ThreadParam>(), 8);
        assert_eq!(offset_of!(ThreadParam, process_id), 0);
        assert_eq!(offset_of!(ThreadParam, addr), 8);
        assert_eq!(offset_of!(ThreadParam, ctx), 16);
        assert_eq!(offset_of!(ThreadParam, ctx2), 24);
    }
}
