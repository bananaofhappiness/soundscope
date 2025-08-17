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
        Self {
            loudness_meter: EbuR128::new(2, 44100, Mode::all()).unwrap(),
        }
    }
}

impl Analyzer {
    /// used when new file selected
    pub fn new(&mut self, channels: u32, rate: u32) -> Result<()> {
        self.loudness_meter = EbuR128::new(channels, rate, Mode::all())?;
        Ok(())
    }

    pub fn get_fft(samples: &[f32]) -> Vec<(f64, f64)> {
        // apply hann window for smoothing
        let hann_window = hann_window(&samples);

        // calc spectrum
        let spectrum = samples_fft_to_spectrum(
            &hann_window,
            // sampling rate
            44100,
            FrequencyLimit::Range(20.0, 20000.0),
            // optional scale
            // Some(&scale_to_zero_to_one),
            Some(&scale_20_times_log10),
            // Some(&divide_by_N),
            // None,
        )
        .unwrap();

        // convert OrderaleF32 to f64
        let fft_vec = spectrum
            .data()
            .into_iter()
            .map(|(x, y)| (x.val() as f64, y.val() as f64))
            .collect::<Vec<(f64, f64)>>();

        // transform to log scale
        let fft_vec = Self::transform_to_log_scale(&fft_vec);
        fft_vec
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

                // up 150 so bar chart looks better
                (chart_x, *val + 150.)
            })
            .collect()
    }

    pub fn get_waveform(samples: &[f32]) -> Vec<(f64, f64)> {
        let iter = samples.iter().step_by(44).map(|x| *x as f64);
        (0..15 * 1000)
            .map(|x| x as f64)
            .zip(iter)
            .collect::<Vec<(f64, f64)>>()
    }

    pub fn add_samples(&mut self, samples: &[f32]) {
        self.loudness_meter.add_frames_f32(samples);
    }

    pub fn get_shortterm_lufs(&mut self) -> Result<f64, ebur128::Error> {
        self.loudness_meter.loudness_shortterm()
    }

    pub fn get_integrated_lufs(&mut self) -> Result<f64, ebur128::Error> {
        self.loudness_meter.loudness_global()
    }

    pub fn get_true_peak(&mut self) -> Result<(f64, f64), ebur128::Error> {
        let tp_left = self.loudness_meter.true_peak(0)?;
        let tp_right = self.loudness_meter.true_peak(1)?;

        Ok((tp_left, tp_right))
    }
}
