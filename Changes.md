# Changelog

All notable changes to this project will be documented in this file.

---

> **Note:** The frequency spectrum visualization may appear noisy. This will be improved in a future versions once ratatui v0.30.1 releases, which adds a filled-area chart rendering mode that will fill the area under the curve.

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
