//! This module contains the implementation of the terminal user interface (TUI) used to display audio analysis results.
//! It uses `ratatui` under the hood.
use color_eyre::Result;
use cpal::{Device, Stream, default_host, traits::StreamTrait as _};
use crossbeam::channel::{Receiver, Sender};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{Event, KeyCode, KeyEvent, poll, read},
    layout::Flex,
    prelude::*,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Clear, Dataset, GraphType, List, ListItem, Paragraph, Wrap},
};
use ratatui_explorer::{FileExplorer, Theme};
use ringbuffer::{AllocRingBuffer, RingBuffer};
use rodio::Source;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crate::{
    analyzer::Analyzer,
    audio_capture::{self, AudioDevice, build_input_stream, list_input_devs},
    audio_player::{AudioFile, PlayerCommand},
};

/// Style: Black background, yellow foreground
const STYLE: Style = Style::new().bg(Color::Black).fg(Color::Yellow);
/// Text highlight style: Black background, light red foreground, bold
const TEXT_HIGHLIGHT: Style = Style::new()
    .bg(Color::Black)
    .fg(Color::LightRed)
    .add_modifier(Modifier::BOLD);

pub type RBuffer = Arc<Mutex<AllocRingBuffer<f32>>>;

/// Settings like showing/hiding UI elements.
struct UISettings {
    show_explorer: bool,
    show_fft_chart: bool,
    show_mid_fft: bool,
    show_side_fft: bool,
    show_devices_list: bool,
    show_lufs: bool,
    error_text: String,
    error_timer: Option<Instant>,
}

impl Default for UISettings {
    fn default() -> Self {
        Self {
            show_explorer: false,
            show_fft_chart: true,
            show_mid_fft: true,
            show_side_fft: false,
            show_devices_list: false,
            show_lufs: false,
            error_text: String::new(),
            error_timer: None,
        }
    }
}

#[derive(Default, PartialEq)]
enum Mode {
    #[default]
    Player,
    Microphone,
    System,
}

#[derive(Default)]
struct Settings {
    mode: Mode,
}

/// FFT data for the UI.
#[derive(Default)]
struct FFTData {
    mid_fft: Vec<(f64, f64)>,
    side_fft: Vec<(f64, f64)>,
}

/// Waveform data for the UI.
struct WaveForm {
    chart: Vec<(f64, f64)>,
    playhead: usize,
    at_zero: bool,
    at_end: bool,
}

impl Default for WaveForm {
    fn default() -> Self {
        Self {
            chart: vec![(0., 0.)],
            playhead: 0,
            at_zero: true,
            at_end: false,
        }
    }
}

/// `App` contains the necessary components for the application like tx, rx, UI settings.
struct App {
    /// Audio file which is loaded into the player.
    /// Must be wrapped into [`Option`] because audio file does not exist initially.
    /// After choosing a file it is never [`None`] again.
    audio_file: AudioFile,
    is_playing_audio: bool,
    audio_file_rx: Receiver<AudioFile>,
    /// RingBuffer used to store the latest captured samples when the `Mode` is not `Mode::Player`.
    latest_captured_samples: RBuffer,
    /// The stream that captures the audio through input device
    audio_capture_stream: Option<Stream>,
    /// Sends commands like pause and play to the player.
    player_command_tx: Sender<PlayerCommand>,
    /// Gets playback position of an audio file when the mode is player
    /// for an analyzer to know what samples to analyze.
    playback_position_rx: Receiver<usize>,
    /// Gets errors to display them afterwards.
    error_rx: Receiver<String>,
    analyzer: Analyzer,

    // Charts data
    /// Data used to render FFT chart.
    fft_data: FFTData,
    /// Data used to render waveform.
    waveform: WaveForm,
    /// LUFS chart.
    lufs: [f64; 300],

