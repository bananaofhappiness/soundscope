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
pub type Samples = Vec<f32>;
// pub type Samples = Vec<f32>;
pub type SampleRate = u32;
pub type PlaybackPosition = usize;

pub enum PlayerCommand {
    SelectFile(String),
    ChangeState,
    Stop,
}

// TODO: introduce streaming
// stream the first portion of the track while the whole track loading in the background
/// `AudioFile` represents a loaded audio file with its samples, sample rate, and channels.
/// It implements [`Source`] and [`Iterator`] for playback.
#[derive(Clone)]
pub struct AudioFile {
    pub samples: Samples,
    pub mid_samples: Samples,
    pub side_samples: Samples,
    pub sample_rate: SampleRate,
    pub duration: Duration,
    // channels of the file (mono, stereo, etc.)
    pub channels: Channels,
    // Global state and the sender of it
    playback_position: usize, // Index of the Samples vec
    playback_position_tx: Sender<usize>,
}

impl Iterator for AudioFile {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let pos = self.playback_position;
        let res = if pos < self.samples.len() {
            Some(self.samples[pos])
        } else {
            None
        };
        if pos % 4096 == 0 {
            if let Err(err) = self.playback_position_tx.send(pos) {
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
    pub fn new(playback_position_tx: Sender<usize>) -> Self {
        AudioFile {
            samples: Vec::new(),
            mid_samples: Vec::new(),
            side_samples: Vec::new(),
            sample_rate: 44100,
            duration: Duration::from_millis(0),
            channels: Channels::all(),
            playback_position: 0,
            playback_position_tx,
        }
    }

    fn from_file(path: &str, playback_position_tx: Sender<usize>) -> Result<Self> {
        let (samples, sample_rate, channels) = Self::decode_file(path)?;
        // TODO: other channels, not only stereo sound.
        let left_samples = samples.iter().step_by(2).cloned().collect::<Vec<f32>>();
        let right_samples = samples
            .iter()
            .skip(1)
            .step_by(2)
            .cloned()
            .collect::<Vec<f32>>();
        let mid_samples = left_samples
            .iter()
            .zip(right_samples.iter())
            .map(|(l, r)| (l + r) / 2.)
            .collect::<Vec<f32>>();
        let side_samples = left_samples
            .iter()
            .zip(right_samples.iter())
            .map(|(l, r)| (l - r) / 2.)
            .collect::<Vec<f32>>();

        let duration = mid_samples.len() as f64 / sample_rate as f64 * 1000.;
        Ok(AudioFile {
            samples,
            mid_samples,
            side_samples,
            sample_rate,
            duration: Duration::from_millis(duration as u64),
            channels,
            playback_position: 0,
            playback_position_tx,
        })
    }

    /// Decodes file and returns its [`Samples`], [`SampleRate`] and [`Channels`]
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
    // sends playback position
    playback_position_tx: Sender<usize>,
    audio_file: AudioFile,
    _stream_handle: rodio::OutputStream,
    state: PlaybackState,
    sink: rodio::Sink,
}

impl AudioPlayer {
    pub fn new(playback_position_tx: Sender<usize>) -> Result<Self> {
        let _stream_handle =
            rodio::OutputStreamBuilder::open_default_stream().expect("open default audio stream");
        let sink = rodio::Sink::connect_new(&_stream_handle.mixer());
        let audio_file = AudioFile::new(playback_position_tx.clone());
        // sink.pause();
        Ok(Self {
            playback_position_tx,
            audio_file,
            _stream_handle,
            state: PlaybackState::Paused,
            sink,
        })
    }

    pub fn run(
        &mut self,
        player_command_rx: Receiver<PlayerCommand>,
        audio_file_tx: Sender<AudioFile>,
    ) -> Result<()> {
        loop {
            if let Ok(cmd) = player_command_rx.try_recv() {
                match cmd {
                    PlayerCommand::SelectFile(path) => {
                        match AudioFile::from_file(&path, self.playback_position_tx.clone()) {
                            Err(err) => {
                                println!("Error loading file: {}", err);
                                continue;
                                // TODO: show a ratatui paragraph with error message
                            }
                            Ok(af) => {
                                self.audio_file = af.clone();
                                if let Err(err) = audio_file_tx.send(af) {
                                    // TODO: show a ratatui paragraph with error message
                                }
                            }
                        };
                        self.audio_file.playback_position = 0;
                        self.state = PlaybackState::Paused;

                        // clear the sink and append new file
                        self.sink.stop();
                        self.sink.clear();
                        self.sink.append(self.audio_file.clone());
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
