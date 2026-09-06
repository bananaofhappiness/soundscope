mod analyzer;
mod audio_capture;
mod audio_player;
mod builtin_themes;
mod system_sound_capture;
mod tui;
use crate::audio_player::{AudioFile, AudioPlayer, PlaybackPosition, PlayerCommand};
use crossbeam::channel::{bounded, unbounded};
use eyre::Result;
use std::{env, path::PathBuf, thread};

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

    #[cfg(target_os = "macos")]
    let _system_audio_device = unsafe { system_sound_capture::SystemAudioDevice::new() };

    let mut startup_file = None;
    let startup_path = args.get(1).map(PathBuf::from);
    if let Some(f) = startup_path {
        if f.is_file() {
            let current_working_dir = env::current_dir()?;
            startup_file = Some(f.canonicalize()?);
            env::set_current_dir(
                f.parent()
                    .filter(|&s| s.to_str().unwrap() != "")
                    .unwrap_or(&current_working_dir),
            )?;
        } else if f.is_dir() {
            env::set_current_dir(f)?;
        }
    }

    let tui_handler = thread::spawn(|| {
        tui::run(
            None,
            player_command_tx,
            audio_file_rx,
            playback_position_rx,
            error_rx,
            startup_file,
        )
    });
    player.run(&player_command_rx, &audio_file_tx, &error_tx)?;

    let _ = tui_handler.join().unwrap();

    ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::event::DisableMouseCapture
    )?;

    Ok(())
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
