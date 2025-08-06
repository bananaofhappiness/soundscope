use spectrum_analyzer::scaling::{
    divide_by_N, divide_by_N_sqrt, scale_20_times_log10, scale_to_zero_to_one,
};
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{FrequencyLimit, samples_fft_to_spectrum};

/// Minimal example.
pub fn get_fft(samples: &[f32]) -> Vec<(f64, f64)> {
    // apply hann window for smoothing; length must be a power of 2 for the FFT
    // 2048 is a good starting point with 44100 kHz
    // let samples: &[f32] = &[0.0, 3.1, 2.7, -1.0, -2.0, -4.0, 7.0, 6.0];
    let hann_window = hann_window(&samples[0..1024]);
    // println!("{samples:?}");
    // calc spectrum
    let spectrum_hann_window = samples_fft_to_spectrum(
        // (windowed) samples
        &hann_window,
        // sampling rate
        44100,
        // optional frequency limit: e.g. only interested in frequencies 50 <= f <= 150?
        FrequencyLimit::All,
        // FrequencyLimit::Range(0., 20000.),
        // optional scale
        // Some(&scale_to_zero_to_one),
        // Some(&scale_20_times_log10),
        Some(&divide_by_N),
        // None,
    )
    .unwrap();

    let mut vec = Vec::new();
    for (fr, fr_val) in spectrum_hann_window.data().iter() {
        vec.push((fr.val() as f64, fr_val.val() as f64 * 20000.));
    }
    vec
}
