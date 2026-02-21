# Changelog

All notable changes to this project will be documented in this file.

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
- **Audio decoding**: Added `symphonia-all` feature to rodio for broader audio format support.
- **File explorer**: Updated to use my fork of ratatui-explorer with added `filter` function.
- **UI layout**: Refactored internal UI layout structure for better maintainability.
- **Dependencies**: Added `tui-big-text` crate for rendering large text in empty states.
- **LUFS display precision**: Reduced LUFS values display precision from 2 to 1 decimal place for cleaner UI.
- **True Peak display precision**: Reduced True Peak values display precision from 2 to 1 decimal place.
- **Audio file loading**: Changed from non-blocking `try_recv` to blocking `recv` in audio file receiver thread for more reliable file loading.
- **Key handling**: Prevented mode switch (`m` key) when popups (devices, explorer, themes) are open to avoid accidental mode changes.
- **Code style**: Updated `get_error_popup_area` to use array destructuring pattern (`let [_, area] = ...`).
