use crate::{abi::ThreadParam, queue::EventSender};
use hook_contracts::{DecimalU64, ErrorCode, Event};
use std::{
    ffi::c_char,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU8, AtomicU32, Ordering},
    },
};

static CALLBACK_EVENTS: OnceLock<EventSender<Event>> = OnceLock::new();
static PROCESS_STATES: OnceLock<Arc<ProcessStates>> = OnceLock::new();
const MAX_UTF16_UNITS: usize = 65_536;
const MAX_HOOK_NAME_BYTES: usize = 4_096;

pub const MAX_PROCESS_SLOTS: usize = 16;

struct ProcessSlot {
    pid: AtomicU32,
    state: AtomicU8,
}

/// Connection control does not depend on a possibly full text-event queue.
pub struct ProcessStates {
    slots: [ProcessSlot; MAX_PROCESS_SLOTS],
}

impl ProcessStates {
    pub fn new() -> Self {
        Self {
            slots: [const {
                ProcessSlot {
                    pid: AtomicU32::new(0),
                    state: AtomicU8::new(0),
                }
            }; MAX_PROCESS_SLOTS],
        }
    }

    pub fn register(&self, pid: u32) -> bool {
        if let Some(slot) = self
            .slots
            .iter()
            .find(|slot| slot.pid.load(Ordering::Acquire) == pid)
        {
            slot.state.store(0, Ordering::Release);
            return true;
        }
        for slot in &self.slots {
            if slot
                .pid
                .compare_exchange(0, pid, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return true;
            }
        }
        false
    }

    pub fn get(&self, pid: u32) -> u8 {
        self.slots
            .iter()
            .find(|slot| slot.pid.load(Ordering::Acquire) == pid)
            .map_or(0, |slot| slot.state.load(Ordering::Acquire))
    }

    pub fn update(&self, pid: u32, state: u8) {
        if let Some(slot) = self
            .slots
            .iter()
            .find(|slot| slot.pid.load(Ordering::Acquire) == pid)
        {
            slot.state.store(state, Ordering::Release);
        }
    }

    pub fn forget(&self, pid: u32) {
        if let Some(slot) = self
            .slots
            .iter()
            .find(|slot| slot.pid.load(Ordering::Acquire) == pid)
        {
            slot.state.store(0, Ordering::Release);
            slot.pid.store(0, Ordering::Release);
        }
    }
}

pub fn install(sender: EventSender<Event>, states: Arc<ProcessStates>) -> Result<(), String> {
    PROCESS_STATES
        .set(states)
        .map_err(|_| "process states were already installed")?;
    CALLBACK_EVENTS
        .set(sender)
        .map_err(|_| "single host only; in-process restart is unsupported".into())
}

fn dispatch(f: impl FnOnce() -> Result<Event, String>) {
    let (event, lost_payload) = match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(event)) => (event, false),
        Ok(Err(message)) => (
            Event::Error {
                id: None,
                code: ErrorCode::CallbackInvalid,
                message,
            },
            true,
        ),
        Err(_) => (
            Event::Error {
                id: None,
                code: ErrorCode::CallbackInvalid,
                message: "panic while copying native callback; payload was not delivered".into(),
            },
            true,
        ),
    };
    if let Some(sender) = CALLBACK_EVENTS.get() {
        if lost_payload {
            sender.record_loss();
        }
        sender.try_send(event);
    }
}

/// The pinned native host guarantees a readable, NUL-terminated UTF-16 pointer
/// for the duration of a callback. Never retain that pointer after returning.
unsafe fn wide(pointer: *const u16) -> Result<String, String> {
    if pointer.is_null() {
        return Err("native callback returned a null UTF-16 pointer".into());
    }
    for length in 0..MAX_UTF16_UNITS {
        // SAFETY: The callback ABI guarantees a readable string. The explicit
        // cap prevents unbounded scans of malformed, unterminated payloads.
        if unsafe { *pointer.add(length) } == 0 {
            // SAFETY: The preceding scan established these initialized units.
            return String::from_utf16(unsafe { std::slice::from_raw_parts(pointer, length) })
                .map_err(|error| format!("invalid native UTF-16: {error}"));
        }
    }
    Err("native UTF-16 payload exceeds callback copy limit; integrity lost".into())
}

unsafe fn name(pointer: *const c_char) -> Result<String, String> {
    if pointer.is_null() {
        return Err("native callback returned a null hook name".into());
    }
    for length in 0..MAX_HOOK_NAME_BYTES {
        // SAFETY: The ABI guarantees a readable C string during the callback.
        if unsafe { *pointer.add(length) } == 0 {
            // SAFETY: The scan established these initialized bytes.
            let bytes = unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), length) };
            return String::from_utf8(bytes.to_vec())
                .map_err(|error| format!("invalid native hook-name UTF-8: {error}"));
        }
    }
    Err("native hook name exceeds callback copy limit; integrity lost".into())
}

pub unsafe extern "C" fn connected(pid: u32) {
    if let Some(states) = PROCESS_STATES.get() {
        states.update(pid, 1);
    }
    dispatch(|| Ok(Event::ProcessConnected { pid }));
}

pub unsafe extern "C" fn disconnected(pid: u32) {
    if let Some(states) = PROCESS_STATES.get() {
        states.update(pid, 2);
    }
    dispatch(|| Ok(Event::ProcessDisconnected { pid }));
}

pub unsafe extern "C" fn created(code: *const u16, hook_name: *const c_char, tp: ThreadParam) {
    // SAFETY: All pointers are borrowed from the pinned host for this callback.
    dispatch(|| unsafe {
        Ok(Event::StreamCreated {
            stream: tp.into(),
            hook_code: wide(code)?,
            hook_name: name(hook_name)?,
        })
    });
}

pub unsafe extern "C" fn destroyed(code: *const u16, hook_name: *const c_char, tp: ThreadParam) {
    // SAFETY: Copy before the host returns to destroying the stream.
    dispatch(|| unsafe {
        Ok(Event::StreamDestroyed {
            stream: tp.into(),
            hook_code: wide(code)?,
            hook_name: name(hook_name)?,
        })
    });
}

pub unsafe extern "C" fn output(
    code: *const u16,
    hook_name: *const c_char,
    tp: ThreadParam,
    text: *const u16,
) {
    // SAFETY: Copy each callback-owned string while it is still valid.
    dispatch(|| unsafe {
        Ok(Event::Text {
            stream: tp.into(),
            hook_code: wide(code)?,
            hook_name: name(hook_name)?,
            text: wide(text)?,
        })
    });
}

pub unsafe extern "C" fn info(kind: i32, text: *const u16) {
    // SAFETY: This pointer is valid only for the duration of the callback.
    dispatch(|| unsafe {
        Ok(Event::Log {
            kind,
            message: wide(text)?,
        })
    });
}

pub unsafe extern "C" fn inserted(pid: u32, address: u64, code: *const u16) {
    // SAFETY: Copy the callback-owned hook code immediately.
    dispatch(|| unsafe {
        Ok(Event::HookInserted {
            pid,
            address: DecimalU64(address),
            hook_code: wide(code)?,
        })
    });
}
