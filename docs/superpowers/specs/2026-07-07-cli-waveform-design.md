# Design: One-shot `--waveform` CLI render mode

**Date:** 2026-07-07
**Status:** Approved
**Target:** upstream `bananaofhappiness/soundscope`

## Problem

Soundscope is an interactive `ratatui` TUI. It already computes and displays a
waveform (min-max decimation, `Analyzer::get_waveform`, `src/analyzer.rs:107`),
but only inside the full-screen event loop. There is no simple, pipe-friendly
way to render a static waveform from a file to the terminal and exit.

Surveyed alternatives do not fill this gap cleanly:

| Tool | Fit | Problem |
|------|-----|---------|
| ffmpeg `showwavespic` / BBC `audiowaveform` | file → image | Output is a PNG, not terminal text |
| `cava`, `cli-visualizer`, `cli-viz` | terminal | Real-time mic/playback bars, not one-shot from a file |
| `ascii-media` (Python), curzona gist | file → ASCII | Experimental / a snippet, not packaged or robust |

The DSP already lives in soundscope; the missing piece is a non-interactive
render path. This makes soundscope the right home and a useful upstream PR.

## Goal

Add a one-shot mode: `soundscope --waveform <FILE>` decodes the file, prints a
single-row block-character waveform plus a time axis to **stdout**, and exits.
Non-interactive and safe to pipe or redirect.

### Example output

```
  ▁▂▃▅▇█▇▅▃▂▁ ▂▄▆█▆▄▂ ▁▃▅▇▇▅▃▁
  0:00                     3:14
```

## Non-goals (v1 / YAGNI)

- Color output (monochrome only; theme color is a possible future flag).
- Multi-row / mirrored envelope rendering.
- Per-channel (stereo) display — audio is downmixed to mono.
- Image/PNG output.
- New arg-parsing dependency (e.g. `clap`) — keep the existing hand-rolled style.

## Chosen approach

A thin non-interactive path that reuses soundscope's existing `symphonia`
decode capability and the min-max decimation idea, with a **new pure,
terminal-independent rendering layer** that can be unit-tested without a TTY.

Rejected alternatives:
- **Reuse the ratatui chart pipeline** — coupled to the interactive event loop
  and terminal backend; not usable one-shot.
- **Shell out to ffmpeg** — adds a runtime dependency and defeats the purpose.

## Components

### 1. CLI surface — `src/main.rs`
- Detect a new `--waveform` flag in the existing hand-rolled `env::args()` block,
  alongside `-h`/`-v`.
- Optional `--width <N>` to override the column count.
- When `--waveform` is present: decode → render → print → `return Ok(())`
  **before** any TUI or audio-player threads are spawned.
- Bare file with no flag = TUI, behavior unchanged.
- Update `print_help()` to document `--waveform` and `--width`.
- Argument errors and decode/IO failures print to **stderr** and exit non-zero.
  Successful render prints to **stdout**.

### 2. Decode helper (reuse `symphonia`, already a dependency)
- `decode_to_mono(path) -> Result<(Vec<f32>, u32)>` returning mono samples and
  the sample rate. Interleaved channels are downmixed by averaging. No playback,
  no audio device is opened.

### 3. Render layer — new module `src/waveform_render.rs` (pure functions)
- `bucket_peaks(samples: &[f32], width: usize) -> Vec<f32>` — split samples into
  `width` contiguous buckets; each bucket's value is its peak `abs` amplitude.
  Handles `samples.len() < width` (fewer/degenerate buckets) without panicking.
- `render_row(peaks: &[f32]) -> String` — peak-normalize: divide each bucket by
  the file's maximum bucket peak so 0..1 maps across the full block range (a
  quiet track still shows its shape rather than a flat line). Map each normalized
  value to one of `▁▂▃▄▅▆▇█`; a silent/zero bucket renders as a space. If the
  whole file is silent (max peak is 0), render all spaces.
- `format_time(secs: f64) -> String` — `M:SS` (e.g. `194.0 → "3:14"`).
- `time_axis(total_secs: f64, width: usize) -> String` — start label `0:00`
  left-aligned and total-duration label right-aligned within `width`.
- Output is monochrome. The printed block row and axis are indented two spaces
  (matching the example).

### 4. Width resolution
- Default width from `crossterm::terminal::size()` (already a dependency), minus
  the 2-space indent.
- Fall back to **80** when stdout is not a TTY or the size query errors.
- `--width <N>` overrides both.

## Data flow

```
--waveform <FILE> [--width N]
        │
        ▼
decode_to_mono(path) ──► (Vec<f32> mono samples, sample_rate)
        │
        ▼
width = --width  OR  terminal width - 2  OR  80
        │
        ▼
bucket_peaks(samples, width) ──► Vec<f32> peaks
        │
        ▼
render_row(peaks)            ──► "▁▂▃▅▇█…"
time_axis(len/rate, width)   ──► "0:00        3:14"
        │
        ▼
print both lines to stdout, exit 0
```

## Error handling

- File does not exist / unreadable → stderr message, non-zero exit.
- Decode failure (unsupported/corrupt) → stderr message surfacing the underlying
  error, non-zero exit.
- Empty / zero-length audio → print an empty (or all-space) row and a `0:00`
  axis rather than panicking.
- `--width` value that is non-numeric or `0` → stderr usage error, non-zero exit.

## Testing

Following the existing `#[cfg(test)]` style in `src/analyzer.rs`:

- `bucket_peaks`: correct bucket count for `samples.len() >= width`; peak chosen
  per bucket; graceful handling when `samples.len() < width`; empty input.
- `render_row`: amplitude→char mapping at boundaries, full-scale → `█`,
  silence → space, normalization.
- `format_time`: `0.0 → "0:00"`, `194.0 → "3:14"`, rounding behavior.
- `time_axis`: start/end labels placed correctly and total length == `width`.
- Integration-ish: generate a tiny in-memory/temp WAV, run the decode+render
  path, assert two non-empty lines are produced.

## Files touched

- `src/main.rs` — flag parsing, dispatch, help text.
- `src/waveform_render.rs` — **new**, pure render/format functions + tests.
- Possibly a small decode helper in `src/main.rs` or a new `src/decode.rs`
  (decided during planning; keep it minimal).
- `README.md` — document the new one-shot usage (docs-review step).
- `Changes.md` — changelog entry.
