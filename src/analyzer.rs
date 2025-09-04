use color_eyre::Result;
use ebur128::{EbuR128, Mode};
use spectrum_analyzer::scaling::scale_20_times_log10;
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{FrequencyLimit, samples_fft_to_spectrum};

pub struct Analyzer {
    loudness_meter: EbuR128,
}

impl Default for Analyzer {
    fn default() -> Self {
        let loudness_meter = match EbuR128::new(2, 44100, Mode::all()) {
            Ok(loudness_meter) => loudness_meter,
            Err(err) => panic!("Failed to create loudness meter: {}", err),
        };
        Self { loudness_meter }
    }
}

impl Analyzer {
    /// used when new file selected
    pub fn select_new_file(&mut self, channels: u32, rate: u32) -> Result<()> {
        self.loudness_meter = EbuR128::new(channels, rate, Mode::all())?;
        Ok(())
    }

    pub fn get_fft(&mut self, samples: &[f32], sample_rate: usize) -> Vec<(f64, f64)> {
        // apply hann window for smoothing
        let hann_window = hann_window(samples);

        // calc spectrum
        let spectrum = samples_fft_to_spectrum(
            &hann_window,
            sample_rate as u32,
            FrequencyLimit::Range(20.0, 20000.0),
            Some(&scale_20_times_log10),
        )
        .unwrap();

        // convert OrderaleF32 to f64
        let fft_vec = spectrum
            .data()
            .iter()
            .map(|(x, y)| (x.val() as f64, y.val() as f64))
            .collect::<Vec<(f64, f64)>>();
        // transform to log scale
        Self::transform_to_log_scale(&fft_vec)
    }

    pub fn transform_to_log_scale(fft_data: &[(f64, f64)]) -> Vec<(f64, f64)> {
        // set frequency range
        let min_freq_log = 20_f64.log10();
        let max_freq_log = 20000_f64.log10();
        let log_range = max_freq_log - min_freq_log;

        // set chart width to 100 (from 0 to 100)
        let chart_width = 100.;
        fft_data
            .iter()
            .map(|(freq, val)| {
                let log_freq = freq.log10();
                // normalize frequency to range [0.0, 1.0]
                let normalized_pos = (log_freq - min_freq_log) / log_range;
                // Scale normalized position to chart width
                let chart_x = normalized_pos * chart_width;

                (chart_x, *val)
            })
            .collect()
    }

    pub fn get_waveform(samples: &[f32], sample_rate: usize) -> Vec<(f64, f64)> {
        let samples_in_one_ms = sample_rate / 1000;
        let iter = samples.iter().step_by(samples_in_one_ms).map(|x| *x as f64);
        (0..15 * 1000)
            .map(|x| x as f64)
            .zip(iter)
            .collect::<Vec<(f64, f64)>>()
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Checks if the transformation to log scale works correctly and frequencies are in a given range
    fn test_transform_to_log_scale() {
        let input = vec![(20.0, -10.0), (100.0, -5.0), (1000.0, 0.0), (20000.0, 5.0)];

        let result = Analyzer::transform_to_log_scale(&input);

        assert!((result[0].0 - 0.0).abs() < 1e-6); // 20Hz → 0
        assert!((result[3].0 - 100.0).abs() < 1e-6); // 20kHz → 100
    }
    #[test]
    /// Tests the FFT functionality with a simple sine wave
    fn test_get_fft() {
        let mut analyzer = Analyzer::default();

        // Generate a simple sine wave at 440Hz
        let sample_rate = 44100;
        let frequency = 440.0;
        // 1 sec of samples
        let samples: Vec<f32> = (0..16384 as usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * frequency * t).sin()
            })
            .collect();

        let fft_result = analyzer.get_fft(&samples, sample_rate);

        // Should have some data points
        assert!(!fft_result.is_empty());
    }

    #[test]
    /// Tests the waveform generation
    fn test_get_waveform() {
        let samples: Vec<f32> = (0..44100).map(|i| (i as f32 / 44100.0).sin()).collect();

        let waveform = Analyzer::get_waveform(&samples, 44100);

        // Should have data points
        assert!(!waveform.is_empty());

        // Check that x values are sequential
        for i in 1..waveform.len().min(100) {
            assert!(waveform[i].0 > waveform[i - 1].0);
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
        let result = analyzer.select_new_file(1, 48000); // mono, 48kHz
        assert!(result.is_ok());

        let result = analyzer.select_new_file(6, 96000); // 5.1 surround, 96kHz
        assert!(result.is_ok());
    }

    #[test]
    /// Tests edge cases for transform_to_log_scale
    fn test_transform_to_log_scale_edge_cases() {
        // Empty input
        let empty_input: Vec<(f64, f64)> = vec![];
        let result = Analyzer::transform_to_log_scale(&empty_input);
        assert!(result.is_empty());

        // Single frequency
        let single_input = vec![(1000.0, -3.0)];
        let result = Analyzer::transform_to_log_scale(&single_input);
        assert_eq!(result.len(), 1);
        assert!(result[0].0 >= 0.0 && result[0].0 <= 100.0);
        assert_eq!(result[0].1, -3.0);
    }
}
