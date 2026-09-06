# Changelog

All notable changes to this project will be documented in this file.

---

## [1.10.0] - 2026-09-06

### Features
- **Added** system audio capture (macOS only for now)! To let the app capture your screen and system audio, grant System Audio Recording permission in System Settings > Privacy & Security > Screen & System Audio Recording.
- Improve support for mono files and devices.
- Microphones with low sample rate are now also supported.

---

## [1.9.2] - 2026-06-09

### Fixes
- **Fixed** freeze when directory passed as argument. Set it as the current working directory.
- **Fixed** FFT scaling, set range to -100..-18 dBFS

### Changes
- Use a filled-area chart rendering mode for the spectrum analyzer and LUFS. Now they look better and the spectrum analyzer doesn't appear noisy.

### Known Issues
- Rapidly seeking through an audio file may cause lag, resulting in the playhead being in an incorrect position. Pausing playback and waiting for the playhead to return to the correct spot before resuming usually resolves the issue.
- In some audio files the playhead may gradually drift slightly to the right of the waveform center over time.
- Some files with high sample rate play back normally, but real-time visualization (waveform, spectrum, LUFS) lags significantly behind. For example, this occurs with `.wav` files with a sample rate > 48000, while `.mp3` files at 92000 play without noticeable lag.

---

## [1.9.1] - 2026-05-05

### Features
- **Added** support for `.toml` theme files. Using `.toml` instead of the custom `.theme` extension enables syntax highlighting in editors, since the format was always TOML under the hood. It is recommended to rename existing `.theme` files to `.toml`, although `.theme` files are still supported for backwards compatibility.

### Changes
- **Fixed** config directory and `.current_theme` file being created on startup even when the user hasn't changed the theme from default. Now they are only created once the user actually switches to a non-default theme ([#64](https://github.com/bananaofhappiness/soundscope/issues/64)).

### Known Issues
- Rapidly seeking through an audio file may cause lag, resulting in the playhead being in an incorrect position. Pausing playback and waiting for the playhead to return to the correct spot before resuming usually resolves the issue.
- In some audio files the playhead may gradually drift slightly to the right of the waveform center over time.
- Some files with high sample rate play back normally, but real-time visualization (waveform, spectrum, LUFS) lags significantly behind. For example, this occurs with `.wav` files with a sample rate > 48000, while `.mp3` files at 92000 play without noticeable lag.

---

## [1.9.0] - 2026-03-22

### Features
- **Added** dBFS (decibels relative to full scale) scaling for accurate spectrum visualization.
- **Added** pink noise compensation (+3 dB/octave) to make pink noise appear flat on the logarithmic frequency scale.
- **Added** FFT normalization so that songs with lower loudness don't appear at the bottom of the chart.

### Fixes
- **Fixed** custom theme selection issue.

### Changes
- **Updated** TUI Y-axis bounds to display the new 0 to -100 dB range.

### Known Issues
- Rapidly seeking through an audio file may cause lag, resulting in the playhead being in an incorrect position. Pausing playback and waiting for the playhead to return to the correct spot before resuming usually resolves the issue.
- In some audio file formats, the playhead may gradually drift slightly to the right of the waveform center over time.

---

## [1.8.0] - 2026-02-21

### Added
- **Built-in themes**: Added 16 pre-defined color schemes including popular themes like Catppuccin (4 variants), Dracula, Gruvbox Dark, Nord, Tokyo Night, One Dark/Light, Solarized Dark/Light, Monokai, Material Dark, Ayu Dark, and minimal Black & White themes.
- **Help popup**: Press `?`/`h`/`F1` to view a comprehensive help screen with all keyboard shortcuts and features.
- **Empty state display**: When no audio file or visualization is active, the app now displays helpful instructions and "Soundscope" title text using big text renderer.
- **Theme loading from file explorer**: You can now now load theme files directly from the file explorer.
- **Arrow key navigation for device list**: Added Up/Down arrow key navigation to the device list with Enter to select, matching the theme list behavior.

### Fixes
- **Fixed** waveform not rendering after switching from microphone input back to audio file.
- **Fixed** theme explorer functionality to properly display and select themes.
- **Fixed** startup race condition - the application now properly waits for audio file to load before rendering the UI.

### Changes
- **Config directory locations**: Updated config directory paths for consistency across platforms:
  - Linux/Unix/BSD/macOS: `~/.config/soundscope`;
  - Windows: Changed from `%APPDATA%\Roaming\soundscope` to `%LOCALAPPDATA%\soundscope` (Local AppData instead of Roaming);
  - On macOS specifically, changed from `~/Library/Application Support/soundscope` to `~/.config/soundscope`;
- **Dependencies**: Added `tui-big-text` crate for rendering large text in empty states.
- **LUFS display precision**: Reduced LUFS values display precision from 2 to 1 decimal place for cleaner UI.
- **True Peak display precision**: Reduced True Peak values display precision from 2 to 1 decimal place.
- **Key handling**: Prevented mode switch (`m` key) when popups (devices, explorer, themes) are open to avoid accidental mode changes.

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
- The application crashes when trying to select a microphone with low sample rate (at least ≤16000).
- Unsuppressed ALSA error messages when device list is open on Linux.

---

## [1.5.0] - 2025-12-15

### Added
- Added the ability to hover over the FFT chart to see additional information about frequency and amplitude.

---

## [1.4.1] - 2025-10-17

### Fixes
- The program crashed when trying to play an audio file without selecting it beforehand.
- LUFS was displayed incorrectly during playback when both the audio file and microphone input capture modes were enabled: the program was picking up input from both sources simultaneously.

### Added
- Added functionality to seek a file even after it has reached the end. Previously, it was necessary to reopen the file for this.

---

## [1.4.0] - 2025-10-05

### Fixes
- The explorer and device list borders had the wrong color in the default theme.
- Microphone input now stops when not in microphone capture mode.
- A missing .theme file previously caused an error on every startup; now a single notification is shown and the theme is reset to default.

### Added
- The ability to zoom in and out of the waveform.
- More options for theme customization.

---

## [1.3.0] - 2025-10-05

### Fixes
- Fixed excessive CPU usage (over 90%).
- Added mono-channel support for microphone input to resolve latency issues on some microphones.

---

## [1.2.0] - 2025-09-19

### Added
- Custom themes support. README contains a guide on how to create a custom theme.
- QoL: using arrow keys in the explorer no longer seeks the audio.

---

## [1.1.0] - 2025-09-06

### Added
- Microphone input support with real-time analysis

---

## [1.0.2] - 2025-09-06

### Added
- Removed twitching of a shortterm LUFS when it goes from 2-digit number to 1-digit number

### Known Issues
- Crashes may occur with files shorter than 15 seconds
- Playback cannot restart without reopening the file
- Untested with multi-channel (>2) audio

---

## [1.0.1] - 2025-09-06

### Added
- The app now updates LUFS only on play, not pause, so it looks smoother.

### Known Issues
- Crashes may occur with files shorter than 15 seconds
- Playback cannot restart without reopening the file
- Untested with multi-channel (>2) audio

---

## [1.0.0] - 2025-09-05

### Added
- Initial release 🎉
- FFT Spectrum visualization
- Waveform display with live playback
- LUFS metering and True Peak measurement
- File explorer with Vim-style navigation (`HJKL` / arrow keys)
- Playback controls (`Space`, `←`, `→`)
- Mid/Side frequency toggle (`M` / `S`)

### Known Issues
- Crashes may occur with files shorter than 15 seconds
- Playback cannot restart without reopening the file
- Untested with multi-channel (>2) audio
