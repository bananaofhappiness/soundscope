use color_eyre::Result;
use crossbeam::channel::{Receiver, Sender};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{Event, KeyCode, poll, read},
    layout::Flex,
    prelude::*,
    widgets::{Axis, Block, Chart, Clear, Dataset, GraphType},
};
use ratatui_explorer::{FileExplorer, Theme};
use std::{fmt, time::Duration, usize::MAX};
use symphonia::core::sample::Sample;

use crate::{
    analyzer::{self, get_fft},
    audio_player::{AudioFile, PlayerCommand, Samples},
};

/// Settings like showing/hiding UI elements
struct UISettings {
    show_explorer: bool,
    show_mid_fft: bool,
    show_side_fft: bool,
}

impl Default for UISettings {
    fn default() -> Self {
        Self {
            show_explorer: false,
            show_mid_fft: true,
            show_side_fft: false,
        }
    }
}

#[derive(Default)]
struct FFTData {
    mid_fft: Vec<(f64, f64)>,
    side_fft: Vec<(f64, f64)>,
}

/// `App` contains the necessary components for the application like tx, rx, UI settings.
struct App {
    /// Audio file which is loaded into the player.
    /// Must be wrapped into [`Option`] because audio file does not exist initially.
    /// After choosing a file it is never [`None`] again.
    audio_file: AudioFile,
    audio_file_rx: Receiver<AudioFile>,
    /// Sends commands like pause and play to the player.
    player_command_tx: Sender<PlayerCommand>,
    /// Gets playback position for an analyzer to know what samples to analyze.
    playback_position_rx: Receiver<usize>,
    // Charts data
    fft_data: FFTData,
    explorer: FileExplorer,
    ui_settings: UISettings,
}

impl App {
    fn new(
        audio_file: AudioFile,
        player_command_tx: Sender<PlayerCommand>,
        audio_file_rx: Receiver<AudioFile>,
        playback_position_rx: Receiver<usize>,
        explorer: FileExplorer,
    ) -> Self {
        Self {
            audio_file,
            audio_file_rx,
            player_command_tx,
            playback_position_rx,
            fft_data: FFTData::default(),
            explorer,
            ui_settings: UISettings::default(),
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        let area = f.area();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
            .split(area);

        self.render_fft(f, layout[1]);
        // render explorer
        if self.ui_settings.show_explorer {
            let area = Self::popup_area(area, 50, 70);
            f.render_widget(Clear, area);
            f.render_widget(&self.explorer.widget(), area);
        }
    }

    fn render_fft(&mut self, frame: &mut Frame, area: Rect) {
        let x_labels = vec![
            Span::styled("20Hz", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("632Hz"),
            Span::styled("20kHz", Style::default().add_modifier(Modifier::BOLD)),
        ];

        let mut datasets = Vec::new();
        if self.ui_settings.show_mid_fft {
            datasets.push(
                Dataset::default()
                    .name("Mid Frequency")
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(Color::Green))
                    .data(&self.fft_data.mid_fft),
            );
        }
        if self.ui_settings.show_side_fft {
            datasets.push(
                Dataset::default()
                    .name("Side Frequency")
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(Color::Red))
                    .data(&self.fft_data.side_fft),
            );
        }

        let chart = Chart::new(datasets)
            .block(Block::bordered())
            .x_axis(
                Axis::default()
                    .title("Hz")
                    .style(Style::default().fg(Color::Black))
                    .labels(x_labels)
                    .bounds([0., 100.]),
            )
            .y_axis(
                Axis::default()
                    .title("Db")
                    .style(Style::default().fg(Color::Black))
                    .labels(vec![Span::raw("idk"), Span::raw("some db")])
                    .bounds([0., 250.]),
            );

        frame.render_widget(chart, area);
    }

    fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|f| self.draw(f))?;

            // receive audio file
            if let Ok(af) = self.audio_file_rx.try_recv() {
                self.audio_file = af;
            }

            // receive playback position
            if let Ok(pos) = self.playback_position_rx.try_recv() {
                let left_bound = pos.saturating_sub(16384);
                if left_bound != 0 {
                    let audio_file = &self.audio_file;
                    let mid_samples = &audio_file.mid_samples[left_bound..pos];
                    let side_samples = &audio_file.side_samples[left_bound..pos];

                    // get fft
                    self.fft_data.mid_fft = get_fft(mid_samples);
                    self.fft_data.side_fft = get_fft(side_samples);
                }
            }

            // event reader
            if poll(Duration::from_micros(1))? {
                let event = read()?;
                if let Event::Key(key) = event {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('e') => {
                            self.ui_settings.show_explorer = !self.ui_settings.show_explorer
                        }
                        KeyCode::Enter => self.select_file(),
                        KeyCode::Char('s') => {
                            self.ui_settings.show_side_fft = !self.ui_settings.show_side_fft
                        }
                        KeyCode::Char('m') => {
                            self.ui_settings.show_mid_fft = !self.ui_settings.show_mid_fft
                        }
                        KeyCode::Char(' ') => {
                            if let Err(err) =
                                self.player_command_tx.send(PlayerCommand::ChangeState)
                            {
                                //do smth idk
                            }
                        }
                        _ => (),
                    }
                }
                if self.ui_settings.show_explorer {
                    self.explorer.handle(&event)?;
                }
            }
        }
    }

    fn select_file(&mut self) {
        let file = self.explorer.current();
        // let file_name = self.explorer.current().name();
        let file_path = self.explorer.current().path().to_str().unwrap().to_owned();
        if !file.is_file() {
            return;
        }
        // audio_file.lock().unwrap().load_file(&file_path)?;
        self.ui_settings.show_explorer = false;
        if let Err(err) = self
            .player_command_tx
            .send(PlayerCommand::SelectFile(file_path))
        {
            //do smth idk
        }
        // Ok(())
    }

    fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
        let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);
        area
    }
}

pub fn run(
    audio_file: AudioFile,
    player_command_tx: Sender<PlayerCommand>,
    audio_file_rx: Receiver<AudioFile>,
    playback_position_rx: Receiver<usize>,
) -> Result<()> {
    let terminal = ratatui::init();
    let theme = Theme::default()
        .add_default_title()
        .with_item_style(Style::default().fg(Color::Black));
    let file_explorer = FileExplorer::with_theme(theme)?;
    let app_result = App::new(
        audio_file,
        player_command_tx,
        audio_file_rx,
        playback_position_rx,
        file_explorer,
    )
    .run(terminal);
    ratatui::restore();
    app_result
}
