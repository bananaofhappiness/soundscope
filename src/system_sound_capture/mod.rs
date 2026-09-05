mod macos;
pub const SYSTEM_TAP_DEVICE_NAME: &str = "Soundscope System Tap";

#[cfg(target_os = "macos")]
pub type SystemAudioDevice = macos::SystemAudioDevice;
