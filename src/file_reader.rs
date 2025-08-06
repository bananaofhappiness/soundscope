use std::fs::File;

use rodio::Decoder;
use symphonia::core::audio::SampleBuffer;

use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::fft;

pub fn read_file(path: &str) -> Vec<(f64, f64)> {
    // Open the media source.
    let src = std::fs::File::open(path).expect("failed to open media");
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    // Create a probe hint using the file's extension. [Optional]
    let mut hint = Hint::new();
    hint.with_extension("mp3");

    // Use the default options when reading and decoding.
    let format_opts: FormatOptions = Default::default();
    let metadata_opts: MetadataOptions = Default::default();
    let decoder_opts: DecoderOptions = Default::default();

    // Get format
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .unwrap();
    let mut format = probed.format;

    // Get the default track.
    let track = format.default_track().unwrap();

    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &decoder_opts)
        .unwrap();

    // Store the track identifier, we'll use it to filter packets.
    let track_id = track.id;

    let sample_count = 0;
    let mut sample_buf = None;

    let mut x = 0;
    loop {
        // Get the next packet from the format reader.
        let packet = format.next_packet().unwrap();

        // If the packet does not belong to the selected track, skip it.
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet into audio samples, ignoring any decode errors.
        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                // The decoded audio samples may now be accessed via the audio buffer if per-channel
                // slices of samples in their native decoded format is desired. Use-cases where
                // the samples need to be accessed in an interleaved order or converted into
                // another sample format, or a byte buffer is required, are covered by copying the
                // audio buffer into a sample buffer or raw sample buffer, respectively. In the
                // example below, we will copy the audio buffer into a sample buffer in an
                // interleaved order while also converting to a f32 sample format.

                // If this is the *first* decoded packet, create a sample buffer matching the
                // decoded audio buffer format.
                if sample_buf.is_none() {
                    // Get the audio buffer specification.
                    let spec = *audio_buf.spec();
                    let output = format!(
                        "Sample rate: {}\nChannels: {}\n",
                        spec.rate,
                        spec.channels.count()
                    );
                    // std::fs::write("audio_info.txt", output)
                    //     .expect("Failed to write audio info to file");
                    // Get the capacity of the decoded buffer. Note: This is capacity, not length!
                    let duration = audio_buf.capacity() as u64;

                    // Create the f32 sample buffer.
                    sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                }

                // Copy the decoded audio buffer into the sample buffer in an interleaved format.
                if let Some(buf) = &mut sample_buf {
                    if x < 10 {
                        x += 1;
                        continue;
                    }
                    buf.copy_interleaved_ref(audio_buf);
                    // let samples: Vec<f32> = (0..2048)
                    //     .map(|i| {
                    //         let t = i as f32 / 44100 as f32;
                    //         (2.0 * std::f32::consts::PI * 10000.0 * t).sin()
                    //     })
                    //     .collect();

                    // The samples may now be access via the `samples()` function.
                    let samples = buf.samples();
                    let mono_samples: Vec<f32> =
                        // Берите только левый канал (каждый второй элемент)
                        samples.iter().step_by(2).cloned().collect();
                    // println!("{}", samples.len());
                    let v = fft::get_fft(&mono_samples);
                    // std::fs::write("output.txt", format!("{:?}", v))
                    //     .expect("Failed to write to file");
                    return v;
                }
            }
            Err(Error::DecodeError(_)) => (),
            Err(_) => break,
        }
    }
    vec![(0., 0.)]
}

pub fn play_audio(path: &str) {
    // Get an output stream handle to the default physical sound device.
    // Note that the playback stops when the stream_handle is dropped.//!
    let stream_handle =
        rodio::OutputStreamBuilder::open_default_stream().expect("open default audio stream");
    let sink = rodio::Sink::connect_new(&stream_handle.mixer());
    // Load a sound from a file, using a path relative to Cargo.toml
    let file = File::open(path).unwrap();
    // Decode that sound file into a source
    let source = Decoder::try_from(file).unwrap();
    sink.append(source);
    sink.sleep_until_end();
}

pub fn read_file_complete(path: &str) -> Vec<(f64, f64)> {
    // Open the media source.
    let src = std::fs::File::open(path).expect("failed to open media");
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    // Create a probe hint using the file's extension. [Optional]
    let mut hint = Hint::new();
    hint.with_extension("mp3");

    // Use the default options when reading and decoding.
    let format_opts: FormatOptions = Default::default();
    let metadata_opts: MetadataOptions = Default::default();
    let decoder_opts: DecoderOptions = Default::default();

    // Get format
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .unwrap();
    let mut format = probed.format;

    // Get the default track.
    let track = format.default_track().unwrap();

    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &decoder_opts)
        .unwrap();

    // Store the track identifier, we'll use it to filter packets.
    let track_id = track.id;

    let mut all_samples = Vec::<f32>::new();
    let mut sample_buf = None;

    loop {
        // Get the next packet from the format reader.
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(_) => break,
        };

        // If the packet does not belong to the selected track, skip it.
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet into audio samples, ignoring any decode errors.
        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                // If this is the *first* decoded packet, create a sample buffer matching the
                // decoded audio buffer format.
                if sample_buf.is_none() {
                    // Get the audio buffer specification.
                    let spec = *audio_buf.spec();

                    // Get the capacity of the decoded buffer. Note: This is capacity, not length!
                    let duration = audio_buf.capacity() as u64;

                    // Create the f32 sample buffer.
                    sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                }

                // Copy the decoded audio buffer into the sample buffer in an interleaved format.
                if let Some(buf) = &mut sample_buf {
                    buf.copy_interleaved_ref(audio_buf);

                    // Append the samples to our complete buffer
                    let samples = buf.samples();
                    all_samples.extend_from_slice(samples);
                }
            }
            Err(Error::DecodeError(_)) => (),
            Err(_) => break,
        }
    }

    // Process all samples at once
    // println!("{:?}", all_samples);
    fft::get_fft(&all_samples[0..2048])
}
