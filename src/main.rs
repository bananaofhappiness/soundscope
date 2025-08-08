mod app;
mod audio_player;
mod fft;
mod file_reader;
mod tui;
use crate::audio_player::{AudioFile, AudioPlayer, PlayerCommand};
use color_eyre::Result;
use crossbeam::channel::bounded;
use std::{
    sync::{Arc, Mutex, atomic::AtomicUsize},
    thread,
    time::Duration,
};

fn main() -> Result<()> {
    color_eyre::install()?;
    // create a tui sender that sends signals when the file is stopped, selected etc.
    let (tui_tx, audio_player_rx) = bounded::<PlayerCommand>(1);

    //create a audio player sender that sends signals to the analyzer when its time to analyze
    let (audio_tx, analyzer_rx) = bounded::<usize>(1);
    // create an audio file
    let audio_file = AudioFile::new(audio_tx)?;
    // copy an audio_file to analyzer so it can use its samples to analyze
    let analyzer_audio_fyle = audio_file.clone();

    let mut player = AudioPlayer::from_file(audio_file)?;

    // thread::spawn(|| tui::run(tui_reader, tui_tx));
    thread::spawn(move || player.run(audio_player_rx));
    // thread::spawn(analyzer.run());
    tui::run(tui_tx)
}
