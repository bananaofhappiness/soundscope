//! One-shot, non-interactive waveform rendering for the `--waveform` CLI mode.
//!
//! The pure functions in this module are terminal-independent and unit-tested.
//! I/O (decoding a file, querying terminal width, printing) is kept in the thin
//! `run` orchestrator so the rendering logic can be exercised without a TTY.

use crate::audio_player::AudioFile;
use crossterm::terminal;
use eyre::Result;
use std::path::Path;

/// Block characters from shortest to tallest, mapped to normalized amplitude.
const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Fallback column count when the terminal width can't be determined.
const FALLBACK_WIDTH: usize = 80;

/// Left indent applied to the printed row and axis.
const INDENT: &str = "  ";

/// Decode `path`, render a one-shot waveform, and print it to stdout.
///
/// `width_override` forces the column count; otherwise the terminal width
/// (minus the indent) is used, falling back to [`FALLBACK_WIDTH`].
pub fn run(path: &Path, width_override: Option<usize>) -> Result<()> {
    let (samples, sample_rate, channels) = AudioFile::decode_file(&path.to_path_buf())?;
    let mono = downmix_to_mono(&samples, channels.count());
    let width = resolve_width(width_override);
    println!("{}", render(&mono, sample_rate, width));
    Ok(())
}

/// Resolve the number of columns to render: the explicit override if given,
/// otherwise the current terminal width minus the indent, falling back to
/// [`FALLBACK_WIDTH`] when the width can't be determined (e.g. piped output).
fn resolve_width(width_override: Option<usize>) -> usize {
    if let Some(w) = width_override {
        return w.max(1);
    }
    let cols = terminal::size()
        .map(|(c, _)| c as usize)
        .unwrap_or(FALLBACK_WIDTH);
    cols.saturating_sub(INDENT.len()).max(1)
}

/// Average interleaved multi-channel samples down to a single mono channel.
///
/// `channels` is the number of interleaved channels. A frame is `channels`
/// consecutive samples; each mono sample is the mean of its frame. A trailing
/// partial frame (if any) is averaged over the samples it does have.
pub fn downmix_to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    let ch = channels.max(1);
    if ch == 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks(ch)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

/// Split `samples` into `width` contiguous buckets, returning the peak absolute
/// amplitude of each bucket. Output length is always `width`; buckets that map
/// to no samples yield `0.0`. Returns an empty vec when `width == 0`.
pub fn bucket_peaks(samples: &[f32], width: usize) -> Vec<f32> {
    if width == 0 {
        return Vec::new();
    }
    let len = samples.len();
    if len == 0 {
        return vec![0.0; width];
    }
    let per = len as f64 / width as f64;
    (0..width)
        .map(|i| {
            let start = (i as f64 * per) as usize;
            let end = (((i + 1) as f64 * per).ceil() as usize).min(len);
            if start >= end {
                0.0
            } else {
                samples[start..end]
                    .iter()
                    .map(|s| s.abs())
                    .fold(0.0f32, f32::max)
            }
        })
        .collect()
}

/// Peak-normalize `peaks` (divide by the max peak) and map each to a block
/// character. A zero bucket renders as a space. If every peak is zero, the row
/// is all spaces.
pub fn render_row(peaks: &[f32]) -> String {
    let max = peaks.iter().copied().fold(0.0f32, f32::max);
    if max <= 0.0 {
        return " ".repeat(peaks.len());
    }
    peaks
        .iter()
        .map(|&p| {
            if p <= 0.0 {
                ' '
            } else {
                let norm = p / max;
                let idx = ((norm * BLOCKS.len() as f32).ceil() as usize)
                    .clamp(1, BLOCKS.len())
                    - 1;
                BLOCKS[idx]
            }
        })
        .collect()
}

/// Format a duration in seconds as `M:SS`, flooring to whole seconds.
pub fn format_time(secs: f64) -> String {
    let total = secs.max(0.0).floor() as u64;
    let m = total / 60;
    let s = total % 60;
    format!("{m}:{s:02}")
}

/// Build a time axis: `0:00` left-aligned and the total duration right-aligned,
/// padded with spaces to `width` columns.
pub fn time_axis(total_secs: f64, width: usize) -> String {
    let start = format_time(0.0);
    let end = format_time(total_secs);
    let min = start.chars().count() + end.chars().count() + 1;
    if width < min {
        return format!("{start} {end}");
    }
    let pad = width - start.chars().count() - end.chars().count();
    format!("{start}{}{end}", " ".repeat(pad))
}

