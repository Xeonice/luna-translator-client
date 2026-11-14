//! Narrow host utilities. This crate never starts a game or fabricates native events.

pub mod abi;
pub mod framing;
pub mod queue;
pub mod runtime;

#[cfg(all(windows, any(target_arch = "x86_64", target_arch = "x86")))]
pub mod windows;
