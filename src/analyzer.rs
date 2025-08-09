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
        FrequencyLimit::All,
        // optional scale
        // Some(&scale_to_zero_to_one),
        Some(&scale_20_times_log10),
        // Some(&divide_by_N),
        // None,
    )
    .unwrap();

    let mut vec = Vec::new();
    for (fr, fr_val) in spectrum_hann_window.data().iter() {
        // move +120 up if chart is with graphtype::bar so it better
        vec.push((fr.val() as f64, fr_val.val() as f64));
    }
    vec
}
