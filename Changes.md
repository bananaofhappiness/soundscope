# Changelog

All notable changes to this project will be documented in this file.

---
## [1.4.1] - 2025-10-17

### Fixes
- The program crashed when trying to play an audio file without selecting it beforehand.
- LUFS was displayed incorrectly during playback when both the audio file and microphone input capture modes were enabled: the program was picking up input from both sources simultaneously.

### Added
- Added functionality to seek a file even after it has reached the end. Previously, it was necessary to reopen the file for this.

### Known Issues
