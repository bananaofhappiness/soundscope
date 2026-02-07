mod analyzer;
mod audio_capture;
mod audio_player;
mod tui;
use crate::audio_player::{AudioFile, AudioPlayer, PlaybackPosition, PlayerCommand};
use crossbeam::channel::{bounded, unbounded};
use eyre::Result;
use ringbuffer::{AllocRingBuffer, RingBuffer};
use std::{
    env,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    // Handle help flag
    if args.len() > 1 && (args[1] == "-h" || args[1] == "--help") {
        print_help();
        return Ok(());
    }

    // Handle version flag
    if args.len() > 1 && (args[1] == "-v" || args[1] == "--version") {
        println!("soundscope {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    suppress_alsa_messages();
    // create a tui sender that sends signals when the file is stopped, selected etc.
    let (player_command_tx, player_command_rx) = bounded::<PlayerCommand>(1);

    // create an audio player sender that sends position to the analyzer so it knows what samples to use
    let (playback_position_tx, playback_position_rx) = unbounded::<PlaybackPosition>();

    // create an audio_file sender to send audio file from player to the tui app
    let (audio_file_tx, audio_file_rx) = bounded::<AudioFile>(1);

    // create an error sender to send errors from player to the tui app
    let (error_tx, error_rx) = bounded::<String>(1);

    // create an audio player
    let mut player = AudioPlayer::new(playback_position_tx.clone())?;

    // just a place holder audio_file to initialize app
    let audio_file = AudioFile::new(playback_position_tx);

    let mut startup_file = args.get(1).map(PathBuf::from);
    if let Some(f) = startup_file {
        let current_working_dir = env::current_dir()?;
        startup_file = Some(f.canonicalize()?);
        println!("{:?}", f);
        env::set_current_dir(
            f.parent()
                .filter(|&s| s.to_str().unwrap() != "")
                .unwrap_or(&current_working_dir),
        )?;
    };

    let mut buf = AllocRingBuffer::new(44100usize * 30);
    buf.fill(0.0);
    let latest_captured_samples = Arc::new(Mutex::new(buf));

    thread::spawn(|| {
        tui::run(
            audio_file,
            player_command_tx,
            audio_file_rx,
            playback_position_rx,
            error_rx,
            latest_captured_samples,
            startup_file,
        )
    });
    player.run(player_command_rx, audio_file_tx, error_tx)
}

fn print_help() {
    println!("Usage: soundscope [OPTIONS] [FILE]");
    println!();
    println!("Arguments:");
    println!("  [FILE]  Audio file to open on startup");
    println!();
    println!("Options:");
    println!("  -h, --help     Print help");
    println!("  -v, --version  Print version");
}

// The code below suppresses ALSA error messages
#[cfg(target_os = "linux")]
#[link(name = "asound")]
unsafe extern "C" {
    fn snd_lib_error_set_handler(
        handler: Option<extern "C" fn(*const i8, i32, *const i8, i32, *const i8)>,
    );
}

#[cfg(target_os = "linux")]
extern "C" fn no_errors(_: *const i8, _: i32, _: *const i8, _: i32, _: *const i8) {}

#[cfg(target_os = "linux")]
fn suppress_alsa_messages() {
    unsafe {
        snd_lib_error_set_handler(Some(no_errors));
    }
}
