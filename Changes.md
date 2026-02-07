# Changelog

All notable changes to this project will be documented in this file.

---
## [1.7.1] - 2026-02-07
Follow-up to 1.7.0 performance work.

### Fixes

### Changes
- **Performance**: FFT is now computed only if frequency spectrum is visible.

### Added

### Known Issues
- Rapidly seeking through an audio file may cause lag, resulting in the playhead being in an incorrect position. Pausing playback and waiting for the playhead to return to the correct spot before resuming usually resolves the issue.
- In `.m4a` files, the playhead may gradually drift further to the right over time.