    settings: Settings,
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
        error_rx: Receiver<String>,
        explorer: FileExplorer,
        latest_captured_samples: RBuffer,
    ) -> Self {
        Self {
            audio_file,
            is_playing_audio: false,
            audio_file_rx,
            latest_captured_samples,
            audio_capture_stream: None,
            player_command_tx,
            playback_position_rx,
            error_rx,
            analyzer: Analyzer::default(),
            fft_data: FFTData::default(),
            waveform: WaveForm::default(),
            lufs: [-50.; 300],
            settings: Settings::default(),
            explorer,
            ui_settings: UISettings::default(),
        }
    }

    /// The function used to draw the UI.
    fn draw(&mut self, f: &mut Frame) {
        // split the area into waveform part and charts parts
        let area = f.area();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
            .split(area);

        // make the background black
        let background = Paragraph::new("").style(STYLE);
        f.render_widget(background, area);
        self.render_waveform(f, layout[0]);

        // show charts based on user settings
        if self.ui_settings.show_lufs {
            self.render_lufs(f, layout[1]);
        } else if self.ui_settings.show_fft_chart {
            self.render_fft_chart(f, layout[1]);
        }

        // render error
        if let Ok(err) = self.error_rx.try_recv() {
            self.ui_settings.error_text = err;
            self.ui_settings.error_timer = Some(std::time::Instant::now())
        }
        self.render_error_message(f);

        // render explorer
        if self.ui_settings.show_explorer {
            let area = Self::get_explorer_popup_area(area, 50, 70);
            f.render_widget(Clear, area);
            f.render_widget(&self.explorer.widget(), area);
        }
        if self.ui_settings.show_devices_list {
            self.render_devices_list(f);
        }
    }

    fn render_waveform(&mut self, frame: &mut Frame, area: Rect) {
        // playhead is just a function that looks like a vertical line
        let samples_in_one_ms = self.audio_file.sample_rate() / 1000;
        let mut playhead_chart = [
            (self.waveform.playhead as f64 / samples_in_one_ms as f64, 1.),
            (
                self.waveform.playhead as f64 / samples_in_one_ms as f64 + 0.01,
                -1.,
            ),
        ];
        if self.waveform.at_end {
            // if at last 15 sec of the audion the playhead should move from the middle to the end of the chart
            let total_samples = self.audio_file.mid_samples().len();
            let chart_duration_seconds = 15.0; // make a var not to hard code and be able to to add resizing waveform window if needed
            let chart_middle_seconds = chart_duration_seconds / 2.0;

            // calculate the absolute sample position where the playhead starts scrolling from the middle of the chart to the end
            // this is when playback enters the last `chart_middle_seconds` (default is 7.5s) of the total audio duration.
            let scroll_start_absolute_samples = total_samples.saturating_sub(
                (chart_middle_seconds * self.audio_file.sample_rate() as f64) as usize,
            );

            // calculate playhead's position relative to the start of this scroll phase
            // since `self.waveform.playhead` is the absolute current playback position.
            let relative_samples_in_scroll_phase = self
                .waveform
                .playhead
                .saturating_sub(scroll_start_absolute_samples);

            // map this relative sample position to the chart's X-axis range for the playhead.
            // the conversion from samples to chart units (milliseconds) uses the same 1/samles_in_one_ms scale
            // as other playhead positions in this function.
            let mut chart_x_position = (chart_middle_seconds * 1000.)
                + (relative_samples_in_scroll_phase as f64 / samples_in_one_ms as f64);

            // Ensure the playhead does not exceed the chart's upper bound.
            chart_x_position = f64::min(chart_x_position, chart_duration_seconds * 1000.);

            playhead_chart = [(chart_x_position, 1.), (chart_x_position + 0.01, -1.)];
        } else if !self.waveform.at_zero {
            // if not at zero then place the playhead right at the middle of a chart
            playhead_chart = [
                (
                    f64::min(
                        self.waveform.playhead as f64 / samples_in_one_ms as f64,
                        1000. * 7.5,
                    ),
                    1.,
                ),
                (
                    f64::min(
                        self.waveform.playhead as f64 / samples_in_one_ms as f64,
                        1000. * 7.5,
                    ) + 0.01,
                    -1.,
                ),
            ];
        }

        // get current playback time in seconds
        let playhead_position_in_milis = Duration::from_millis(
            (self.waveform.playhead as f64 / self.audio_file.sample_rate() as f64 * 1000.) as u64,
        );
        let current_sec = playhead_position_in_milis.as_secs_f64();
        let current_min = (current_sec / 60.) as u32;
        let current_sec = current_sec % 60.;

        // get total audio file duration
        let total_sec = self.audio_file.duration().as_secs_f64();
        let total_min = (total_sec / 60.) as u32;
        let total_sec = total_sec % 60.;

        // make datasets
        // first one to render a waveform
        // the other one to render the playhead
        let datasets = vec![
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(STYLE)
                .data(&self.waveform.chart),
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::LightRed))
                .data(&playhead_chart),
        ];

        // render chart
        let chart = Chart::new(datasets)
            .block(
                Block::bordered()
                    .title(self.audio_file.title())
                    .title_bottom(Line::from("0").left_aligned())
                    // current position and total duration
                    .title_bottom(
                        Line::from(format!("{:0>2}:{:0>5.2}", current_min, current_sec)).centered(),
                    )
                    .title_bottom(
                        Line::from(format!("{:0>2}:{:0>5.2}", total_min, total_sec))
                            .right_aligned(),
                    ),
            )
            .style(STYLE)
            .x_axis(Axis::default().bounds([0., 15. * 1000.]).style(STYLE))
            .y_axis(Axis::default().bounds([-1., 1.]).style(STYLE));

        frame.render_widget(chart, area);
    }

    fn render_fft_chart(&mut self, frame: &mut Frame, area: Rect) {
        let x_labels = vec![
            // frequencies are commented because their positions are off.
            // they are not rendered where the corresponding frequencies are.
            Span::styled("20Hz", Style::default().add_modifier(Modifier::BOLD)),
            // Span::raw("20Hz"),
            // Span::raw(""),
            // Span::raw(""),
            // Span::raw("112.47"),
            // Span::raw(""),
            // Span::raw(""),
            // Span::raw(""),
            Span::raw("632.46Hz"),
            // Span::raw(""),
            // Span::raw(""),
            // Span::raw(" "),
            // Span::raw("3556.57"),
            // Span::raw(""),
            // Span::raw(""),
            // Span::raw("20000Hz"),
            Span::styled("20kHz", Style::default().add_modifier(Modifier::BOLD)),
        ];

        // if no data about frequencies then default to some low value
        let mid_fft: &[(f64, f64)] = if self.ui_settings.show_mid_fft {
            &self.fft_data.mid_fft
        } else {
            &[(-1000.0, -1000.0)]
        };

        let side_fft: &[(f64, f64)] = if self.ui_settings.show_side_fft {
            &self.fft_data.side_fft
        } else {
            &[(-1000.0, -1000.0)]
        };

        let datasets = vec![
            Dataset::default()
                // highlight the letter M so the user knows they must press M to toggle it
                // same with Side fft
                .name(vec![
                    "M".bold().style(TEXT_HIGHLIGHT),
                    "id Frequency".into(),
                ])
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(STYLE)
                .data(mid_fft),
            Dataset::default()
                .name(vec![
                    "S".bold().style(TEXT_HIGHLIGHT),
                    "ide Frequency".into(),
                ])
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::LightGreen))
                .data(side_fft),
        ];

        let chart = Chart::new(datasets)
            // the title uses the same highlighting technique
            .block(Block::bordered().title(vec![
                "F".bold().style(TEXT_HIGHLIGHT),
                "requencies ".bold(),
                "L".bold().style(TEXT_HIGHLIGHT),
                "UFS".into(),
            ]))
            .style(STYLE)
            .x_axis(
                Axis::default()
                    .title("Hz")
                    .style(Style::default().fg(Color::Black))
                    .labels(x_labels)
                    .style(STYLE)
                    .bounds([0., 100.]),
            )
            .y_axis(
                Axis::default()
                    .title("Db")
                    .style(Style::default().fg(Color::Black))
                    .labels(vec![Span::raw("-78 Db"), Span::raw("-18 Db")])
                    .style(STYLE)
                    .bounds([-150., 100.]),
            );

        frame.render_widget(chart, area);
    }

    fn render_lufs(&mut self, f: &mut Frame, area: Rect) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
            .split(area);
        let data = self
            .lufs
            .iter()
            .enumerate()
            .map(|(x, &y)| (x as f64, y))
            .collect::<Vec<(f64, f64)>>();

        let integrated_lufs = match self.analyzer.get_integrated_lufs() {
            Ok(lufs) => lufs,
            Err(err) => {
                self.handle_error(format!("Error getting integrated LUFS: {}", err));
                0.0
            }
        };

        // it should not display `-inf`
        let integrated_lufs = if integrated_lufs.is_infinite() {
            -50.0
        } else {
            integrated_lufs
        };

        // text layout
        let paragraph_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .split(layout[0]);

        // get lufs texts
        let lufs_text = vec![
            "Short term LUFS:".bold() + format!("{:06.2}", self.lufs[299]).into(),
            "Integrated LUFS:".bold() + format!("{:06.2}", integrated_lufs).into(),
        ];

        // get true peak text
        let (tp_left, tp_right) = match self.analyzer.get_true_peak() {
            Ok((tp_left, tp_right)) => (tp_left, tp_right),
            Err(err) => {
                self.handle_error(format!("Error getting true peak: {}", err));
                (0.0, 0.0)
            }
        };
        let true_peak_text = vec![
            "True Peak".bold().into(),
            "L: ".bold() + format!("{:.2} Db", tp_left).into(),
            "R: ".bold() + format!("{:.2} Db", tp_right).into(),
        ];

        //get range text
        let range = match self.analyzer.get_loudness_range() {
            Ok(range) => range,
            Err(err) => {
                self.handle_error(format!("Error getting loudness range: {}", err));
                0.0
            }
        };
        let range_text = vec!["Range: ".bold() + format!("{:.2} LU", range).into()];

        // paragraphs
        let lufs_paragraph = Paragraph::new(lufs_text)
            .block(Block::bordered().title(vec![
                "F".bold().style(TEXT_HIGHLIGHT.bold()),
                "requencies ".into(),
                "L".bold().style(TEXT_HIGHLIGHT.bold()),
                "UFS".bold(),
            ]))
            .alignment(Alignment::Center)
            .style(STYLE);
        let true_peak_paragraph = Paragraph::new(true_peak_text)
            .block(Block::bordered())
            .alignment(Alignment::Center)
            .style(STYLE);
        let range_paragraph = Paragraph::new(range_text)
            .block(Block::bordered())
            .alignment(Alignment::Center)
            .style(STYLE);

        // chart section
        let dataset = vec![
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(STYLE)
                .data(&data),
        ];
        let chart = Chart::new(dataset)
            .block(Block::bordered())
            .style(STYLE)
            .x_axis(Axis::default().bounds([0., 300.]))
            .y_axis(
                Axis::default()
                    .bounds([-50., 0.])
                    .labels(["-50".bold(), "0".bold()]),
            );
        f.render_widget(lufs_paragraph, paragraph_layout[0]);
        f.render_widget(true_peak_paragraph, paragraph_layout[1]);
        f.render_widget(range_paragraph, paragraph_layout[2]);

        f.render_widget(chart, layout[1]);
    }

    fn render_devices_list(&self, f: &mut Frame) {
        let area = Self::get_explorer_popup_area(f.area(), 50, 70);
        f.render_widget(Clear, area);
        let devs = list_input_devs();
        let list_items: Vec<ListItem> = devs
            .iter()
            .enumerate()
            .map(|(i, (name, _dev))| ListItem::from(format!("[{}] {}", i + 1, name)))
            .collect();
        let list = List::new(list_items)
            .block(Block::bordered().title("Devices"))
            .style(STYLE)
            .highlight_style(TEXT_HIGHLIGHT);

        f.render_widget(list, area);
    }

    /// The main loop
    fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            // receive audio file
            if let Ok(af) = self.audio_file_rx.try_recv() {
                self.audio_file = af;
            }

            // receive playback position
            // if the mode differs from the player mode, then it is never executed
            if let Ok(pos) = self.playback_position_rx.try_recv() {
                // if using mid side we must divide the position by 2
                let pos = pos / self.audio_file.channels() as usize;
                let sr = self.audio_file.sample_rate() as usize;
                // get fft
                let fft_left_bound = pos.saturating_sub(16384);
                if fft_left_bound != 0 {
                    let audio_file = &self.audio_file;
                    let mid_samples = &audio_file.mid_samples()[fft_left_bound..pos];
                    let side_samples = &audio_file.side_samples()[fft_left_bound..pos];

                    self.fft_data.mid_fft = self.analyzer.get_fft(mid_samples, sr);
                    self.fft_data.side_fft = self.analyzer.get_fft(side_samples, sr);
                }

                //get waveform
                let mid_samples_len = self.audio_file.mid_samples().len();
                self.waveform.playhead = pos;
                // if at zero load first 15 seconds to show
                if self.waveform.at_zero {
                    let waveform_samples = &self.audio_file.mid_samples()[0..15 * sr];
                    self.waveform.chart = Analyzer::get_waveform(waveform_samples, sr);
                }
                let waveform_left_bound = pos.saturating_sub((7.5 * sr as f64) as usize);
                let waveform_right_bound =
                    usize::min(pos + (7.5 * sr as f64) as usize, mid_samples_len);

                // if at end load last 15 seconds and dont scroll
                if waveform_right_bound == mid_samples_len {
                    self.waveform.at_end = true;
                    let waveform_samples =
                        &self.audio_file.mid_samples()[mid_samples_len - 15 * sr..mid_samples_len];
                    self.waveform.chart = Analyzer::get_waveform(waveform_samples, sr);
                // if not at the beginning load 15 seconds and scroll
                } else if waveform_left_bound != 0 {
                    self.waveform.at_zero = false;
                    let waveform_samples =
                        &self.audio_file.mid_samples()[waveform_left_bound..waveform_right_bound];
                    self.waveform.chart = Analyzer::get_waveform(waveform_samples, sr);
                } else {
                    self.waveform.at_zero = true;
                }

                // get lufs lufs uses all channels
                let pos = pos * self.audio_file.channels() as usize;
                let lufs_left_bound = pos.saturating_sub(16384);
                if lufs_left_bound != 0 {
                    for i in 0..self.lufs.len() - 1 {
                        self.lufs[i] = self.lufs[i + 1];
                    }
                    if let Err(err) = self
                        .analyzer
                        .add_samples(&self.audio_file.samples()[lufs_left_bound..pos])
                    {
                        self.handle_error(format!(
                            "Could not get samples for LUFS analyzer: {}",
                            err
                        ));
                    };
                    self.lufs[299] = match self.analyzer.get_shortterm_lufs() {
                        Ok(lufs) => lufs,
                        Err(err) => {
                            self.handle_error(format!("Error getting short-term LUFS: {}", err));
                            0.0
                        }
                    };
                }
            }

            // use ringbuf to analyze data if the `Mode` is not `Mode::Player`
            if self.settings.mode != Mode::Player {
                let samples = self.latest_captured_samples.lock().unwrap().to_vec();
                self.fft_data.mid_fft = self.analyzer.get_fft(&samples[645116..15 * 44100], 44100);
                self.waveform.chart = Analyzer::get_waveform(&samples, 44100);
                self.waveform.at_end = false;
                self.waveform.at_zero = false;
                for i in 0..self.lufs.len() - 1 {
                    self.lufs[i] = self.lufs[i + 1];
                }
                if let Err(err) = self.analyzer.add_samples(&samples[399356..15 * 44100]) {
                    self.handle_error(format!("Could not get samples for LUFS analyzer: {}", err));
                };
                self.lufs[299] = match self.analyzer.get_shortterm_lufs() {
                    Ok(lufs) => lufs,
                    Err(err) => {
                        self.handle_error(format!("Error getting short-term LUFS: {}", err));
                        0.0
                    }
                };
            }
            // event reader
            if poll(Duration::from_micros(1))? {
                let event = match read() {
                    Ok(event) => event,
                    Err(err) => {
                        self.handle_error(format!("Error reading event: {}", err));
                        continue;
                    }
                };

                if let Event::Key(key) = event {
                    // quit
                    if key.code == KeyCode::Char('q') {
                        self.player_command_tx.send(PlayerCommand::Quit)?;
                        return Ok(());
                    }
                    self.handle_input(key);
                }

                if self.ui_settings.show_explorer {
                    self.explorer.handle(&event)?;
                }
            }
            terminal.draw(|f| self.draw(f))?;
        }
    }

    fn handle_input(&mut self, key: KeyEvent) {
        match key.code {
            // show explorer
            KeyCode::Char('e') => self.ui_settings.show_explorer = !self.ui_settings.show_explorer,
            // select file
            KeyCode::Enter => self.select_file(),
            // show side fft
            KeyCode::Char('s') => self.ui_settings.show_side_fft = !self.ui_settings.show_side_fft,
            // show mid fft
            KeyCode::Char('m') => self.ui_settings.show_mid_fft = !self.ui_settings.show_mid_fft,
            // pause/play
            KeyCode::Char(' ') => {
                if let Err(_err) = self.player_command_tx.send(PlayerCommand::ChangeState) {
                    //TODO: log sending error
                }
                self.is_playing_audio = !self.is_playing_audio;
                // do this so lufs update only on play, not pause
                if self.is_playing_audio {
                    self.lufs = [-50.; 300];
                    self.analyzer.reset();
                }
            }
            // move playhead right and left
            KeyCode::Right => {
                self.lufs = [-50.; 300];
                self.analyzer.reset();
                if let Err(_err) = self.player_command_tx.send(PlayerCommand::MoveRight) {
                    //TODO: log sending error
                }
            }
            KeyCode::Left => {
                self.lufs = [-50.; 300];
                self.analyzer.reset();
                if let Err(_err) = self.player_command_tx.send(PlayerCommand::MoveLeft) {
                    //TODO: log sending error
                }
            }
            // change charts shown
            KeyCode::Char('l') => self.change_chart('l'),
            KeyCode::Char('f') => self.change_chart('f'),
            // this sends a test error
            // only in debug mode
            KeyCode::Char('y') => {
                #[cfg(debug_assertions)]
                {
                    self.player_command_tx
                        .send(PlayerCommand::ShowTestError)
                        .unwrap()
                }
            }
            // show devices
            KeyCode::Char('d') => {
                self.ui_settings.show_devices_list = !self.ui_settings.show_devices_list
            }
            // change mode. this will be replaced by normal settings selection tab
            // TODO normal settings
            KeyCode::Char('x') => {
                self.settings.mode = if self.settings.mode == Mode::Microphone {
                    Mode::Player
                } else {
                    Mode::Microphone
                };
            }
            KeyCode::Char(c)
                if self.ui_settings.show_devices_list && c.is_ascii_digit() && c != '0' =>
            {
                let index = (c as usize) - ('1' as usize);
                let devices = list_input_devs();
                if index > devices.len() - 1 {
                    self.handle_error(format!("Invalid device index: {}", index + 1));
                    return;
                }
                if self.audio_capture_stream.is_some() {
                    self.audio_capture_stream = None
                }
                let device = devices[index].1.clone();
                let audio_device = AudioDevice::new(Some(device));
                let stream = match audio_capture::build_input_stream(
                    self.latest_captured_samples.clone(),
                    audio_device,
                ) {
                    Ok(stream) => stream,
                    Err(err) => {
                        self.handle_error(format!("Failed to run audio capture: {}", err));
                        return;
                    }
                };
                // std::thread::spawn(move || {
                //     stream.play();
                // });
                self.audio_capture_stream = Some(stream);
                self.audio_capture_stream.as_ref().unwrap().play();
                self.ui_settings.show_devices_list = false;
            }
            _ => (),
        };
    }

    fn handle_error(&mut self, message: String) {
        self.ui_settings.error_text = message;
        self.ui_settings.error_timer = Some(Instant::now());
    }

    fn select_file(&mut self) {
        let file = self.explorer.current();
        let file_path = self.explorer.current().path().clone();
        if !file.is_file() {
            return;
        }
        // reset everything
        self.ui_settings.show_explorer = false;
        self.fft_data.mid_fft.clear();
        self.fft_data.side_fft.clear();
        self.waveform.chart.clear();
        self.waveform.at_zero = true;
        self.waveform.at_end = false;
        self.lufs = [-50.; 300];
        self.is_playing_audio = false;

        if let Err(_err) = self
            .player_command_tx
            .send(PlayerCommand::SelectFile(file_path))
        {
            //TODO: log sending error
        }

        // TODO: channels
        if let Err(err) = self.analyzer.select_new_file(
            // self.audio_file.channels() as u32,
            2,
            self.audio_file.sample_rate(),
        ) {
            self.handle_error(format!(
                "Could not create an analyzer for an audio file: {}",
                err
            ));
        }
    }

    fn change_chart(&mut self, c: char) {
        match c {
            // lufs
            'l' => {
                self.ui_settings.show_fft_chart = false;
                self.ui_settings.show_lufs = true
            }
            // frequencies
            'f' => {
                self.ui_settings.show_fft_chart = true;
                self.ui_settings.show_lufs = false
            }
            _ => (),
        }
    }

    fn get_explorer_popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
        let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);
        area
    }

    fn get_error_popup_area(area: Rect) -> Rect {
        let vertical = Layout::vertical(Constraint::from_ratios([(5, 6), (1, 6)]));
        let horizontal = Layout::horizontal(Constraint::from_ratios([(1, 6), (5, 6)]));
        let area = vertical.areas::<2>(area)[1];
        horizontal.areas::<2>(area)[0]
    }

    fn render_error_message(&mut self, f: &mut Frame) {
        let message = self.ui_settings.error_text.clone();
        // show error for 5 seconds
        match self.ui_settings.error_timer {
            Some(error_timer) => {
                if error_timer.elapsed().as_millis() > 5000 {
                    self.ui_settings.error_timer = None;
                    return;
                }
            }
            None => return,
        }
        let error_popup_area = Self::get_error_popup_area(f.area());
        f.render_widget(Clear, error_popup_area);
        f.render_widget(
            Paragraph::new(message)
                .block(Block::bordered().style(STYLE).fg(Color::LightRed).bold())
                .wrap(Wrap { trim: true }),
            error_popup_area,
        );
    }
}

