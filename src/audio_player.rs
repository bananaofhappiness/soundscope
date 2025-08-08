use color_eyre::Result;
use crossbeam::channel::Receiver;
use rodio::Source;
// use color_eyre::eyre::Error;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use symphonia::core::audio::{Channels, SampleBuffer};
// use color_eyre::eyre::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

// Samples of the whole file
pub type Samples = Arc<Vec<f32>>;
pub type SampleRate = u32;

pub enum PlayerCommand {
    SelectFile(String),
    ChangeState,
    Stop,
}

// TODO: introduce streaming
// stream the first portion of the track while the whole track loading in the background
#[derive(Clone)]
pub struct AudioFile {
    file_path: Option<String>,
    samples: Samples,
    sample_rate: u32,
    channels: Channels,
    // duration: Duration,

    // Global state
    playback_position: Arc<AtomicUsize>, // Index of the Samples vec
                                         // is_playing: Arc<AtomicBool>,
}

impl Iterator for AudioFile {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        // let mut playback_position = self.playback_position;
        // if playback_position < self.samples.len() {
        //     let sample = self.samples[playback_position];
        //     playback_position += 1;
        //     Some(sample)
        // } else {
        //     None
        // }
        let pos = self.playback_position.fetch_add(1, Ordering::SeqCst);
        if pos < self.samples.len() {
            Some(self.samples[pos])
        } else {
            None
        }
    }
}

impl Source for AudioFile {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.channels.count() as u16
    }

    fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl AudioFile {
    pub fn new() -> Result<Self> {
        let af = AudioFile {
            file_path: None,
            samples: Arc::new(vec![0.; 0]),
            sample_rate: 44100,
            channels: Channels::all(),
            // duration: Duration::from_secs(1),
            playback_position: Arc::new(AtomicUsize::new(0)),
            // is_playing: Arc::new(AtomicBool::new(true)),
        };
        Ok(af)
    }

    pub fn load_file(&mut self, path: &str) -> Result<()> {
        let (samples, sample_rate, channels) = Self::decode_file(path)?;
        self.file_path = Some(path.to_string());
        self.samples = samples;
        self.sample_rate = sample_rate;
        self.channels = channels;
        Ok(())
    }

    fn decode_file(path: &str) -> Result<(Samples, SampleRate, Channels)> {
        // Open the media source.
        let src = std::fs::File::open(path).expect("failed to open media");
        // Create the media source stream.
        let mss = MediaSourceStream::new(Box::new(src), Default::default());

        // Create a probe hint using the file's extension.
        let mut hint = Hint::new();
        hint.with_extension("mp3");

        // Use the default options for metadata and format readers.
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        // Probe the media source.
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .expect("unsupported format");

        // Get the instantiated format reader.
        let mut format = probed.format;

        // Find the first audio track with a known (decodeable) codec.
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .expect("no supported audio tracks");

        // Use the default options for the decoder.
        let dec_opts: DecoderOptions = Default::default();

        // Create a decoder for the track.
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &dec_opts)
            .expect("unsupported codec");

        // Store the track identifier, it will be used to filter packets.
        let track_id = track.id;

        // Make a sample buffer to hold the decoded audio samples.
        let mut all_samples = Vec::<f32>::new();
        let mut sample_buf = None;

        // Defaults for sample rate and channels
        let mut sample_rate = 44100;
        let mut channels = Channels::empty();

        // The decode loop.
        loop {
            // Get the next packet from the format reader.
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(Error::IoError(_)) => {
                    // End of stream - return Ok to indicate successful completion
                    return Ok((Arc::new(all_samples), sample_rate, channels));
                }
                Err(err) => {
                    return Err(err.into());
                    // return Err(err.into());
                }
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
                        sample_rate = spec.rate;
                        channels = spec.channels;

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
                Err(symphonia::core::errors::Error::DecodeError(_)) => (),
                Err(err) => {
                    return Err(err.into());
                }
            }
        }
    }
}

#[derive(PartialEq)]
pub enum PlaybackState {
    Playing,
    Paused,
}

pub struct AudioPlayer {
    audio_file: Arc<Mutex<AudioFile>>,
    stream_handle: rodio::OutputStream,
    state: Arc<Mutex<PlaybackState>>,
    // _stream: rodio::OutputStream,
    sink: rodio::Sink,
}

impl AudioPlayer {
    pub fn from_file(audio_file: Arc<Mutex<AudioFile>>) -> Result<Self> {
        let stream_handle =
            rodio::OutputStreamBuilder::open_default_stream().expect("open default audio stream");
        let sink = rodio::Sink::connect_new(&stream_handle.mixer());
        // sink.pause();
        Ok(Self {
            audio_file,
            stream_handle,
            state: Arc::new(Mutex::new(PlaybackState::Paused)),
            // _stream,
            sink,
        })
    }

    // fn play_audio(&self) {
    //     // Get an output stream handle to the default physical sound device.
    //     // Note that the playback stops when the stream_handle is dropped.//!
    //     let stream_handle =
    //         rodio::OutputStreamBuilder::open_default_stream().expect("open default audio stream");
    //     let sink = rodio::Sink::connect_new(&stream_handle.mixer());
    //     // Decode that sound file into a source
    //     self.is_playing.store(true, Ordering::Release);
    //     let source = self.clone();
    //     sink.append(source);
    //     sink.sleep_until_end();
    // }

    // fn pause_audio() {
    //     todo!()
    // }

    // pub fn get_file(self) -> Arc<Mutex<AudioFile>> {
    //     self.audio_file
    // }

    pub fn run(&self, audio_player_rx: Receiver<PlayerCommand>) -> Result<()> {
        loop {
            if let Ok(cmd) = audio_player_rx.try_recv() {
                match cmd {
                    PlayerCommand::SelectFile(path) => {
                        let mut audio_file = self.audio_file.lock().unwrap();
                        if let Err(err) = audio_file.load_file(&path) {
                            println!("Error loading file: {}", err);
                            continue;
                            // todo: show a ratatui paragraph with error message
                        }
                        // playback position <- 0
                        audio_file.playback_position.store(0, Ordering::SeqCst);

                        // clear the sink and append new file
                        self.sink.stop();
                        self.sink.clear();
                        self.sink.append(audio_file.clone());

                        *self.state.lock().unwrap() = PlaybackState::Paused;
                    }

                    PlayerCommand::ChangeState => {
                        if *self.state.lock().unwrap() == PlaybackState::Playing {
                            self.sink.pause();
                            *self.state.lock().unwrap() = PlaybackState::Paused;
                        } else {
                            self.sink.play();
                            *self.state.lock().unwrap() = PlaybackState::Playing;
                        }
                    }
                    PlayerCommand::Stop => {
                        self.sink.stop();
                        self.sink.clear();
                        self.audio_file
                            .lock()
                            .unwrap()
                            .playback_position
                            .store(0, Ordering::SeqCst);
                        *self.state.lock().unwrap() = PlaybackState::Paused;
                    }
                }
            }
        }
        // Ok(())
    }
}
