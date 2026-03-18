//! This module is responsible for analyzing audio files.
//! Taking samples it returns the loudness and spectrum.

use ebur128::{EbuR128, Mode};
use eyre::Result;
use spectrum_analyzer::{
    FrequencyLimit, samples_fft_to_spectrum, scaling::SpectrumDataStats, windows::hann_window,
};

/// Scaling function to convert FFT magnitude to dBFS (decibels relative to full scale).
/// This follows the standard approach from <https://dsp.stackexchange.com/questions/32076/fft-to-spectrum-in-decibel>:
/// 1. Multiply by 2 (for one-sided spectrum)
/// 2. Divide by sum of window (to compensate for windowing energy loss)
/// 3. Divide by reference value (1.0 for float audio, 32768 for int16)
/// 4. Convert to dB: 20 * `log10()`
/// 5. Apply calibration offset to ensure 0 dBFS sine wave shows as 0 dB
///
/// For Hann window, sum(window) ≈ N/2, so the formula becomes:
/// dB = 20 * log10(val * 2 / (N/2) / 1.0) = 20 * log10(val * 4 / N)
///
/// Calibration offset compensates for window function characteristics to ensure
/// that a full-scale (0 dBFS) sine wave displays at approximately 0 dB on the spectrum.
fn scale_to_dbfs(val: f32, stats: &SpectrumDataStats) -> f32 {
    // Reference value for float audio (full scale = 1.0)
    const REFERENCE_DBFS: f32 = 1.0;

    // Calibration offset to compensate for Hann window and ensure accurate dBFS reading.
    // This value was determined empirically by testing with a 0 dBFS sine wave.
    const CALIBRATION_OFFSET_DB: f32 = 0.0;

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
        20.0 * (scaled / REFERENCE_DBFS).log10() + CALIBRATION_OFFSET_DB
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

    pub fn get_fft(&self, samples: &[f32]) -> Result<Vec<(f64, f64)>> {
        // apply hann window for smoothing
        let hann_window = hann_window(samples);

        // calc spectrum with proper dBFS scaling
        let spectrum = samples_fft_to_spectrum(
            &hann_window,
            self.sample_rate,
            FrequencyLimit::Range(20., 20000.),
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

                // Apply pink noise compensation
                let compensation = PINK_NOISE_SLOPE * (freq / PINK_NOISE_REF_FREQ).log10();
                (freq, val + compensation)
            })
            .collect();

        // Convert to log scale for display
        let min_freq_log = 20_f64.log10();
        let max_freq_log = 20000_f64.log10();
        let log_range = max_freq_log - min_freq_log;
        let chart_width = 100.;

        let fft_vec = data
            .into_iter()
            .map(|(freq, val)| {
                let log_freq = freq.log10();
                let normalized_pos = (log_freq - min_freq_log) / log_range;
                let chart_x = normalized_pos * chart_width;

                (chart_x, val)
            })
            .collect();

        Ok(fft_vec)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Tests the FFT functionality with a simple sine wave
    fn test_get_fft() {
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

        let fft_result = analyzer.get_fft(&samples).unwrap();

        // Find max to verify calibration is reasonable
        let max_db = fft_result
            .iter()
            .map(|(_, db)| *db)
            .fold(f64::NEG_INFINITY, f64::max);

        println!(
            "440Hz sine wave (off-bin): Max dB = {max_db} (expected ~ -1 to -2 dB due to spectral leakage)"
        );

        // Should have some data points
        assert!(!fft_result.is_empty());
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

        let fft_result = analyzer.get_fft(&samples).unwrap();

        // Find the maximum value in the spectrum
        let max_db = fft_result
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

        let fft_1khz = analyzer.get_fft(&samples_1khz).unwrap();
        let max_1khz = fft_1khz
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

        let fft_125hz = analyzer.get_fft(&samples_125hz).unwrap();
        let max_125hz = fft_125hz
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
