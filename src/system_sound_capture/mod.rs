mod macos;
pub const SYSTEM_TAP_DEVICE_NAME: &str = "Soundscope System Tap";

#[cfg(target_os = "macos")]
pub use macos::ensure_screen_capture_permission;

#[cfg(target_os = "macos")]
pub type SystemAudioDevice = macos::SystemAudioDevice;
