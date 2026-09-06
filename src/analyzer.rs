//! This module is responsible for analyzing audio files.
//! Taking samples it returns the loudness and spectrum.

use ebur128::{EbuR128, Mode};
use eyre::Result;
use spectrum_analyzer::{
    FrequencyLimit, samples_fft_to_spectrum, scaling::SpectrumDataStats, windows::hann_window,
};

// Approach from <https://dsp.stackexchange.com/questions/32076/fft-to-spectrum-in-decibel>:
fn scale_to_dbfs(val: f32, stats: &SpectrumDataStats) -> f32 {
    const REFERENCE_DBFS: f32 = 1.0;

    // stats.n is the length of the FFT window (N)
    let n = stats.n;

    // For Hann window: sum ≈ N/2
    // Formula: 20 * log10(val * 2 / sum(window) / reference) + calibration
    // Simplified: 20 * log10(val * 4 / N) + calibration
    if val == 0.0 {
        // Return a very low value instead of -infinity
        -150.0
    } else {
        let scaled = val * 4.0 / n;
        20.0 * (scaled / REFERENCE_DBFS).log10()
    }
}

pub struct Analyzer {
    loudness_meter: EbuR128,
    sample_rate: u32,
}

impl Default for Analyzer {
    fn default() -> Self {
        let loudness_meter = match EbuR128::new(2, 44100, Mode::all()) {
            Ok(loudness_meter) => loudness_meter,
            Err(err) => panic!("Failed to create loudness meter: {err}"),
        };
        Self {
            loudness_meter,
            sample_rate: 44100,
        }
    }
}

impl Analyzer {
    /// used when new file or device selected
    pub fn create_loudness_meter(&mut self, channels: u32, rate: u32) -> Result<()> {
        self.sample_rate = rate;
        self.loudness_meter = EbuR128::new(channels, rate, Mode::all())?;
        Ok(())
    }

    pub fn get_spectrum(&self, samples: &[f32]) -> Result<Vec<(f64, f64)>> {
        // apply hann window for smoothing
        let hann_window = hann_window(samples);

        let max_frequency = (self.sample_rate as f32 / 2.0).min(20000.);

        // calc spectrum with proper dBFS scaling
        let spectrum = samples_fft_to_spectrum(
            &hann_window,
            self.sample_rate,
            FrequencyLimit::Range(20., max_frequency),
            Some(&scale_to_dbfs),
        )?;

        // Reference frequency for pink noise compensation (1 kHz is standard)
        const PINK_NOISE_REF_FREQ: f64 = 1000.;
        // Pink noise compensation: +3 dB/octave to make pink noise appear flat
        // on a logarithmic frequency scale.
        // Formula: 3 dB/octave = 10 × log10(freq/ref)
        const PINK_NOISE_SLOPE: f64 = 10.;

        // Collect data with pink noise compensation
        let data: Vec<(f64, f64)> = spectrum
            .data()
            .iter()
            .map(|(freq, val)| {
                let freq = freq.val() as f64;
                let val = val.val() as f64;

                let compensation = PINK_NOISE_SLOPE * (freq / PINK_NOISE_REF_FREQ).log10();
                (freq, val + compensation)
            })
            .collect();

        // Convert to log scale for display
        let min_freq_log = 20_f64.log10();
        let max_freq_log = 20000_f64.log10();
        let log_range = max_freq_log - min_freq_log;
        let chart_width = 100.;

        let spectrum_vec = data
            .into_iter()
            .map(|(freq, val)| {
                let log_freq = freq.log10();
                let normalized_pos = (log_freq - min_freq_log) / log_range;
                let chart_x = normalized_pos * chart_width;

                (chart_x, val)
            })
            .collect();

        Ok(spectrum_vec)
    }

    pub fn get_waveform(samples: &[f32], waveform_window: f64) -> Vec<(f64, f64)> {
        let window = (waveform_window * 1000.) as usize;
        let samples_per_point = samples.len() as f64 / window as f64;

        // pre-allocate with 2 points per window position
        let mut points = Vec::with_capacity(window * 2);

        // min-max decimation
        let samples_len = samples.len();

        for i in 0..window {
            let start = (i as f64 * samples_per_point) as usize;
            let end = ((i + 1) as f64 * samples_per_point).ceil() as usize;
            let end = end.min(samples_len);

            if start >= samples_len {
                break;
            }

            let chunk = &samples[start..end];

            let min = chunk.iter().copied().reduce(f32::min).unwrap_or(0.0);
            let max = chunk.iter().copied().reduce(f32::max).unwrap_or(0.0);

            let x = i as f64;
            points.push((x, min as f64));
            points.push((x, max as f64));
        }

        points
    }

