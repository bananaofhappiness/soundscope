use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum AudioCommand {
    LoadFile(String),
    Play,
    Pause,
    Stop,
    Seek(f64),
}

#[derive(Debug, Clone, Default)]
pub struct AudioState {
    pub is_playing: bool,
    pub position: f64,
    pub duration: f64,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AnalysisData {
    pub frequencies: Vec<(f32, f32)>,
    pub lufs: Option<f32>,
    pub waveform: Vec<f32>,
    pub rms: Option<f32>,
}

pub struct AppState {
    pub audio_cmd_tx: Sender<AudioCommand>,
    pub shared_state: Arc<Mutex<(AudioState, AnalysisData)>>,
    pub show_explorer: bool,
    pub selected_file: Option<String>,
}
