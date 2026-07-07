mod analyzer;
mod audio_capture;
mod audio_player;
mod builtin_themes;
mod tui;
mod waveform_render;
use crate::audio_player::{AudioFile, AudioPlayer, PlaybackPosition, PlayerCommand};
use crossbeam::channel::{bounded, unbounded};
use eyre::{Result, eyre};
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

    // Handle one-shot, non-interactive waveform mode. Prints a static waveform
    // to stdout and exits without spawning the TUI or opening an audio device.
    if args.iter().any(|a| a == "--waveform") {
        return run_waveform(&args[1..]);
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
    player.run(&player_command_rx, &audio_file_tx, &error_tx)
}

fn print_help() {
    println!("Usage: soundscope [OPTIONS] [FILE]");
    println!();
    println!("Arguments:");
    println!("  [FILE]  Audio file to open on startup");
    println!();
    println!("Options:");
    println!("      --waveform     Print a one-shot waveform for FILE to stdout and exit");
    println!("      --width <N>    Waveform width in columns (default: terminal width)");
    println!("  -h, --help         Print help");
    println!("  -v, --version      Print version");
}

/// Parse `--waveform` mode arguments (the slice after the program name) and
/// render a one-shot waveform. Errors are returned and surfaced on stderr.
fn run_waveform(args: &[String]) -> Result<()> {
    let (file, width_override) = parse_waveform_args(args)?;
    if !file.is_file() {
        return Err(eyre!("not an audio file: {}", file.display()));
    }
    waveform_render::run(&file, width_override)
}

/// Parse `--waveform` mode arguments (the slice after the program name) into the
/// target file and an optional width override. Pure: performs no filesystem
/// access, so it can be unit-tested. Rejects unknown flags and extra positional
/// arguments rather than silently ignoring them.
fn parse_waveform_args(args: &[String]) -> Result<(PathBuf, Option<usize>)> {
    let mut width_override: Option<usize> = None;
    let mut file: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--waveform" => {}
            "--width" => {
                i += 1;
                let value = args.get(i).ok_or_else(|| eyre!("--width requires a value"))?;
                let width: usize = value
                    .parse()
                    .map_err(|_| eyre!("--width must be a positive integer, got '{value}'"))?;
                if width == 0 {
                    return Err(eyre!("--width must be greater than 0"));
                }
                width_override = Some(width);
            }
            other if other.starts_with('-') => {
                return Err(eyre!("unknown option '{other}' (see --help)"));
            }
            other => {
                if file.is_none() {
                    file = Some(PathBuf::from(other));
                } else {
                    return Err(eyre!("unexpected extra argument '{other}'"));
                }
            }
        }
        i += 1;
    }

    let file = file.ok_or_else(|| eyre!("--waveform requires an audio file path"))?;
    Ok((file, width_override))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_file_only() {
        let (file, width) = parse_waveform_args(&args(&["--waveform", "song.mp3"])).unwrap();
        assert_eq!(file, PathBuf::from("song.mp3"));
        assert_eq!(width, None);
    }

    #[test]
    fn parses_width_override_in_any_order() {
        let (file, width) =
            parse_waveform_args(&args(&["--waveform", "--width", "120", "song.mp3"])).unwrap();
        assert_eq!(file, PathBuf::from("song.mp3"));
        assert_eq!(width, Some(120));
    }

    #[test]
    fn rejects_extra_positional_argument() {
        let err = parse_waveform_args(&args(&["--waveform", "a.mp3", "b.mp3"])).unwrap_err();
        assert!(err.to_string().contains("extra argument"));
    }

    #[test]
    fn rejects_missing_file() {
        assert!(parse_waveform_args(&args(&["--waveform"])).is_err());
    }

    #[test]
    fn rejects_zero_width() {
        assert!(parse_waveform_args(&args(&["--waveform", "--width", "0", "a.mp3"])).is_err());
    }

    #[test]
    fn rejects_non_numeric_width() {
        assert!(parse_waveform_args(&args(&["--waveform", "--width", "abc", "a.mp3"])).is_err());
    }

    #[test]
    fn rejects_width_without_value() {
        assert!(parse_waveform_args(&args(&["--waveform", "--width"])).is_err());
    }

    #[test]
    fn rejects_unknown_option() {
        assert!(parse_waveform_args(&args(&["--waveform", "--bogus", "a.mp3"])).is_err());
    }
}
