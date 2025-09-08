//! This module is responsible for capturing audio from the PC and microphone.
use crate::tui::RBuffer;
use color_eyre::{Result, eyre::Ok};
use cpal::{
    Device, Stream, StreamConfig, default_host,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use ringbuffer::AllocRingBuffer;
use std::sync::{Arc, Mutex};

pub struct AudioDevice {
    device: Device,
    config: StreamConfig,
}

impl AudioDevice {
    pub fn new(preferred_dev: Option<cpal::Device>) -> Self {
        let host = default_host();
        let device = preferred_dev.unwrap_or(host.default_input_device().unwrap());
        let config = device.default_input_config().unwrap().config();
        Self { device, config }
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn config(&self) -> &StreamConfig {
        &self.config
    }
}

pub fn build_input_stream(
    latest_captured_samples: RBuffer,
    audio_device: AudioDevice,
) -> Result<Stream> {
    let dev = audio_device.device();
    let cfg = audio_device.config();
    let stream = dev.build_input_stream(
        cfg,
        move |data: &[f32], _info| {
            let mut audio_buf = latest_captured_samples.lock().unwrap();
        },
        |err| {
            eprintln!("got stream error: {}", err.to_string());
        },
        None,
    )?;
    Ok(stream)
}

pub fn list_input_devs() -> Vec<(String, Device)> {
    let host = default_host();
    let mut devs: Vec<(String, Device)> = host
        .input_devices()
        .unwrap()
        .map(|dev| {
            (
                dev.name().unwrap_or_else(|_| String::from("<unknown>")),
                dev,
            )
        })
        .collect();
    devs.sort_by(|(n1, _), (n2, _)| n1.cmp(n2));
    devs
}
