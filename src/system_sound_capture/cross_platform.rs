// Linux and Windows systems audio capture
#![cfg(not(target_os = "macos"))]
use cpal::traits::{DeviceTrait, HostTrait};
use eyre::{Result, eyre};

pub fn ensure_screen_capture_permission() -> bool {
    true
}

pub struct SystemAudioDevice;

impl SystemAudioDevice {
    pub unsafe fn new() -> Self {
        Self
    }
}

pub fn system_capture_device() -> Result<cpal::Device> {
    #[cfg(target_os = "linux")]
    {
        // Prefer JACK if JACK server is active
        if let Ok(jack_host) = cpal::host_from_id(cpal::HostId::Jack) {
            if let Some(dev) = jack_host.default_input_device() {
                return Some(dev).ok_or_else(|| eyre!("Failed to open jack device"));
            }
        }

        let host = cpal::default_host();
        if let Ok(mut devices) = host.input_devices() {
            if let Some(monitor) = devices.find(|d| {
                d.name()
                    .map(|n| n.to_lowercase().contains("monitor"))
                    .unwrap_or(false)
            }) {
                return Some(monitor).ok_or_else(|| eyre!("Failed to open system capture device"));
            }
        }

        host.default_output_device()
            .ok_or_else(|| eyre!("No system audio capture or monitor device found"))
    }

    #[cfg(target_os = "windows")]
    {
        cpal::default_host()
            .default_output_device()
            .ok_or_else("No default Windows output device found for loopback capture")
    }
}
