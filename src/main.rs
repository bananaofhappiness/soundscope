mod app;
mod audio_player;
mod fft;
mod file_reader;
mod tui;
use crate::audio_player::{AudioFile, AudioReader, PlayerCommand};
use color_eyre::Result;
use crossbeam::channel::bounded;
use std::{
    sync::{Arc, Mutex},
    thread,
};

fn main() -> Result<()> {
    color_eyre::install()?;
    // create an audio file and its readers
    let audio_file = Arc::new(Mutex::new(AudioFile::new()?));
    let tui_reader = AudioReader::from_file(Arc::clone(&audio_file))?;
    let player_reader = AudioReader::from_file(Arc::clone(&audio_file))?;
    let analyzer_reader = AudioReader::from_file(Arc::clone(&audio_file))?;

    // create a tui sender that sends signals when the file is stopped, selected etc.
    let (tui_tx, rx) = bounded::<PlayerCommand>(3);
    let audio_player_rx = rx.clone();
    thread::spawn(|| tui::run(tui_reader, tui_tx));
    thread::spawn(|| audio_player::run(player_reader, audio_player_rx));
    Ok(())
}
