//! This module contains the implementation of the audio player used to play audio files in user's terminal.
//! under the hood it uses `rodio` for playback and `symphonia` for decoding.
use crossbeam::channel::{Receiver, Sender};
use eyre::{Result, eyre};
use rodio::{ChannelCount, OutputStream, OutputStreamBuilder, Sink, Source, source};
use std::{path::PathBuf, time::Duration};
use symphonia::core::{
    audio::{Channels, SampleBuffer},
    codecs::{CODEC_TYPE_NULL, DecoderOptions},
    errors::Error,
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};

// Samples of the whole file
pub type Samples = Vec<f32>;
// pub type Samples = Vec<f32>;
pub type SampleRate = u32;
pub type PlaybackPosition = usize;

pub enum PlayerCommand {
    SelectFile(PathBuf),
    ChangeState,
    // Had to add Quit because on MacOS tui can't be on the main thread (smth does not implement Send), player must be there.
    // So when tui quits, player must know tui has quit and quits too.
    Quit,
    /// Move the playhead right
    MoveRight,
    /// Move the playhead left
    MoveLeft,
    /// Shows an error (only in debug mode)
    #[cfg(debug_assertions)]
    ShowTestError,
}

/// `AudioFile` represents a loaded audio file with its samples, sample rate, and channels.
/// It implements [`Source`] and [`Iterator`] for playback.
#[derive(Clone)]
pub struct AudioFile {
    title: String,
    samples: Samples,
    mid_samples: Samples,
    side_samples: Samples,
    sample_rate: SampleRate,
    duration: Duration,
    // channels of the file (mono, stereo, etc.)
    channels: Channels,
    // Global state and the sender of it
    playback_position: usize, // Index of the Samples vec
    playback_position_tx: Sender<usize>,
}

impl AudioFile {
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn samples(&self) -> &Samples {
        &self.samples
    }

    pub fn mid_samples(&self) -> &Samples {
        &self.mid_samples
    }

    pub fn side_samples(&self) -> &Samples {
        &self.side_samples
    }

    pub fn duration(&self) -> &Duration {
        &self.duration
    }
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
        if pos.is_multiple_of(4096)
            && let Err(_err) = self.playback_position_tx.send(pos)
        {
            // TODO: log sending error
        }
        self.playback_position += 1;
        res
    }
}

impl Source for AudioFile {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        self.channels.count() as u16
    }

    fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.duration)
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), source::SeekError> {
        // TODO: other channels, see https://docs.rs/rodio/latest/src/rodio/buffer.rs.html#88-105
        let curr_channel = self.playback_position % self.channels() as usize;
        let new_pos = pos.as_secs_f32() * self.sample_rate() as f32 * self.channels() as f32;
        // saturate pos at the end of the source
        let new_pos = new_pos as usize;
        let new_pos = new_pos.min(self.samples.len());
        // make sure the next sample is for the right channel
        let new_pos = new_pos.next_multiple_of(self.channels() as usize);
        let new_pos = new_pos - curr_channel;

        self.playback_position = new_pos;
        // send position again so the charts update even when the audio is paused.
        if let Err(_err) = self.playback_position_tx.send(new_pos) {
            // TODO: log sending error
        }
        Ok(())
    }
}

impl AudioFile {
    pub fn new(playback_position_tx: Sender<usize>) -> Self {
        AudioFile {
            title: "".to_string(),
            samples: Vec::new(),
            mid_samples: Vec::new(),
            side_samples: Vec::new(),
            sample_rate: 44100,
            duration: Duration::from_secs(15),
            channels: Channels::all(),
            playback_position: 0,
            playback_position_tx,
        }
    }

