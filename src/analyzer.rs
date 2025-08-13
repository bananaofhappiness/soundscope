use spectrum_analyzer::scaling::{divide_by_N, scale_20_times_log10};
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{FrequencyLimit, samples_fft_to_spectrum};

pub fn get_fft(samples: &[f32]) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
    let left_samples = samples.iter().step_by(2).cloned().collect::<Vec<f32>>();
    let right_samples = samples
        .iter()
        .skip(1)
        .step_by(2)
        .cloned()
        .collect::<Vec<f32>>();
    let mid = left_samples
        .iter()
        .zip(right_samples.iter())
        .map(|(l, r)| (l + r) / 2.)
        .collect::<Vec<f32>>();
    let side = left_samples
        .iter()
        .zip(right_samples.iter())
        .map(|(l, r)| (l - r) / 2.)
        .collect::<Vec<f32>>();
    // apply hann window for smoothing
    let mid_hann_window = hann_window(&mid);
    let side_hann_window = hann_window(&side);

    // calc spectrum
    let mid_spectrum = samples_fft_to_spectrum(
        &mid_hann_window,
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
    let side_spectrum = samples_fft_to_spectrum(
        &side_hann_window,
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
    let mid_fft_vec = mid_spectrum
        .data()
        .into_iter()
        .map(|(x, y)| (x.val() as f64, y.val() as f64))
        .collect::<Vec<(f64, f64)>>();
    let side_fft_vec = side_spectrum
        .data()
        .into_iter()
        .map(|(x, y)| (x.val() as f64, y.val() as f64))
        .collect::<Vec<(f64, f64)>>();

    // transform to log scale
    let mid_fft_vec = transform_to_log_scale(&mid_fft_vec);
    let side_fft_vec = transform_to_log_scale(&side_fft_vec);
    (mid_fft_vec, side_fft_vec)
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
