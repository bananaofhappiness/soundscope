use color_eyre::Result;
use crossbeam::channel::{Receiver, Sender};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{Event, KeyCode, poll, read},
    layout::Flex,
    prelude::*,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Clear, Dataset, GraphType, Paragraph},
};
use ratatui_explorer::{FileExplorer, Theme};
use rodio::Source;
use std::time::{Duration, Instant};

use crate::{
    analyzer::Analyzer,
    audio_player::{AudioFile, PlayerCommand},
};

const STYLE: Style = Style::new().bg(Color::Black).fg(Color::Yellow);
const TEXT_HIGHLIGHT: Style = Style::new()
    .bg(Color::Black)
    .fg(Color::LightRed)
    .add_modifier(Modifier::BOLD);

/// Settings like showing/hiding UI elements.
struct UISettings {
    show_explorer: bool,
    show_fft_chart: bool,
    show_mid_fft: bool,
    show_side_fft: bool,
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
            show_lufs: false,
            error_text: String::new(),
            error_timer: None,
        }
    }
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
    audio_file_rx: Receiver<AudioFile>,
    /// Sends commands like pause and play to the player.
    player_command_tx: Sender<PlayerCommand>,
    /// Gets playback position for an analyzer to know what samples to analyze.
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
    ) -> Self {
        Self {
            audio_file,
            audio_file_rx,
            player_command_tx,
            playback_position_rx,
            error_rx,
            analyzer: Analyzer::default(),
            fft_data: FFTData::default(),
            waveform: WaveForm::default(),
            lufs: [-50.; 300],
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
        self.render_error_message(f, &self.ui_settings.error_text);

        // render explorer
        if self.ui_settings.show_explorer {
            let area = Self::get_explorer_popup_area(area, 50, 70);
            f.render_widget(Clear, area);
            f.render_widget(&self.explorer.widget(), area);
        }
    }

    fn render_waveform(&mut self, frame: &mut Frame, area: Rect) {
        // playhead is just a function that looks like a vertical line
        let mut playhead_chart = [
            (self.waveform.playhead as f64 / 44., 1.),
            (self.waveform.playhead as f64 / 44. + 0.01, -1.),
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
            // the conversion from samples to chart units (milliseconds) uses the same 1/44. scale
            // as other playhead positions in this function.
            // TODO: change 44 to sample_rate/1000 as u32
            let mut chart_x_position =
                (chart_middle_seconds * 1000.) + (relative_samples_in_scroll_phase as f64 / 44.);

            // Ensure the playhead does not exceed the chart's upper bound.
            chart_x_position = f64::min(chart_x_position, chart_duration_seconds * 1000.);

            playhead_chart = [(chart_x_position, 1.), (chart_x_position + 0.01, -1.)];
        } else if !self.waveform.at_zero {
            // if not at zero then place the playhead right at the middle of a chart
            playhead_chart = [
                (
                    f64::min(self.waveform.playhead as f64 / 44., 1000. * 7.5),
                    1.,
                ),
                (
                    f64::min(self.waveform.playhead as f64 / 44., 1000. * 7.5) + 0.01,
                    -1.,
                ),
            ];
        }

        // get current playback time in seconds
        let playhead_position_in_milis =
            Duration::from_millis((self.waveform.playhead as f64 / 44100. * 1000.) as u64);
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
                .style(Style::default().fg(Color::LightMagenta))
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
            "Short term LUFS:".bold() + format!("{:.2}", self.lufs[299]).into(),
            "Integrated LUFS:".bold() + format!("{:.2}", integrated_lufs).into(),
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

    /// The main loop
    fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            // receive audio file
            if let Ok(af) = self.audio_file_rx.try_recv() {
                self.audio_file = af;
            }

            // receive playback position
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

                    self.fft_data.mid_fft = self.analyzer.get_fft(mid_samples);
                    self.fft_data.side_fft = self.analyzer.get_fft(side_samples);
                }

                //get waveform
                let mid_samples_len = self.audio_file.mid_samples().len();
                self.waveform.playhead = pos;
                // if at zero load first 15 seconds to show
                if self.waveform.at_zero {
                    let waveform_samples = &self.audio_file.mid_samples()[0..15 * sr];
                    self.waveform.chart = Analyzer::get_waveform(waveform_samples);
                }
                let waveform_left_bound = pos.saturating_sub((7.5 * sr as f64) as usize);
                let waveform_right_bound =
                    usize::min(pos + (7.5 * 44100.) as usize, mid_samples_len);

                // if at end load last 15 seconds and dont scroll
                if waveform_right_bound == mid_samples_len {
                    self.waveform.at_end = true;
                    let waveform_samples =
                        &self.audio_file.mid_samples()[mid_samples_len - 15 * sr..mid_samples_len];
                    self.waveform.chart = Analyzer::get_waveform(waveform_samples);
                // if not at the beginning load 15 seconds and scroll
                } else if waveform_left_bound != 0 {
                    self.waveform.at_zero = false;
                    let waveform_samples =
                        &self.audio_file.mid_samples()[waveform_left_bound..waveform_right_bound];
                    self.waveform.chart = Analyzer::get_waveform(waveform_samples);
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
                    self.analyzer
                        .add_samples(&self.audio_file.samples()[lufs_left_bound..pos]);
                    self.lufs[299] = match self.analyzer.get_shortterm_lufs() {
                        Ok(lufs) => lufs,
                        Err(err) => {
                            self.handle_error(format!("Error getting short-term LUFS: {}", err));
                            0.0
                        }
                    };
                }
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
                    match key.code {
                        // quit
                        KeyCode::Char('q') => {
                            self.player_command_tx.send(PlayerCommand::Quit)?;
                            return Ok(());
                        }
                        // show explorer
                        KeyCode::Char('e') => {
                            self.ui_settings.show_explorer = !self.ui_settings.show_explorer
                        }
                        // select file
                        KeyCode::Enter => self.select_file(),
                        // show side fft
                        KeyCode::Char('s') => {
                            self.ui_settings.show_side_fft = !self.ui_settings.show_side_fft
                        }
                        // show mid fft
                        KeyCode::Char('m') => {
                            self.ui_settings.show_mid_fft = !self.ui_settings.show_mid_fft
                        }
                        // pause/play
                        KeyCode::Char(' ') => {
                            if let Err(err) =
                                self.player_command_tx.send(PlayerCommand::ChangeState)
                            {
                                //do smth idk
                            }
                        }
                        // move playhead right and left
                        KeyCode::Right => {
                            if let Err(err) = self.player_command_tx.send(PlayerCommand::MoveRight)
                            {
                                //do smth idk
                            }
                        }
                        KeyCode::Left => {
                            if let Err(err) = self.player_command_tx.send(PlayerCommand::MoveLeft) {
                                //do smth idk
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
                        _ => (),
                    }
                }
                if self.ui_settings.show_explorer {
                    self.explorer.handle(&event)?;
                }
            }
            terminal.draw(|f| self.draw(f))?;
        }
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
        self.ui_settings.show_explorer = false;
        self.fft_data.mid_fft.clear();
        self.fft_data.side_fft.clear();
        self.waveform.chart.clear();
        self.waveform.at_zero = true;
        self.waveform.at_end = false;
        self.lufs = [-50.; 300];

        if let Err(err) = self
            .player_command_tx
            .send(PlayerCommand::SelectFile(file_path))
        {
            //TODO: log sending error
        }

        // TODO: channels
        if let Err(err) = self.analyzer.new(
            // self.audio_file.channels() as u32,
            2,
            self.audio_file.sample_rate(),
        ) {
            self.handle_error(format!(
                "Could not create an analyzer for an audio file: {}",
                err.to_string()
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
        let area = horizontal.areas::<2>(area)[0];
        area
    }

    fn render_error_message(&self, f: &mut Frame, message: &str) {
        // show error for 5 seconds
        match self.ui_settings.error_timer {
            Some(error_timer) => {
                if error_timer.elapsed().as_millis() > 5000 {
                    return;
                }
            }
            None => return,
        }
        let error_popup_area = Self::get_error_popup_area(f.area());
        f.render_widget(Clear, error_popup_area);
        f.render_widget(
            Paragraph::new(message)
                .block(Block::bordered().style(STYLE).fg(Color::LightRed).bold()),
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
) -> Result<()> {
    let terminal = ratatui::init();
    let theme = Theme::default().with_style(STYLE).with_item_style(STYLE);
    let file_explorer = FileExplorer::with_theme(theme)?;
    let app_result = App::new(
        audio_file,
        player_command_tx,
        audio_file_rx,
        playback_position_rx,
        error_rx,
        file_explorer,
    )
    .run(terminal);
    ratatui::restore();
    app_result
}
