use spectrum_analyzer::scaling::{divide_by_N, scale_20_times_log10};
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{FrequencyLimit, samples_fft_to_spectrum};

pub fn get_fft(samples: &[f32]) -> Vec<(f64, f64)> {
    let samples = samples.iter().step_by(2).cloned().collect::<Vec<f32>>();
    // apply hann window for smoothing; length must be a power of 2 for the FFT
    // 2048 is a good starting point with 44100 kHz
    let hann_window = hann_window(&samples);
    // calc spectrum
    let spectrum_hann_window = samples_fft_to_spectrum(
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

    spectrum_hann_window
        .data()
        .into_iter()
        // convert OrderaleF32 to f64
        .map(|(x, y)| (x.val() as f64, y.val() as f64))
        .collect()
}

pub fn transform_to_log_scale(
    fft_data: &[(f64, f64)],
    x_bounds: [f64; 2],
    freq_range: [f64; 2],
) -> Vec<(f64, f64)> {
    let min_freq_log = freq_range[0].log10();
    let max_freq_log = freq_range[1].log10();
    let log_range = max_freq_log - min_freq_log;
    let chart_width = x_bounds[1] - x_bounds[0];

    fft_data
        .iter()
        .map(|(freq, val)| {
            let log_freq = freq.log10();
            // Нормализуем позицию частоты в логарифмическом диапазоне (от 0.0 до 1.0)
            let normalized_pos = (log_freq - min_freq_log) / log_range;
            // Масштабируем нормализованную позицию до ширины чарта
            let chart_x = x_bounds[0] + normalized_pos * chart_width;

            // up 150 so bar chart looks better
            (chart_x, *val + 150.)
        })
        .collect()
}
