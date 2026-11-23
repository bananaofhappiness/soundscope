mod cross_platform;
mod macos;
pub const SYSTEM_TAP_DEVICE_NAME: &str = "Soundscope System Tap";

#[cfg(target_os = "macos")]
pub use macos::ensure_screen_capture_permission;

#[cfg(not(target_os = "macos"))]
pub use cross_platform::{
    SystemAudioDevice, ensure_screen_capture_permission, system_capture_device,
};

#[cfg(target_os = "macos")]
pub type SystemAudioDevice = macos::SystemAudioDevice;
