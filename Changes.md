# Changelog

All notable changes to this project will be documented in this file.

---
## [1.7.0] - 2026-02-07

### Fixes
- Fixed potential panic in `get_fft` when spectrum computation fails (usually caused by using a microphone with low sample rate).
- Fixed inefficient waveform rendering causing excessive CPU usage.
- Fixed unnecessary redraws when no state changes occurred.

### Changes
- **Performance**: Implemented render throttling - UI now only redraws when actual state changes occur (playhead movement, timers, user input).
- **Performance**: Optimized waveform rendering with better pre-allocation and more efficient min-max calculation.
- **Performance**: Simplified FFT computation pipeline, combining log-scale transformation into a single pass.
- **Performance**: Removed redundant intermediate data structures in FFT processing.
- Added `profiling` profile for performance analysis with debug info enabled.
- Improved error handling for FFT analysis with proper error propagation and user-friendly messages.
- Refactored time calculations in waveform rendering for better clarity and efficiency.
- Cleaned up unused variables in waveform rendering code.
- Reduced redundant allocations in waveform data processing.

### Added
- Added CLI arguments support: `-h/--help` for usage information and `-v/--version` for version display.
- Added one-time waveform pre-computation when loading audio files instead of calculating on the fly.

### Known Issues
- Rapidly seeking through an audio file may cause lag, resulting in the playhead being in an incorrect position. Pausing playback and waiting for the playhead to return to the correct spot before resuming usually resolves the issue.
- In `.m4a` files, the playhead may gradually drift further to the right over time.
