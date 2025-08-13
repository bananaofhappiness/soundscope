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
    analyzer::{self, get_fft, get_waveform},
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

struct WaveForm {
    window: [f64; 2],
    chart: Vec<(f64, f64)>,
    at_zero: bool,
    playhead: usize,
}

impl Default for WaveForm {
    fn default() -> Self {
        Self {
            window: [0., 0.],
            chart: vec![(0., 0.)],
            at_zero: true,
            playhead: 0,
        }
    }
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
    waveform: WaveForm,
    //UI
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
            waveform: WaveForm::default(),
            explorer,
            ui_settings: UISettings::default(),
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        let area = f.area();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
            .split(area);

        self.render_waveform(f, layout[0]);
        self.render_fft_chart(f, layout[1]);
        // render explorer
        if self.ui_settings.show_explorer {
            let area = Self::popup_area(area, 50, 70);
            f.render_widget(Clear, area);
            f.render_widget(&self.explorer.widget(), area);
        }
    }

    fn render_waveform(&mut self, frame: &mut Frame, area: Rect) {
        let playhead_chart = [
            (self.waveform.playhead as f64 / 44., 1.),
            (self.waveform.playhead as f64 / 44. + 0.01, -1.),
        ];

        // get current playback time
        let playhead_position_in_milis =
            Duration::from_millis((self.waveform.playhead as f64 / 44100. * 1000.) as u64);
        let secs = playhead_position_in_milis.as_secs() as f64;
        let millis = playhead_position_in_milis.subsec_millis() as f64;
        let current_time = secs + millis / 1000.0;

        let secs = self.audio_file.duration.as_secs() as f64;
        let millis = self.audio_file.duration.subsec_millis() as f64;
        let total_time = secs + millis / 1000.0;

        let x_labels = vec![
            Span::styled(
                format!("{}", self.waveform.window[0]),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "{}",
                (self.waveform.window[0] + self.waveform.window[1]) / 2.0
            )),
            Span::styled(
                format!("{:?}", self.audio_file.duration),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ];
        let datasets = vec![
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Black))
                .data(&self.waveform.chart),
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Red))
                .data(&playhead_chart),
        ];

        let chart = Chart::new(datasets)
            .block(
                Block::bordered()
                    .title_bottom(Line::from("0").left_aligned())
                    .title_bottom(Line::from(format!("{:.2}s", current_time)).centered())
                    .title_bottom(Line::from(format!("{:.2}s", total_time)).right_aligned()),
            )
            .x_axis(Axis::default().bounds([0., 15. * 1000.]))
            .y_axis(Axis::default().bounds([-1., 1.]));

        frame.render_widget(chart, area);
    }

    fn render_fft_chart(&mut self, frame: &mut Frame, area: Rect) {
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
                // if using mid side we must divide the position by 2
                let pos = pos / 2;
                // get fft
                let fft_left_bound = pos.saturating_sub(16384);
                if fft_left_bound != 0 {
                    let audio_file = &self.audio_file;
                    let mid_samples = &audio_file.mid_samples[fft_left_bound..pos];
                    let side_samples = &audio_file.side_samples[fft_left_bound..pos];

                    self.fft_data.mid_fft = get_fft(mid_samples);
                    self.fft_data.side_fft = get_fft(side_samples);
                }

                //get waveform
                if self.waveform.at_zero {
                    let waveform_samples = &self.audio_file.mid_samples[0..15 * 44100];
                    self.waveform.chart = get_waveform(waveform_samples);
                    self.waveform.playhead = pos;
                }
                let waveform_left_bound = pos.saturating_sub((7.5 * 44100.) as usize);
                let waveform_right_bound = pos.saturating_add((7.5 * 44100.) as usize);

                if waveform_left_bound != 0 {
                    self.waveform.at_zero = false;
                    let waveform_samples =
                        &self.audio_file.mid_samples[waveform_left_bound..waveform_right_bound];
                    self.waveform.chart = get_waveform(waveform_samples);
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
        self.waveform.at_zero = true;
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