    pub fn add_samples(&mut self, samples: &[f32]) -> Result<(), ebur128::Error> {
        self.loudness_meter.add_frames_f32(samples)
    }

    pub fn reset(&mut self) {
        self.loudness_meter.reset();
    }

    pub fn get_shortterm_lufs(&mut self) -> Result<f64, ebur128::Error> {
        self.loudness_meter.loudness_shortterm()
    }

    pub fn get_integrated_lufs(&mut self) -> Result<f64, ebur128::Error> {
        self.loudness_meter.loudness_global()
    }

    pub fn get_loudness_range(&mut self) -> Result<f64, ebur128::Error> {
        self.loudness_meter.loudness_range()
    }

    pub fn get_true_peak(&mut self) -> Result<(f64, f64), ebur128::Error> {
        let tp_left = self.loudness_meter.true_peak(0)?;
        let tp_right = self.loudness_meter.true_peak(1)?;

        Ok((tp_left, tp_right))
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn calculate_integrated_lufs(&mut self, channels: u32, samples: &[f32]) -> Option<f64> {
        let Ok(mut analyzer) = EbuR128::new(channels, self.sample_rate, Mode::all()) else {
            return None;
        };

        for chunk in samples.chunks(self.sample_rate as usize * 2) {
            if analyzer.add_frames_f32(chunk).is_err() {
                return None;
            }
        }

        analyzer.loudness_global().ok()
    }
}

/// Returns the FFT window size in samples (a power of two) for the given
/// sample rate, but never longer in time than the proven 16384-sample window
/// at 44.1 kHz (~371 ms).
//  Useful when we are dealing with devices with sample rate
//  lower than this. E.g. 16k microphone with 16384 sample rate window will
//  take ~1 second to update the fft
pub fn fft_window_size(sample_rate: u32) -> usize {
    const MAX_WINDOW: usize = 16384;
    // largest power of two N with N / sample_rate <= 16384 / 44100,
    // rounded down
    let target = (sample_rate as usize * MAX_WINDOW / 44100).max(1);
    let window = 1 << (usize::BITS - 1 - target.leading_zeros());
    window.clamp(1024, MAX_WINDOW)
}

pub fn get_mid_and_side_samples(samples: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let left_samples = samples.iter().step_by(2).copied().collect::<Vec<f32>>();
    let right_samples = samples
        .iter()
        .skip(1)
        .step_by(2)
        .copied()
        .collect::<Vec<f32>>();
    let mid_samples = left_samples
        .iter()
        .zip(right_samples.iter())
        .map(|(l, r)| (l + r) / 2.)
        .collect::<Vec<f32>>();
    let side_samples = left_samples
        .iter()
        .zip(right_samples.iter())
        .map(|(l, r)| (l - r) / 2.)
        .collect::<Vec<f32>>();
    (mid_samples, side_samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Tests the FFT functionality with a simple sine wave
    fn test_get_spectrum() {
        let analyzer = Analyzer::default();

        // Generate a simple sine wave at 440Hz with amplitude 1.0 (0 dBFS for float)
        // Note: 440Hz doesn't align perfectly with FFT bins, so some spectral leakage is expected
        let sample_rate = 44100;
        let frequency = 440.0;
        // 16384 samples (power of 2)
        let samples: Vec<f32> = (0..16384_usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * frequency * t).sin()
            })
            .collect();

        let spectrum = analyzer.get_spectrum(&samples).unwrap();

        // Find max to verify calibration is reasonable
        let max_db = spectrum
            .iter()
            .map(|(_, db)| *db)
            .fold(f64::NEG_INFINITY, f64::max);

        println!(
            "440Hz sine wave (off-bin): Max dB = {max_db} (expected ~ -1 to -2 dB due to spectral leakage)"
        );

        // Should have some data points
        assert!(!spectrum.is_empty());
    }

