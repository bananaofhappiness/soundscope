# Changelog

All notable changes to this project will be documented in this file.

---
## [1.6.0] - 2026-01-24

### Fixes
- Fixed waveform flickering.
- Fixed washed-out colors.

### Changes
- Waveform rendering engine: Replaced naive "every N-th sample" decimation with a proper Min-Max Decimation algorithm. This ensures that audio transients (peaks) are always captured and displayed regardless of the zoom level, preventing aliasing and visual "jitter" when scrolling.
- Updated keybindings in README to use lowercase letters where applicable for better clarity.
- Removed hundredth from total file duration and current playback time. 

### Added
- Added support for passing an audio file path as a command-line argument to open files directly on startup.

### Known Issues