    /// creates a new `AudioFile` from file
    fn from_file(path: &PathBuf, playback_position_tx: Sender<usize>) -> Result<Self> {
        // get file name
        let title = path.file_name().unwrap().to_string_lossy().to_string();
        let (samples, sample_rate, channels) = Self::decode_file(path)?;
        // TODO: other channels, not only stereo sound.
        let (mid_samples, side_samples) = get_mid_and_side_samples(&samples);
        let duration = mid_samples.len() as f64 / sample_rate as f64 * 1000.;
        Ok(AudioFile {
            title,
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
    fn decode_file(path: &PathBuf) -> Result<(Samples, SampleRate, Channels)> {
        // open the media source and create a stream
        let src = std::fs::File::open(path)?;
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
            None => {
                return Err(eyre!("No audio track found with a decodeable codec"));
            }
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

pub struct AudioPlayer {
    // sends playback position
    playback_position_tx: Sender<usize>,
    audio_file: AudioFile,
    _stream_handle: OutputStream,
    sink: Sink,
}

impl AudioPlayer {
    pub fn new(playback_position_tx: Sender<usize>) -> Result<Self> {
        let _stream_handle = OutputStreamBuilder::open_default_stream()?;
        let sink = Sink::connect_new(_stream_handle.mixer());
        let audio_file = AudioFile::new(playback_position_tx.clone());
        Ok(Self {
            playback_position_tx,
            audio_file,
            _stream_handle,
            sink,
        })
    }

    /// Runs `audio_player`
    pub fn run(
        &mut self,
        player_command_rx: Receiver<PlayerCommand>,
        audio_file_tx: Sender<AudioFile>,
        error_tx: Sender<String>,
    ) -> Result<()> {
        loop {
            // recieve a `PlayerCommand` from an UI
            if let Ok(cmd) = player_command_rx.try_recv() {
                match cmd {
                    PlayerCommand::SelectFile(path) => {
                        match AudioFile::from_file(&path, self.playback_position_tx.clone()) {
                            Err(err) => {
                                if let Err(_err) =
                                    error_tx.send(format!("Error loading file: {}", err))
                                {
                                    //TODO: log a sending error
                                }
                                continue;
                            }
                            Ok(af) => {
                                self.audio_file = af.clone();
                                if let Err(_err) = audio_file_tx.send(af) {
                                    //TODO: log a sending error
                                }
                            }
                        };

                        // clear the sink and append new file
                        self.sink.stop();
                        self.sink.clear();
                        self.audio_file.playback_position = 0;
                        self.sink.append(self.audio_file.clone());
                        if let Err(_err) = self.playback_position_tx.send(0) {
                            // TODO: log a sending error
                        }
                    }

                    PlayerCommand::ChangeState => {
                        if self.sink.is_paused() {
                            self.sink.play();
                        } else {
                            self.sink.pause();
                        }
                        // if we hit the end of the track, then load it again
                        if self.sink.empty() {
                            self.audio_file.playback_position = 0;
                            self.sink.append(self.audio_file.clone());
                        }
                    }
                    PlayerCommand::Quit => {
                        self.sink.stop();
                        self.sink.clear();
                        self.audio_file.playback_position = 0;
                        ratatui::crossterm::execute!(
                            std::io::stdout(),
                            ratatui::crossterm::event::DisableMouseCapture
                        )?;
                        return Ok(());
                    }
                    // move the playhead right
                    PlayerCommand::MoveRight => {
                        let pos = self.sink.get_pos();
                        if self.sink.empty() {
                            continue;
                        }
                        let seek = (pos + Duration::from_secs(5)).min(self.audio_file.duration);

                        if let Err(err) = self.sink.try_seek(seek) {
                            println!("Error seeking: {:?}", err);
                            // TODO: error handling
                        }
                    }
                    // move the playhead left
                    PlayerCommand::MoveLeft => {
                        if self.sink.empty() {
                            let pos = self.audio_file.duration - Duration::from_secs(5);
                            self.sink.append(self.audio_file.clone());
                            if let Err(err) = self.sink.try_seek(pos) {
                                println!("Error seeking: {:?}", err);
                                // TODO: error handling
                            }
                            continue;
                        }
                        let pos = self.sink.get_pos();
                        if let Err(_err) = self
                            .sink
                            .try_seek(pos.saturating_sub(Duration::from_secs(5)))
                        {
                            // TODO: error handling
                        }
                    }
                    #[cfg(debug_assertions)]
                    PlayerCommand::ShowTestError => {
                        error_tx.send("This is a test message".to_string()).unwrap()
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // Ok(())
    }
}

pub fn get_mid_and_side_samples(samples: &[f32]) -> (Vec<f32>, Vec<f32>) {
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
    (mid_samples, side_samples)
}
