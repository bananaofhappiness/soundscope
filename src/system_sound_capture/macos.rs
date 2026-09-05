// Approach's taken from official apple's documentation and
// from Minimeter's creator <https://gist.github.com/directmusic/7d653806c24fe5bb8166d12a9f4422de>

#![cfg(target_os = "macos")]
use crate::system_sound_capture::SYSTEM_TAP_DEVICE_NAME;
use std::{
    ffi::CStr,
    mem,
    ptr::{NonNull, null},
    thread,
    time::Duration,
};

use objc2::AnyThread as _;
use objc2_core_audio::{
    AudioDeviceID, AudioHardwareCreateAggregateDevice, AudioHardwareCreateProcessTap,
    AudioHardwareDestroyAggregateDevice, AudioHardwareDestroyProcessTap,
    AudioObjectGetPropertyData, AudioObjectID, AudioObjectPropertyAddress,
    AudioObjectPropertyElement, AudioObjectPropertyScope, AudioObjectPropertySelector,
    CATapDescription, kAudioAggregateDeviceIsPrivateKey, kAudioAggregateDeviceNameKey,
    kAudioAggregateDeviceTapAutoStartKey, kAudioAggregateDeviceTapListKey,
    kAudioAggregateDeviceUIDKey, kAudioDevicePropertyStreamConfiguration,
    kAudioObjectPropertyElementMain, kAudioObjectPropertyScopeGlobal,
    kAudioObjectPropertyScopeInput, kAudioSubTapDriftCompensationKey, kAudioSubTapUIDKey,
    kAudioTapPropertyUID,
};
use objc2_core_audio_types::{AudioBuffer, AudioBufferList};
use objc2_core_foundation::{CFArray, CFBoolean, CFDictionary, CFRetained, CFString, CFType};
use objc2_foundation::{NSArray, NSNumber};

pub struct SystemAudioDevice {
    id: AudioDeviceID,
    tap_id: AudioObjectID,
}

impl SystemAudioDevice {
    /// Creates a process tap and an aggregate device
    pub unsafe fn new() -> Self {
        unsafe {
            // make a tap
            let tap_desc = CATapDescription::initStereoGlobalTapButExcludeProcesses(
                CATapDescription::alloc(),
                &NSArray::<NSNumber>::new(),
            );
            tap_desc.setPrivate(true);
            tap_desc.setExclusive(true);

            let mut tap_id: AudioObjectID = 0;
            let status = AudioHardwareCreateProcessTap(Some(&tap_desc), &mut tap_id);
            assert_eq!(
                status,
                0,
                "AudioHardwareCreateProcessTap failed: {}",
                os_status_to_str(status)
            );

            // UID of a created tap
            let property_address = get_property_address(kAudioTapPropertyUID, None, None);
            let mut tap_uid_ref: *const CFString = null();
            let mut data_size = mem::size_of::<*const CFString>() as u32;
            let status = AudioObjectGetPropertyData(
                tap_id,
                NonNull::from(&property_address),
                0,
                null(),
                NonNull::from(&mut data_size),
                NonNull::from(&mut tap_uid_ref).cast(),
            );
            assert_eq!(
                status,
                0,
                "kAudioTapPropertyUID failed: {}",
                os_status_to_str(status)
            );
            let tap_uid: &CFString = &*tap_uid_ref;

            // aggregate device
            let tap_entry = CFDictionary::<CFString, CFType>::from_slices(
                &[
                    &cf_key(kAudioSubTapUIDKey),
                    &cf_key(kAudioSubTapDriftCompensationKey),
                ],
                &[tap_uid.as_ref(), CFBoolean::new(true).as_ref()],
            );
            let tap_list = CFArray::<CFType>::from_objects(&[tap_entry.as_ref()]);
            let description = CFDictionary::<CFString, CFType>::from_slices(
                &[
                    &cf_key(kAudioAggregateDeviceNameKey),
                    &cf_key(kAudioAggregateDeviceUIDKey),
                    &cf_key(kAudioAggregateDeviceIsPrivateKey),
                    &cf_key(kAudioAggregateDeviceTapListKey),
                    &cf_key(kAudioAggregateDeviceTapAutoStartKey),
                ],
                &[
                    CFString::from_str(SYSTEM_TAP_DEVICE_NAME).as_ref(),
                    CFString::from_str(&format!("soundscope.system-tap.{}", std::process::id()))
                        .as_ref(),
                    CFBoolean::new(true).as_ref(),
                    tap_list.as_ref(),
                    CFBoolean::new(false).as_ref(),
                ],
            );

            let mut id: AudioDeviceID = 0;
            let status =
                AudioHardwareCreateAggregateDevice(description.as_opaque(), NonNull::from(&mut id));
            assert_eq!(
                status,
                0,
                "AudioHardwareCreateAggregateDevice failed: {}",
                os_status_to_str(status)
            );

            // wait for an aggregate to initialize
            for _ in 0..40 {
                if scope_channel_count(id, kAudioObjectPropertyScopeInput) > 0 {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }

            Self { id, tap_id }
        }
    }
}

impl Drop for SystemAudioDevice {
    fn drop(&mut self) {
        unsafe {
            AudioHardwareDestroyAggregateDevice(self.id);
            AudioHardwareDestroyProcessTap(self.tap_id);
        }
    }
}

fn fourcc(value: u32) -> String {
    value
        .to_be_bytes()
        .iter()
        .map(|&b| {
            if (0x20..0x7f).contains(&b) {
                b as char
            } else {
                '?'
            }
        })
        .collect()
}

fn os_status_to_str(status: i32) -> String {
    if status == 0 {
        "0 (noErr)".to_string()
    } else {
        format!("{} ('{}')", status, fourcc(status as u32))
    }
}

fn cf_key(key: &'static CStr) -> CFRetained<CFString> {
    CFString::from_str(key.to_str().expect("non-utf8 key"))
}

unsafe fn scope_channel_count(id: AudioObjectID, scope: AudioObjectPropertyScope) -> u32 {
    unsafe {
        let property_address = AudioObjectPropertyAddress {
            mSelector: kAudioDevicePropertyStreamConfiguration,
            mScope: scope,
            mElement: kAudioObjectPropertyElementMain,
        };
        // AudioBufferList
        let mut storage =
            [0u8; mem::size_of::<AudioBufferList>() + 32 * mem::size_of::<AudioBuffer>()];
        let mut data_size = storage.len() as u32;
        let status = AudioObjectGetPropertyData(
            id,
            NonNull::from(&property_address),
            0,
            null(),
            NonNull::from(&mut data_size),
            NonNull::from(&mut storage).cast(),
        );
        if status != 0 {
            return 0;
        }
        let list = &*(storage.as_ptr() as *const AudioBufferList);
        (0..list.mNumberBuffers)
            .map(|i| list.mBuffers[i as usize].mNumberChannels)
            .sum()
    }
}

fn get_property_address(
    selector: AudioObjectPropertySelector,
    scope: Option<AudioObjectPropertyScope>,
    element: Option<AudioObjectPropertyElement>,
) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope.unwrap_or(kAudioObjectPropertyScopeGlobal),
        mElement: element.unwrap_or(kAudioObjectPropertyElementMain),
    }
}
