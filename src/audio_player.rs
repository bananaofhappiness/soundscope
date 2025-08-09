use color_eyre::Result;
use color_eyre::eyre::eyre;
use crossbeam::channel::{Receiver, Sender};
use rodio::Source;
// use color_eyre::eyre::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use symphonia::core::audio::{Channels, SampleBuffer};
// use color_eyre::eyre::Error;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

// Samples of the whole file
pub type Samples = Arc<RwLock<Vec<f32>>>;
// pub type Samples = Vec<f32>;
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
    pub samples: Samples,
    sample_rate: SampleRate,
    // channels of the file (mono, stereo, etc.)
    channels: Channels,
    // Global state and the sender of it
    playback_position: usize, // Index of the Samples vec
    audio_tx: Sender<usize>,
}

impl Iterator for AudioFile {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let pos = self.playback_position;
        let res = if pos < self.samples.read().unwrap().len() {
            Some(self.samples.read().unwrap()[pos])
        } else {
            None
        };
        if pos % 4096 == 0 {
            if let Err(err) = self.audio_tx.send(pos) {
                eyre!(err);
            }
            // if let Ok(_) = self.audio_tx.send(pos) {
            //     println!("{pos:}");
            // }
        }
        self.playback_position += 1;
        res
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
    pub fn new(audio_tx: Sender<usize>) -> Result<Self> {
        let af = AudioFile {
            file_path: None,
            samples: Arc::new(RwLock::new(Vec::new())),
            // samples: vec![0.; 0],
            sample_rate: 44100,
            channels: Channels::all(),
            playback_position: 0,
            audio_tx,
        };
        Ok(af)
    }

    pub fn load_file(&mut self, path: &str) -> Result<()> {
        let (samples, sample_rate, channels) = Self::decode_file(path)?;
        self.file_path = Some(path.to_string());
        let mut vec = self.samples.write().unwrap();
        *vec = samples;
        self.sample_rate = sample_rate;
        self.channels = channels;
        self.playback_position = 0;
        Ok(())
    }

    fn decode_file(path: &str) -> Result<(Vec<f32>, SampleRate, Channels)> {
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
        let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

        // Get the instantiated format reader.
        let mut format = probed.format;

        // Find the first audio track with a known (decodeable) codec.
        let track = match format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        {
            Some(track) => track,
            None => return Err(color_eyre::eyre::eyre!("No supported audio tracks found")),
        };

        // Use the default options for the decoder.
        let dec_opts: DecoderOptions = Default::default();

        // Create a decoder for the track.
        let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

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
                    return Ok((all_samples, sample_rate, channels));
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

#[derive(Debug, PartialEq)]
pub enum PlaybackState {
    Playing,
    Paused,
}

pub struct AudioPlayer {
    audio_file: AudioFile,
    _stream_handle: rodio::OutputStream,
    state: PlaybackState,
    sink: rodio::Sink,
}

impl AudioPlayer {
    pub fn from_file(audio_file: AudioFile) -> Result<Self> {
        let _stream_handle =
            rodio::OutputStreamBuilder::open_default_stream().expect("open default audio stream");
        let sink = rodio::Sink::connect_new(&_stream_handle.mixer());
        // sink.pause();
        Ok(Self {
            audio_file,
            _stream_handle,
            state: PlaybackState::Paused,
            sink,
        })
    }

    pub fn run(&mut self, audio_player_rx: Receiver<PlayerCommand>) -> Result<()> {
        loop {
            if let Ok(cmd) = audio_player_rx.try_recv() {
                match cmd {
                    PlayerCommand::SelectFile(path) => {
                        if let Err(err) = self.audio_file.load_file(&path) {
                            println!("Error loading file: {}", err);
                            continue;
                            // todo: show a ratatui paragraph with error message
                        }
                        self.audio_file.playback_position = 0;

                        // clear the sink and append new file
                        self.sink.stop();
                        self.sink.clear();
                        self.sink.append(self.audio_file.clone());

                        self.state = PlaybackState::Paused;
                    }

                    PlayerCommand::ChangeState => {
                        if self.state == PlaybackState::Playing {
                            self.sink.pause();
                            self.state = PlaybackState::Paused;
                        } else {
                            self.sink.play();
                            self.state = PlaybackState::Playing;
                        }
                    }
                    PlayerCommand::Stop => {
                        self.sink.stop();
                        self.sink.clear();
                        self.audio_file.playback_position = 0;
                        self.state = PlaybackState::Paused;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // Ok(())
    }
}
