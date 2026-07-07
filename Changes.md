# Changelog

All notable changes to this project will be documented in this file.

---
## [1.10.0] - 2026-07-07

### Features
- **Added** a one-shot, non-interactive `--waveform` mode: `soundscope --waveform FILE` decodes the file, prints a static single-row waveform with a time axis to stdout, and exits (no TUI). Output is pipe-friendly. Use `--width <columns>` to override the default terminal width.

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
