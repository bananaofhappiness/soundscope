mod analyzer;
mod audio_player;
mod tui;
use crate::audio_player::{AudioFile, AudioPlayer, PlaybackPosition, PlayerCommand};
use color_eyre::Result;
use crossbeam::channel::{bounded, unbounded};
use std::{sync::Arc, thread};

fn main() -> Result<()> {
    color_eyre::install()?;
    // create a tui sender that sends signals when the file is stopped, selected etc.
    let (player_command_tx, player_command_rx) = bounded::<PlayerCommand>(1);

    // create an audio player sender that sends position to the analyzer so it knows what samples to use
    let (playback_position_tx, playback_position_rx) = unbounded::<PlaybackPosition>();

    // create an audio_file sender to send audio file from player to the tui app
    let (audio_file_tx, audio_file_rx) = bounded::<AudioFile>(1);

    // create an audio player
    let mut player = AudioPlayer::new(playback_position_tx.clone())?;

    // just a place holder audio_file to initialize app
    let audio_file = AudioFile::new(playback_position_tx);

    // thread::spawn(|| tui::run(tui_reader, tui_tx));
    thread::spawn(move || player.run(player_command_rx, audio_file_tx));
    tui::run(
        audio_file,
        player_command_tx,
        audio_file_rx,
        playback_position_rx,
    )
}
