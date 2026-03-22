# Changelog

All notable changes to this project will be documented in this file.

---

> **Note:** The frequency spectrum visualization may appear noisy in this release. This will be improved in a future version once [ratatui#2426](https://github.com/ratatui/ratatui/pull/2426) is merged, which adds a filled-area chart rendering mode that will fill the area under the curve.

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
