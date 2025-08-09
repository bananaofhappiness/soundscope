mod analyzer;
mod app;
mod audio_player;
mod tui;
use crate::audio_player::{AudioFile, AudioPlayer, PlayerCommand};
use color_eyre::Result;
use crossbeam::channel::{bounded, unbounded};
use std::{sync::Arc, thread};

fn main() -> Result<()> {
    color_eyre::install()?;
    // create a tui sender that sends signals when the file is stopped, selected etc.
    let (tui_tx, audio_player_rx) = bounded::<PlayerCommand>(1);

    //create a audio player sender that sends signals to the analyzer when its time to analyze
    let (audio_tx, analyzer_rx) = unbounded::<usize>();
    // create an audio file
    let audio_file = AudioFile::new(audio_tx)?;
    // clone a samples of an audio file so it can be use in analyzer
    let samples = audio_file.samples.clone();

    let mut player = AudioPlayer::from_file(audio_file)?;

    // thread::spawn(|| tui::run(tui_reader, tui_tx));
    thread::spawn(move || player.run(audio_player_rx));
    tui::run(tui_tx, analyzer_rx, samples)
}