/// Assemble the two output lines (block row + time axis), each indented, joined
/// by a newline. Pure: takes decoded mono samples so it can be tested without
/// touching the filesystem or a terminal.
pub fn render(mono: &[f32], sample_rate: u32, width: usize) -> String {
    let total_secs = if sample_rate > 0 {
        mono.len() as f64 / sample_rate as f64
    } else {
        0.0
    };
    let peaks = bucket_peaks(mono, width);
    format!(
        "{INDENT}{}\n{INDENT}{}",
        render_row(&peaks),
        time_axis(total_secs, width)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cc(s: &str) -> usize {
        s.chars().count()
    }

    #[test]
    fn render_composes_indented_row_and_axis() {
        // 8 full-scale samples at 8 Hz == 1.0s; 4 columns.
        let out = render(&vec![1.0f32; 8], 8, 4);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "  ████");
        assert!(lines[1].starts_with("  0:00"));
        assert!(lines[1].contains("0:01"));
    }

    #[test]
    fn render_handles_zero_sample_rate_without_panicking() {
        let out = render(&[0.5, 0.5], 0, 4);
        assert_eq!(out.lines().count(), 2);
    }

    // --- downmix_to_mono ---

    #[test]
    fn mono_input_is_unchanged() {
        assert_eq!(downmix_to_mono(&[0.1, 0.2, 0.3], 1), vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn stereo_frames_are_averaged() {
        // frames: (1.0, -1.0) -> 0.0 ; (0.5, 0.5) -> 0.5
        assert_eq!(downmix_to_mono(&[1.0, -1.0, 0.5, 0.5], 2), vec![0.0, 0.5]);
    }

    #[test]
    fn downmix_empty_is_empty() {
        assert!(downmix_to_mono(&[], 2).is_empty());
    }

    // --- bucket_peaks ---

    #[test]
    fn peaks_use_absolute_value() {
        // two buckets of two samples each; peak is max |sample|
        assert_eq!(bucket_peaks(&[-0.9, 0.1, 0.2, -0.8], 2), vec![0.9, 0.8]);
    }

    #[test]
    fn single_bucket_is_global_peak() {
        assert_eq!(bucket_peaks(&[-0.9, 0.1, 0.2, -0.8], 1), vec![0.9]);
    }

    #[test]
    fn width_zero_is_empty() {
        assert!(bucket_peaks(&[0.5, 0.3], 0).is_empty());
    }

    #[test]
    fn fewer_samples_than_width_does_not_panic_and_keeps_width() {
        let peaks = bucket_peaks(&[0.5, 0.3], 4);
        assert_eq!(peaks.len(), 4);
        // the loudest sample must survive somewhere
        let max = peaks.iter().copied().fold(0.0f32, f32::max);
        assert_eq!(max, 0.5);
    }

    // --- render_row ---

    #[test]
    fn full_scale_single_peak_is_full_block() {
        assert_eq!(render_row(&[1.0]), "█");
    }

    #[test]
    fn silence_renders_as_spaces() {
        assert_eq!(render_row(&[0.0, 0.0]), "  ");
    }

    #[test]
    fn zero_bucket_among_signal_is_a_space() {
        assert_eq!(render_row(&[0.0, 1.0]), " █");
    }

    #[test]
    fn peak_normalization_preserves_shape_of_quiet_signal() {
        // A quiet track (0.1, 0.2) normalizes to the same shape as (0.5, 1.0).
        assert_eq!(render_row(&[0.1, 0.2]), render_row(&[0.5, 1.0]));
        assert_eq!(render_row(&[0.5, 1.0]), "▄█");
    }

    // --- format_time ---

    #[test]
    fn format_time_zero() {
        assert_eq!(format_time(0.0), "0:00");
    }

    #[test]
    fn format_time_pads_seconds() {
        assert_eq!(format_time(9.0), "0:09");
    }

    #[test]
    fn format_time_minutes_and_seconds() {
        assert_eq!(format_time(194.0), "3:14");
    }

    #[test]
    fn format_time_floors_fractional_seconds() {
        assert_eq!(format_time(194.7), "3:14");
    }

    // --- time_axis ---

    #[test]
    fn time_axis_places_labels_at_both_ends() {
        let axis = time_axis(194.0, 20);
        assert_eq!(cc(&axis), 20);
        assert!(axis.starts_with("0:00"));
        assert!(axis.ends_with("3:14"));
    }

    #[test]
    fn time_axis_narrow_width_still_shows_both_labels() {
        let axis = time_axis(194.0, 3);
        assert!(axis.contains("0:00"));
        assert!(axis.contains("3:14"));
    }
}