    #[test]
    /// Tests that a 0 dBFS sine wave is displayed at approximately 0 dB on the spectrum.
    /// This verifies the FFT calibration is correct (before pink noise compensation).
    fn test_dbfs_calibration() {
        let analyzer = Analyzer::default();

        // FFT frequency resolution for 16384 samples at 44100 Hz
        // resolution = 44100 / 16384 ≈ 2.69 Hz per bin
        // We use 1 kHz as the reference frequency since pink noise compensation
        // is normalized to 1 kHz (where compensation = 0 dB)
        let sample_rate = 44100;
        let fft_resolution = sample_rate as f32 / 16384.0;
        let target_bin = (1000.0 / fft_resolution).round() as u32; // ~372 bins for 1 kHz
        let frequency = target_bin as f32 * fft_resolution;

        // Generate 0 dBFS sine wave (amplitude 1.0)
        let samples: Vec<f32> = (0..16384_usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * frequency * t).sin()
            })
            .collect();

        let spectrum = analyzer.get_spectrum(&samples).unwrap();

        // Find the maximum value in the spectrum
        let max_db = spectrum
            .iter()
            .map(|(_, db)| *db)
            .fold(f64::NEG_INFINITY, f64::max);

        println!("Frequency: {frequency} Hz (bin {target_bin})");
        println!(
            "Max dB value: {max_db} dB (expected: ~0 dB for 0 dBFS sine wave at ref frequency)",
        );

        // A 0 dBFS sine wave at 1 kHz should display at approximately 0 dB
        // (pink noise compensation is 0 dB at 1 kHz reference frequency)
        // We allow tolerance for windowing and FFT imperfections
        assert!(max_db >= -1.0, "Max dB {max_db} is too low, expected ~0 dB");
        assert!(max_db <= 1.0, "Max dB {max_db} is too high, expected ~0 dB");
    }

    #[test]
    /// Tests that pink noise compensation is applied correctly.
    /// A sine wave at 125 Hz should appear ~9 dB lower than at 1 kHz
    /// (three octaves below = 3 × 3 dB = 9 dB compensation).
    fn test_pink_noise_compensation() {
        let analyzer = Analyzer::default();

        let sample_rate = 44100;
        let fft_resolution = sample_rate as f32 / 16384.0;

        // Test at 1 kHz (reference frequency, compensation = 0 dB)
        let bin_1khz = (1000.0 / fft_resolution).round() as u32;
        let freq_1khz = bin_1khz as f32 * fft_resolution;

        let samples_1khz: Vec<f32> = (0..16384_usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * freq_1khz * t).sin()
            })
            .collect();

        let spectrum_1khz = analyzer.get_spectrum(&samples_1khz).unwrap();
        let max_1khz = spectrum_1khz
            .iter()
            .map(|(_, db)| *db)
            .fold(f64::NEG_INFINITY, f64::max);

        // Test at 125 Hz (three octaves below 1 kHz, compensation ≈ -9 dB)
        let bin_125hz = (125.0 / fft_resolution).round() as u32;
        let freq_125hz = bin_125hz as f32 * fft_resolution;

        let samples_125hz: Vec<f32> = (0..16384_usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * freq_125hz * t).sin()
            })
            .collect();

        let spectrum_125hz = analyzer.get_spectrum(&samples_125hz).unwrap();
        let max_125hz = spectrum_125hz
            .iter()
            .map(|(_, db)| *db)
            .fold(f64::NEG_INFINITY, f64::max);

        println!("1 kHz: {max_1khz} dB, 125 Hz: {max_125hz} dB");
        println!(
            "Difference: {} dB (expected ~ -9 dB due to pink noise compensation, 3 octaves × 3 dB/octave)",
            max_125hz - max_1khz
        );

        // The 125 Hz tone should appear ~9 dB lower than 1 kHz
        // (3 octaves × 3 dB/octave = 9 dB)
        let diff = max_125hz - max_1khz;
        assert!(
            (-10.5..=-8.0).contains(&diff),
            "Pink noise compensation not working correctly: expected ~-9 dB difference, got {diff}"
        );
    }

    #[test]
    /// Tests that low sample rates (e.g. 16 kHz Bluetooth microphones) are
    /// supported: the frequency range is capped at the Nyquist frequency.
    fn test_get_spectrum_low_sample_rate() {
        let mut analyzer = Analyzer::default();
        analyzer.create_loudness_meter(1, 16000).unwrap();

        let sample_rate = 16000;
        // 500 Hz lands exactly on a bin: 16000 / 16384 * 512 == 500
        let frequency = 500.0f32;
        let samples: Vec<f32> = (0..16384_usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * frequency * t).sin()
            })
            .collect();

        let spectrum = analyzer.get_spectrum(&samples).unwrap();

        // the peak must be at ~500 Hz
        let (_, (chart_x, _)) = spectrum
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.1.partial_cmp(&b.1.1).unwrap())
            .unwrap();
        // chart_x is the logarithmic position on the 20 Hz..20000 Hz axis
        let peak_freq = 20. * 1000f64.powf(chart_x / 100.);
        assert!(
            (450.0..=550.0).contains(&peak_freq),
            "Expected peak near {frequency} Hz, got {peak_freq} Hz"
        );

        // the spectrum must end at the Nyquist frequency (8 kHz), which is
        // chart_x = log10(8000/20) / log10(20000/20) * 100 ≈ 86.7
        let last_chart_x = spectrum.last().unwrap().0;
        assert!(
            (86.0..=87.0).contains(&last_chart_x),
            "Spectrum must end at Nyquist (chart_x ≈ 86.7), got {last_chart_x}"
        );
    }

    #[test]
    /// Tests that the FFT window is the full 16384 samples for 44.1 kHz and
    /// above, and that lower rates never exceed the ~371 ms reference
    /// duration (rounding down to a power of two).
    fn test_fft_window_size() {
        assert_eq!(fft_window_size(8000), 2048); // 256 ms
        assert_eq!(fft_window_size(16000), 4096); // 256 ms
        assert_eq!(fft_window_size(22050), 8192); // ~371 ms
        assert_eq!(fft_window_size(44100), 16384); // ~371 ms
        assert_eq!(fft_window_size(48000), 16384); // ~341 ms
        assert_eq!(fft_window_size(96000), 16384); // ~171 ms
        assert_eq!(fft_window_size(192000), 16384); // ~85 ms
        // degenerate inputs stay in bounds
        assert_eq!(fft_window_size(0), 1024);
        assert_eq!(fft_window_size(u32::MAX), 16384);
    }

    #[test]
    /// Tests the waveform generation
    fn test_get_waveform() {
        let samples: Vec<f32> = (0..44100).map(|i| (i as f32 / 44100.0).sin()).collect();

        let waveform = Analyzer::get_waveform(&samples, 15.);

        // Should have data points
        assert!(!waveform.is_empty());

        // With 15 seconds window, we expect 15000 points (15 * 1000)
        // Each point has min and max, so total should be 30000
        let expected_points = 15_000 * 2;
        assert_eq!(waveform.len(), expected_points);

        // Check that we have pairs of (x, min) and (x, max) for each x
        for i in 0..15_000 {
            let min_idx = i * 2;
            let max_idx = i * 2 + 1;

            // Both points should have the same x coordinate
            assert_eq!(waveform[min_idx].0, waveform[max_idx].0);
            assert_eq!(waveform[min_idx].0, i as f64);

            // Min should be <= max (or equal if constant)
            assert!(waveform[min_idx].1 <= waveform[max_idx].1);
        }

        // x values should be sequential integers (starting from i=2 to avoid underflow)
        for i in 2..15_000 {
            let min_idx = i * 2;
            let prev_min_idx = (i - 1) * 2;
            assert_eq!(waveform[min_idx].0, waveform[prev_min_idx].0 + 1.0);
        }
    }

    #[test]
    /// Tests loudness measurement functionality
    fn test_loudness_measurements() {
        let mut analyzer = Analyzer::default();

        // Generate some test audio (1 second of sine wave)
        let samples: Vec<f32> = (0..88200) // 2 seconds stereo at 44.1kHz
                .map(|i| 0.1 * (440.0 * 2.0 * std::f32::consts::PI * (i as f32 / 44100.0)).sin())
                .collect();

        let _ = analyzer.add_samples(&samples);

        // Test integrated loudness (should be valid after enough samples)
        if let Ok(lufs) = analyzer.get_integrated_lufs() {
            assert!(lufs < 0.0); // LUFS values are typically negative
            assert!(lufs > -100.0); // Reasonable range
        }

        // Test true peak
        if let Ok((left, right)) = analyzer.get_true_peak() {
            assert!(left >= 0.0);
            assert!(right >= 0.0);
            assert!(left <= 1.0);
            assert!(right <= 1.0);
        }
    }

    #[test]
    /// Tests analyzer reinitialization with different parameters
    fn test_analyzer_reinit() {
        let mut analyzer = Analyzer::default();

        // Test reinitializing with different parameters
        let result = analyzer.create_loudness_meter(1, 48000); // mono, 48kHz
        assert!(result.is_ok());

        let result = analyzer.create_loudness_meter(6, 96000); // 5.1 surround, 96kHz
        assert!(result.is_ok());
    }
}