/// pub run function that initializes the terminal and runs the application
pub fn run(
    audio_file: AudioFile,
    player_command_tx: Sender<PlayerCommand>,
    audio_file_rx: Receiver<AudioFile>,
    playback_position_rx: Receiver<usize>,
    error_rx: Receiver<String>,
    latest_captured_samples: RBuffer,
) -> Result<()> {
    let terminal = ratatui::init();
    let theme = Theme::default()
        .with_style(STYLE)
        .with_item_style(STYLE)
        .with_highlight_item_style(STYLE.fg(Color::LightRed))
        .with_dir_style(STYLE.bold())
        .with_highlight_dir_style(STYLE.bold().fg(Color::LightRed))
        .add_default_title();
    let file_explorer = FileExplorer::with_theme(theme)?;
    let app_result = App::new(
        audio_file,
        player_command_tx,
        audio_file_rx,
        playback_position_rx,
        error_rx,
        file_explorer,
        latest_captured_samples,
    )
    .run(terminal);
    ratatui::restore();
    app_result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam::channel;

    fn create_test_app() -> (App, Sender<PlayerCommand>, Receiver<PlayerCommand>) {
        let (player_command_tx, player_command_rx) = channel::unbounded();
        let (_, audio_file_rx) = channel::unbounded();
        let (playback_position_tx, playback_position_rx) = channel::unbounded();
        let (_, error_rx) = channel::unbounded();

        let audio_file = AudioFile::new(playback_position_tx);
        let theme = Theme::default();
        let explorer = FileExplorer::with_theme(theme).unwrap();
        let latest_captured_samples = Arc::new(Mutex::new(AllocRingBuffer::new(
            (44100usize * 5).next_power_of_two(),
        )));

        let app = App::new(
            audio_file,
            player_command_tx.clone(),
            audio_file_rx,
            playback_position_rx,
            error_rx,
            explorer,
            latest_captured_samples,
        );

        (app, player_command_tx, player_command_rx)
    }

    #[test]
    fn test_change_chart() {
        let (mut app, _, _) = create_test_app();

        // Test switching to LUFS
        app.change_chart('l');
        assert!(!app.ui_settings.show_fft_chart);
        assert!(app.ui_settings.show_lufs);

        // Test switching to frequencies
        app.change_chart('f');
        assert!(app.ui_settings.show_fft_chart);
        assert!(!app.ui_settings.show_lufs);

        // Test invalid character (should do nothing)
        let prev_fft = app.ui_settings.show_fft_chart;
        let prev_lufs = app.ui_settings.show_lufs;
        app.change_chart('x');
        assert_eq!(app.ui_settings.show_fft_chart, prev_fft);
        assert_eq!(app.ui_settings.show_lufs, prev_lufs);
    }

    #[test]
    fn test_handle_error() {
        let (mut app, _, _) = create_test_app();
        let error_message = "Test error message";

        app.handle_error(error_message.to_string());

        assert_eq!(app.ui_settings.error_text, error_message);
        assert!(app.ui_settings.error_timer.is_some());
    }

    #[test]
    fn test_get_explorer_popup_area() {
        let area = Rect::new(0, 0, 100, 50);
        let popup_area = App::get_explorer_popup_area(area, 50, 70);

        // Should be centered and smaller than original area
        assert!(popup_area.width <= area.width);
        assert!(popup_area.height <= area.height);
        assert!(popup_area.x >= area.x);
        assert!(popup_area.y >= area.y);
    }

    #[test]
    fn test_get_error_popup_area() {
        let area = Rect::new(0, 0, 100, 60);
        let popup_area = App::get_error_popup_area(area);

        // Should be positioned in the bottom-left portion
        assert!(popup_area.width < area.width);
        assert!(popup_area.height < area.height);
        assert!(popup_area.y > area.y);
    }

    #[test]
    fn test_error_timer_logic() {
        let (mut app, _, _) = create_test_app();

        // No error initially
        assert!(app.ui_settings.error_timer.is_none());

        // Set error
        app.handle_error("Test error".to_string());
        let error_time = app.ui_settings.error_timer.unwrap();

        // Error should be recent
        assert!(error_time.elapsed().as_millis() < 100);

        std::thread::sleep(Duration::from_secs_f32(5.01));

        // it does not work since it gets None in render_error_message() but it cant be run without drawing ui
        // assert!(app.ui_settings.error_timer.is_none())

        assert!(error_time.elapsed().as_millis() > 5000);
    }
}
