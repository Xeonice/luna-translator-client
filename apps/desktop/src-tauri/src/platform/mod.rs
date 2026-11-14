pub mod menus;
pub mod window_manager;
pub mod login_startup;

pub mod word_card;

#[cfg(target_os = "macos")]
pub mod macos;
pub mod shortcuts;
pub mod window_geometry;
pub mod window_preview;

#[cfg(target_os = "macos")]
pub(crate) mod window_collector;

pub(crate) mod runtime_observation;
pub(crate) mod session_cover_coordinator;
pub(crate) mod session_preview_coordinator;

pub mod system_browser;
