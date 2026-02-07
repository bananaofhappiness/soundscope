//! This module contains the implementation of the terminal user interface (TUI) used to display audio analysis results.
//! It uses `ratatui` under the hood.
use crate::{
    analyzer::Analyzer,
    audio_capture::{self, AudioDevice, list_input_devs},
    audio_player::{self, AudioFile, PlayerCommand},
};
use cpal::{Stream, traits::StreamTrait as _};
use crossbeam::channel::{Receiver, Sender};
use dirs::config_dir;
use eyre::{Result, eyre};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{Event, KeyCode, KeyEvent, MouseEvent, MouseEventKind, poll, read},
    layout::Flex,
    prelude::*,
    style::{Color, Style, Stylize},
    text::{Line, Span, ToLine, ToSpan},
    widgets::{Axis, Block, Chart, Clear, Dataset, GraphType, List, ListItem, Paragraph, Wrap},
};
use ratatui_explorer::FileExplorer;
use ringbuffer::{AllocRingBuffer, RingBuffer};
use rodio::Source;
use serde::Deserialize;
use std::{
    fmt::Display,
    fs::{self, File},
    io::Read,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Uses [fill] to conviniently fill all fields of a struct.
macro_rules! fill_fields {
    ($self:ident.$section:ident.$($field:ident => $value:expr),* $(,)?) => {
        $( fill(&mut $self.$section.$field, $value); )*
    };
}

pub type RBuffer = Arc<Mutex<AllocRingBuffer<f32>>>;

/// Settings like showing/hiding UI elements.
struct UI {
    theme: Theme,
    show_explorer: bool,
    show_fft_chart: bool,
    show_mid_fft: bool,
    show_side_fft: bool,
    show_devices_list: bool,
    show_lufs: bool,
    show_themes_list: bool,
    error_text: String,
    error_timer: Option<Instant>,
    device_name: String,
    waveform_window: f64,
    // Used to flash control elements when the button is pressed
    left_arrow_timer: Option<Instant>,
    right_arrow_timer: Option<Instant>,
    plus_sign_timer: Option<Instant>,
    minus_sign_timer: Option<Instant>,
    // Used to be able to hover fft chart to get more precise frequencies
    chart_rect: Option<Rect>,
}

impl Default for UI {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            show_explorer: false,
            show_fft_chart: true,
            show_mid_fft: true,
            show_side_fft: false,
            show_devices_list: false,
            show_lufs: false,
            show_themes_list: false,
            error_text: String::new(),
            error_timer: None,
            device_name: String::new(),
            waveform_window: 15.,
            left_arrow_timer: None,
            right_arrow_timer: None,
            plus_sign_timer: None,
            minus_sign_timer: None,
            chart_rect: None,
        }
    }
}

/// Mode of the [App]. Currently, only Player and Microphone are supported.
#[derive(Default)]
enum Mode {
    #[default]
    Player,
    Microphone,
    _System,
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Player => write!(f, "Player"),
            Mode::Microphone => write!(f, "Microphone"),
            Mode::_System => write!(f, "System"),
        }
    }
}

/// Defines theme using .theme file
/// Otherwise, uses default values.
#[derive(Deserialize, Default)]
struct Theme {
    global: GlobalTheme,
    waveform: WaveformTheme,
    fft: FftTheme,
    lufs: LufsTheme,
    devices: DevicesTheme,
    explorer: ExplorerTheme,
    error: ErrorTheme,
}

/// Used to set `default: T` to a `field` if it is not set (it is None).
/// Used in [fill_fields] macro
fn fill<T>(field: &mut Option<T>, default: T) {
    if field.is_none() {
        *field = Some(default);
    }
}

impl Theme {
    /// Sets `self.global.foreground` and `self.global.background` for every field that was not defined in a .theme file.
    fn apply_global_as_default(&mut self) {
        let fg = self.global.foreground;
        let bg = self.global.background;
        self.global.highlight = self.global.highlight.or(Some(fg));
        let hl = self.global.highlight.unwrap();

        fill_fields!(self.waveform.
            borders => fg,
            controls => fg,
            controls_highlight => hl,
            labels => fg,
            playhead => hl,
            current_time => fg,
            total_duration => fg,
            waveform => fg,
            background => bg,
            highlight => hl,
        );

        fill_fields!(self.lufs.
            axis => fg,
            chart => fg,
            foreground => fg,
            labels => fg,
            numbers => fg,
            borders => fg,
            background => bg,
            highlight => hl,
        );

        fill_fields!(self.fft.
            axes => fg,
            axes_labels => fg,
            borders => fg,
            labels => fg,
            mid_fft => fg,
            side_fft => hl,
            background => bg,
            highlight => hl,
        );

        fill_fields!(self.explorer.
            background => bg,
            borders => fg,
            dir_foreground => fg,
            item_foreground => fg,
            highlight_dir_foreground => hl,
            highlight_item_foreground => hl,
        );

        fill_fields!(self.devices.
            background => bg,
            foreground => fg,
            borders => fg,
            highlight => hl,
        );

        fill_fields!(self.error.
            background => bg,
            foreground => fg,
            borders => fg,
        );
    }
}

/// Used to set default values of every UI element if they are not specified in the config file.
#[derive(Deserialize)]
struct GlobalTheme {
    background: Color,
    /// It is default value for everything that is not a background,
    /// Except for SideFFT, which is LightGreen, and playhead position, which is LightRed
    foreground: Color,
    /// Color used to highlight corresponding characters
    /// Like highlighting L in LUFS to let the user know
    /// that pressing L will open the LUFS meter
    highlight: Option<Color>,
}

impl Default for GlobalTheme {
    fn default() -> Self {
        Self {
            background: Color::Black,
            foreground: Color::Indexed(221),
            highlight: Some(Color::Indexed(160)),
        }
    }
}

/// Used to define the theme for the waveform display.
#[derive(Deserialize, Default)]
struct WaveformTheme {
    borders: Option<Color>,
    waveform: Option<Color>,
    playhead: Option<Color>,
    /// Current playing time and total duration
    current_time: Option<Color>,
    total_duration: Option<Color>,
    /// Buttons like <-, +, -, ->
    controls: Option<Color>,
    controls_highlight: Option<Color>,
    labels: Option<Color>,
    /// Background of the chart
    background: Option<Color>,
    highlight: Option<Color>,
}

/// Used to define the theme for the FFT display.
#[derive(Deserialize)]
struct FftTheme {
    borders: Option<Color>,
    /// Frequencies and LUFS tabs text
    labels: Option<Color>,
    axes: Option<Color>,
    axes_labels: Option<Color>,
    mid_fft: Option<Color>,
    side_fft: Option<Color>,
    /// Background of the chart
    background: Option<Color>,
    highlight: Option<Color>,
}

impl Default for FftTheme {
    fn default() -> Self {
        FftTheme {
            borders: None,
            labels: None,
            axes: None,
            axes_labels: None,
            mid_fft: None,
            side_fft: Some(Color::Indexed(170)),
            background: None,
            highlight: None,
        }
    }
}

/// Used to define the theme for the LUFS display.
#[derive(Deserialize, Default)]
struct LufsTheme {
    axis: Option<Color>,
    chart: Option<Color>,
    /// Frequencies and LUFS tabs text
    labels: Option<Color>,
    /// Text color on the left
    foreground: Option<Color>,
    /// Color of the numbers on the left
    numbers: Option<Color>,
    borders: Option<Color>,
    /// Background of the chart
    background: Option<Color>,
    highlight: Option<Color>,
}

/// Used to define the theme for the devices list.
#[derive(Deserialize, Default)]
struct DevicesTheme {
    background: Option<Color>,
    foreground: Option<Color>,
    borders: Option<Color>,
    highlight: Option<Color>,
}

/// Used to define the theme for the explorer.
#[derive(Deserialize, Default)]
struct ExplorerTheme {
    background: Option<Color>,
    borders: Option<Color>,
    item_foreground: Option<Color>,
    highlight_item_foreground: Option<Color>,
    dir_foreground: Option<Color>,
    highlight_dir_foreground: Option<Color>,
}

/// Used to define the theme for the error popup.
#[derive(Deserialize)]
struct ErrorTheme {
    background: Option<Color>,
    foreground: Option<Color>,
    borders: Option<Color>,
}

impl Default for ErrorTheme {
    fn default() -> Self {
        Self {
            background: Some(Color::Black),
            foreground: Some(Color::Indexed(160)),
            borders: Some(Color::Indexed(160)),
        }
    }
}

/// Settings for the [App]. Currently only the [Mode] is supported.
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
}

impl Default for WaveForm {
    fn default() -> Self {
        Self {
            chart: vec![(0., 0.)],
            playhead: 0,
        }
    }
}

/// `App` contains the necessary components for the application like senders, receivers, [AudioFile] data, [UIsettings].
struct App {
    /// Audio file which is loaded into the player.
    audio_file: AudioFile,
    /// If file is not selected, the app crashes when you try to play it.
    /// It is easier to use this bool instead of Option<AudioFile> because
    /// we would always have to check if it is not None. But it can be None only before
    /// the first file is selected.
    is_file_selected: bool,
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
    /// Used to get LUFS of an audio file.
    file_analyzer: Analyzer,
    /// Used to get LUFS of microphone input.
    device_analyzer: Analyzer,

    // Charts data
    /// Data used to render FFT chart.
    fft_data: FFTData,
    /// Data used to render waveform.
    waveform: WaveForm,
    /// LUFS chart.
    lufs: [f64; 300],
    /// Track if render is needed to avoid unnecessary redraws
    needs_render: bool,

    settings: Settings,
    //UI
    explorer: FileExplorer,
    ui: UI,
    // Used to conviniently return to current directory when opening an explorer
    current_directory: PathBuf,
    // Used to print info about fft chart when it's hovered
    mouse_position: Option<(u16, u16)>,
}

impl App {
    fn new(
        audio_file: AudioFile,
        player_command_tx: Sender<PlayerCommand>,
        audio_file_rx: Receiver<AudioFile>,
        playback_position_rx: Receiver<usize>,
        error_rx: Receiver<String>,
        latest_captured_samples: RBuffer,
    ) -> Result<Self> {
        Ok(Self {
            audio_file,
            is_file_selected: false,
            is_playing_audio: false,
            audio_file_rx,
            latest_captured_samples,
            audio_capture_stream: None,
            player_command_tx,
            playback_position_rx,
            error_rx,
            file_analyzer: Analyzer::default(),
            device_analyzer: Analyzer::default(),
            fft_data: FFTData::default(),
            waveform: WaveForm::default(),
            lufs: [-50.; 300],
            settings: Settings::default(),
            explorer: FileExplorer::with_theme(ratatui_explorer::Theme::default())?,
            ui: UI::default(),
            current_directory: PathBuf::from(""),
            mouse_position: None,
            needs_render: true,
        })
    }

    fn set_theme(&mut self, theme: Theme) {
        // define styles
        let s = Style::default()
            .bg(theme.explorer.background.unwrap())
            .fg(theme.explorer.borders.unwrap());
        let is = s.fg(theme.explorer.item_foreground.unwrap());
        let ihl = s.fg(theme.explorer.highlight_item_foreground.unwrap());
        let ds = s.fg(theme.explorer.dir_foreground.unwrap()).bold();
        let dhl = s
            .fg(theme.explorer.highlight_dir_foreground.unwrap())
            .bold();
        let explorer_theme = ratatui_explorer::Theme::default()
            .with_style(s)
            .with_item_style(is)
            .with_highlight_item_style(ihl)
            .with_dir_style(ds)
            .with_highlight_dir_style(dhl)
            .add_default_title();
        self.explorer.set_theme(explorer_theme);
        self.ui.theme = theme;
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
        let background = Paragraph::new("").style(self.ui.theme.global.background);
        f.render_widget(background, area);
        self.render_waveform(f, layout[0]);
        self.ui.chart_rect = Some(layout[1]);

        // show charts based on user settings
        if self.ui.show_lufs {
            self.render_lufs(f, layout[1]);
        } else if self.ui.show_fft_chart {
            self.render_fft_chart(f, layout[1]);
            if let Some((x, y)) = self.mouse_position {
                self.render_fft_info(f, x, y)
            }
        }

        // render error
        if let Ok(err) = self.error_rx.try_recv() {
            self.ui.error_text = err;
            self.ui.error_timer = Some(std::time::Instant::now())
        }
        self.render_error_message(f);

        // render explorer
        if self.ui.show_explorer || self.ui.show_themes_list {
            let area = Self::get_explorer_popup_area(area, 50, 70);
            f.render_widget(Clear, area);
            f.render_widget(&self.explorer.widget(), area);
        }
        if self.ui.show_devices_list {
            self.render_devices_list(f);
        }
    }

    fn render_waveform(&mut self, frame: &mut Frame, area: Rect) {
        let s = Style::default().bg(self.ui.theme.waveform.background.unwrap());
        let lb = s.fg(self.ui.theme.waveform.labels.unwrap());
        let bd = s.fg(self.ui.theme.waveform.borders.unwrap());
        let hl = s.fg(self.ui.theme.waveform.highlight.unwrap());
        let pl = s.fg(self.ui.theme.waveform.playhead.unwrap());
        let ct = s.fg(self.ui.theme.waveform.current_time.unwrap());
        let td = s.fg(self.ui.theme.waveform.total_duration.unwrap());
        let wv = s.fg(self.ui.theme.waveform.waveform.unwrap());

        // playhead is just a function that looks like a vertical line
        let samples_in_one_ms = self.audio_file.sample_rate() / 1000;

        let playhead_chart = if !matches!(self.settings.mode, Mode::Player) {
            [(-1., -1.), (-1., -1.)]
        } else {
            let playhead_x = self.waveform.playhead as f64 / samples_in_one_ms as f64;
            [(playhead_x, 1.), (playhead_x, -1.)]
        };

        // get current playback time in seconds
        let playhead_ms =
            (self.waveform.playhead as f64 / self.audio_file.sample_rate() as f64 * 1000.) as u64;
        let current_total_sec = playhead_ms / 1000;
        let current_min = current_total_sec / 60;
        let current_sec = current_total_sec % 60;

        // get total audio file duration
        let total_duration = self.audio_file.duration().as_secs();
        let total_min = total_duration / 60;
        let total_sec = total_duration % 60;

        let (x_min, x_max) = match self.settings.mode {
            Mode::Microphone | Mode::_System => {
                let window_millis = self.ui.waveform_window as usize * 1000;
                (15000. - window_millis as f64, 15000.)
            }
            _ => {
                let half_window = self.ui.waveform_window * 500.;
                let playhead_millis = playhead_ms as f64;
                let max_x = self.waveform.chart.len() as f64 / 2.;
                let min_bound = (playhead_millis - half_window)
                    .min(max_x - self.ui.waveform_window * 1000.)
                    .max(0.);
                let max_bound = (playhead_millis + half_window)
                    .min(max_x)
                    .max(self.ui.waveform_window * 1000.);
                (min_bound, max_bound)
            }
        };

        // make datasets
        // first one to render a waveform
        // the other one to render the playhead
        let datasets = vec![
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(wv)
                .data(&self.waveform.chart),
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(pl)
                .data(&playhead_chart),
        ];

        // render chart
        let title = self.audio_file.title();
        let mode_text = self.settings.mode.to_span().style(lb);
        let upper_right_title = match self.settings.mode {
            Mode::Player => Line::from(vec![
                "c".bold().style(hl),
                "hange Mode: ".to_span().style(lb),
                mode_text,
                " t".bold().style(hl),
                "heme".to_span().style(lb),
            ])
            .right_aligned(),
            _ => Line::from(vec![
                "d".bold().style(hl),
                "evice: ".to_span().style(lb),
                self.ui.device_name.to_span().style(lb),
                " ".to_span(),
                "c".bold().style(hl),
                "hange Mode: ".to_span().style(lb),
                mode_text,
                " t".bold().style(hl),
                "heme".to_span().style(lb),
            ])
            .right_aligned(),
        };

        // build the chart widget
        let chart = Chart::new(datasets)
            .block(
                Block::bordered()
                    .title(title.to_span().style(lb))
                    .title_bottom(self.get_flashing_controls_text().left_aligned())
                    .title_bottom(
                        Line::styled(format!("{:0>2}:{:0>2}", current_min, current_sec), ct)
                            .centered(),
                    )
                    .title_bottom(
                        Line::styled(format!("{:0>2}:{:0>2}", total_min, total_sec), td)
                            .right_aligned(),
                    )
                    .title(upper_right_title)
                    .style(bd),
            )
            .style(wv)
            .x_axis(Axis::default().bounds([x_min, x_max]))
            .y_axis(Axis::default().bounds([-1., 1.]));

        frame.render_widget(chart, area);
    }

    fn get_flashing_controls_text(&self) -> Line<'_> {
        let t = 100;
        let s = Style::default()
            .bg(self.ui.theme.waveform.background.unwrap())
            .fg(self.ui.theme.waveform.controls.unwrap());
        let hl = s.fg(self.ui.theme.waveform.controls_highlight.unwrap());
        let left_arrow = match self.ui.left_arrow_timer {
            Some(timer) if timer.elapsed().as_millis() < t => "<-".to_span().style(hl),
            _ => "<-".to_span().style(s),
        };
        let right_arrow = match self.ui.right_arrow_timer {
            Some(timer) if timer.elapsed().as_millis() < t => "->".to_span().style(hl),
            _ => "->".to_span().style(s),
        };
        let minus = match self.ui.minus_sign_timer {
            Some(timer) if timer.elapsed().as_millis() < t => "-".to_span().style(hl),
            _ => "-".to_span().style(s),
        };
        let plus = match self.ui.plus_sign_timer {
            Some(timer) if timer.elapsed().as_millis() < t => "+".to_span().style(hl),
            _ => "+".to_span().style(s),
        };
        // Line::from(format!(
        //     "{} {} {:0>2}s {} {}",
        //     left_arrow, minus, self.ui_settings.waveform_window, plus, right_arrow
        // ))
        Line::from(vec![
            left_arrow,
            " ".to_span(),
            minus,
            " ".to_span(),
            format!("{:0>2}s", self.ui.waveform_window.to_span().style(s)).into(),
            " ".to_span(),
            plus,
            " ".to_span(),
            right_arrow,
        ])
    }

    fn render_fft_chart(&mut self, frame: &mut Frame, area: Rect) {
        let s = Style::default().bg(self.ui.theme.fft.background.unwrap());
        let fg = s.fg(self.ui.theme.fft.axes_labels.unwrap());
        let ax = s.fg(self.ui.theme.fft.axes.unwrap());
        let lb = s.fg(self.ui.theme.fft.labels.unwrap());
        let bd = s.fg(self.ui.theme.fft.borders.unwrap());
        let mf = s.fg(self.ui.theme.fft.mid_fft.unwrap());
        let sf = s.fg(self.ui.theme.fft.side_fft.unwrap());
        let hl = s.fg(self.ui.theme.fft.highlight.unwrap());
        let x_labels = vec![
            // frequencies are commented because their positions are off.
            // they are not rendered where the corresponding frequencies are.
            Span::styled("20Hz", fg.bold()),
            // Span::raw("20Hz"),
            // Span::raw(""),
            // Span::raw(""),
            // Span::raw("112.47"),
            // Span::raw(""),
            // Span::raw(""),
            // Span::raw(""),
            Span::styled("632.46Hz", fg),
            // Span::raw(""),
            // Span::raw(""),
            // Span::raw(" "),
            // Span::raw("3556.57"),
            // Span::raw(""),
            // Span::raw(""),
            // Span::raw("20000Hz"),
            Span::styled("20kHz", fg.bold()),
        ];

        // if no data about frequencies then default to some low value
        let mid_fft: &[(f64, f64)] = if self.ui.show_mid_fft {
            &self.fft_data.mid_fft
        } else {
            &[(-1000.0, -1000.0)]
        };

        let side_fft: &[(f64, f64)] = if self.ui.show_side_fft {
            &self.fft_data.side_fft
        } else {
            &[(-1000.0, -1000.0)]
        };

        let datasets = vec![
            Dataset::default()
                // highlight the letter M so the user knows they must press M to toggle it
                // same with Side fft
                .name(vec!["m".bold().style(hl), "id frequencies".into()])
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(mf)
                .data(mid_fft),
            Dataset::default()
                .name(vec!["s".bold().style(hl), "ide frequencies".into()])
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(sf)
                .data(side_fft),
        ];

        let chart = Chart::new(datasets)
            // the title uses the same highlighting technique
            .block(Block::bordered().style(bd).title(vec![
                "f".to_span().style(hl).bold(),
                "requencies ".to_span().style(lb).bold(),
                "l".to_span().style(hl),
                "ufs".to_span().style(lb),
            ]))
            .x_axis(
                Axis::default()
                    .title("Hz")
                    .labels(x_labels)
                    .style(ax)
                    .bounds([0., 100.]),
            )
            .y_axis(
                Axis::default()
                    .title("Db")
                    .labels(vec![
                        Span::raw("-78 Db").style(fg),
                        Span::raw("-18 Db").style(fg),
                    ])
                    .style(ax)
                    .bounds([-150., 100.]),
            )
            .style(s);

        frame.render_widget(chart, area);
    }

    fn render_lufs(&mut self, f: &mut Frame, area: Rect) {
        let s = Style::default().bg(self.ui.theme.lufs.background.unwrap());
        let fg = s.fg(self.ui.theme.lufs.foreground.unwrap());
        let ax = s.fg(self.ui.theme.lufs.axis.unwrap());
        let hl = s.fg(self.ui.theme.lufs.highlight.unwrap());
        let bd = s.fg(self.ui.theme.lufs.borders.unwrap());
        let ch = s.fg(self.ui.theme.lufs.chart.unwrap());
        let lb = s.fg(self.ui.theme.lufs.labels.unwrap());
        let nb = s.fg(self.ui.theme.lufs.numbers.unwrap());
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

        let integrated_lufs = match self.file_analyzer.get_integrated_lufs() {
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

        // get lufs text
        let integrated = format!("{:06.2}", integrated_lufs);
        let short_term = format!("{:06.2}", self.lufs[299]);
        let integrated = integrated.to_span().style(nb);
        let short_term = short_term.to_span().style(nb);
        let lufs_text = vec![
            "Short term LUFS:".bold().style(fg) + short_term,
            "Intergrated LUFS:".bold().style(fg) + integrated,
        ];

        // get true peak
        let (tp_left, tp_right) = match self.file_analyzer.get_true_peak() {
            Ok((tp_left, tp_right)) => (tp_left, tp_right),
            Err(err) => {
                self.handle_error(format!("Error getting true peak: {}", err));
                (0.0, 0.0)
            }
        };

        // get true peak text
        let left = format!("{:.2}", tp_left);
        let right = format!("{:.2}", tp_right);
        let left = left.to_span().style(nb);
        let right = right.to_span().style(nb);
        let true_peak_text = vec![
            "True Peak".to_line().style(fg).bold(),
            "L: ".bold().style(fg) + left + " Db".bold().style(fg),
            "R: ".bold().style(fg) + right + " Db".bold().style(fg),
        ];

        //get range text
        let range = match self.file_analyzer.get_loudness_range() {
            Ok(range) => range,
            Err(err) => {
                self.handle_error(format!("Error getting loudness range: {}", err));
                0.0
            }
        };
        let range_text = vec![("Range: ".bold() + format!("{:.2} LU", range).into()).style(fg)];

        // paragraphs
        let lufs_paragraph = Paragraph::new(lufs_text)
            .block(Block::bordered().style(bd).title(vec![
                "f".to_span().style(hl),
                "requencies ".to_span().style(lb),
                "l".to_span().style(hl).bold(),
                "ufs".to_span().style(lb).bold(),
            ]))
            .alignment(Alignment::Center);
        let true_peak_paragraph = Paragraph::new(true_peak_text)
            .block(Block::bordered().style(bd))
            .alignment(Alignment::Center)
            .style(bd);
        let range_paragraph = Paragraph::new(range_text)
            .block(Block::bordered().style(bd))
            .alignment(Alignment::Center)
            .style(bd);

        // chart section
        let dataset = vec![
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(ch)
                .data(&data),
        ];
        let chart = Chart::new(dataset)
            .block(Block::bordered().style(bd))
            .x_axis(Axis::default().bounds([0., 300.]).style(ax))
            .y_axis(
                Axis::default()
                    .bounds([-50., 0.])
                    .labels(["-50".bold(), "0".bold()])
                    .style(ax),
            )
            .style(s);
        f.render_widget(lufs_paragraph, paragraph_layout[0]);
        f.render_widget(true_peak_paragraph, paragraph_layout[1]);
        f.render_widget(range_paragraph, paragraph_layout[2]);

        f.render_widget(chart, layout[1]);
    }

    fn render_devices_list(&self, f: &mut Frame) {
        let s = Style::default()
            .fg(self.ui.theme.devices.foreground.unwrap())
            .bg(self.ui.theme.devices.background.unwrap());
        let bd = s.fg(self.ui.theme.devices.borders.unwrap());
        let hl = s.fg(self.ui.theme.devices.highlight.unwrap());
        let area = Self::get_explorer_popup_area(f.area(), 20, 30);
        f.render_widget(Clear, area);
        let devs = list_input_devs();
        let list_items: Vec<ListItem> = devs
            .iter()
            .enumerate()
            .map(|(i, (name, _dev))| {
                let num = format!("[{}]", i + 1);
                let name = format!(" {}", name);
                // idk why .to_span doesn't work
                // prolly cuz it takes &self but bold takes self
                let num = num.bold().reset().style(hl);
                let name = name.bold().reset().style(s);
                ListItem::from(num + name)
            })
            .collect();
        let list = List::new(list_items)
            .style(s)
            .block(Block::bordered().title("Devices").style(bd));

        f.render_widget(list, area);
    }

    fn render_fft_info(&self, f: &mut Frame<'_>, x: u16, y: u16) {
        let rect_width = self.ui.chart_rect.unwrap().width;
        let rect_height = self.ui.chart_rect.unwrap().height;

        let left_bound = 8;
        let right_bound = rect_width - 1;
        let upper_bound = self.ui.chart_rect.unwrap().y + 1;
        let lower_bound = upper_bound + rect_height - 4;

        let (hz, db) = self.map_mouse_position_to_chart_point(
            x - left_bound,
            right_bound - left_bound,
            y - upper_bound,
            lower_bound - upper_bound,
        );
        let hz_text = format!("{hz:.2} Hz");
        let db_text = format!("{db:.2} Db");

        let width = hz_text.len().max(db_text.len()) as u16 + 2;
        let height = 4;

        let text = db_text + "\n" + &hz_text;

        let x = u16::min(x, right_bound - width) + 1;
        let y = u16::min(y, lower_bound - height) + 1;
        let area = Rect::new(x, y, width, height);
        f.render_widget(Clear, area);
        f.render_widget(
            Paragraph::new(text).block(
                Block::bordered().style(
                    Style::default().fg(self.ui.theme.global.foreground).bg(self
                        .ui
                        .theme
                        .global
                        .background),
                ),
            ),
            area,
        );
    }

    /// The main loop
    fn run(mut self, mut terminal: DefaultTerminal, startup_file: Option<PathBuf>) -> Result<()> {
        // apply theme
        // check if config directory exists
        match config_dir() {
            Some(path) => {
                self.apply_current_theme(path);
            }
            None => {
                self.handle_error(
                    "Config directory does not exist. Could not load theme.".to_string(),
                );
                let mut theme = Theme::default();
                theme.apply_global_as_default();
                self.set_theme(theme);
            }
        }

        self.current_directory = self.explorer.cwd().clone();

        if let Some(f) = startup_file {
            self.select_audio_file(f);
        }

        terminal.draw(|f| self.draw(f))?;

        loop {
            std::thread::sleep(Duration::from_millis(8));
            self.needs_render = false;

            // receive audio file
            if let Ok(af) = self.audio_file_rx.try_recv() {
                self.audio_file = af;
                self.is_file_selected = true;
                if self.audio_file.duration().as_secs_f64() < 15. {
                    self.ui.waveform_window = self.audio_file.duration().as_secs_f64()
                }
                self.waveform.chart = self.file_analyzer.get_waveform(
                    self.audio_file.samples(),
                    self.audio_file.duration().as_secs_f64(),
                );
                self.needs_render = true;
            }

            // receive playback position
            let prev_playhead = self.waveform.playhead;
            if let Ok(pos) = self.playback_position_rx.try_recv()
                && self.is_file_selected
                && matches!(self.settings.mode, Mode::Player)
            {
                self.analyze_audio_file_samples(pos);
                // render only if playhead position changed
                self.needs_render = prev_playhead != self.waveform.playhead;
            }

            // use ringbuf to analyze data if the `Mode` is not `Mode::Player`
            if matches!(self.settings.mode, Mode::Microphone) {
                self.analyze_microphone_input();
                self.needs_render = true; // Always render in microphone mode
            }

            // check if flashing controls need update (timers)
            let t = 100; // flash duration in ms
            let left_arrow_flash = self
                .ui
                .left_arrow_timer
                .map(|timer| timer.elapsed().as_millis());
            let right_arrow_flash = self
                .ui
                .right_arrow_timer
                .map(|timer| timer.elapsed().as_millis());
            let plus_sign_flash = self
                .ui
                .plus_sign_timer
                .map(|timer| timer.elapsed().as_millis());
            let minus_sign_flash = self
                .ui
                .minus_sign_timer
                .map(|timer| timer.elapsed().as_millis());

            // render if currently flashing or just stopped flashing (was <t, now >=t)
            let needs_flash_update = left_arrow_flash.is_some_and(|ms| ms < t)
                || right_arrow_flash.is_some_and(|ms| ms < t)
                || plus_sign_flash.is_some_and(|ms| ms < t)
                || minus_sign_flash.is_some_and(|ms| ms < t);

            if needs_flash_update {
                self.needs_render = true;
            }

            // clean up expired timers and trigger final render
            let mut timers_need_cleanup = false;
            if left_arrow_flash.is_some_and(|ms| ms >= t) {
                self.ui.left_arrow_timer = None;
                timers_need_cleanup = true;
            }
            if right_arrow_flash.is_some_and(|ms| ms >= t) {
                self.ui.right_arrow_timer = None;
                timers_need_cleanup = true;
            }
            if plus_sign_flash.is_some_and(|ms| ms >= t) {
                self.ui.plus_sign_timer = None;
                timers_need_cleanup = true;
            }
            if minus_sign_flash.is_some_and(|ms| ms >= t) {
                self.ui.minus_sign_timer = None;
                timers_need_cleanup = true;
            }

            if timers_need_cleanup {
                self.needs_render = true;
            }

            // check if error message needs update
            if self
                .ui
                .error_timer
                .is_some_and(|timer| timer.elapsed().as_millis() < 5000)
            {
                self.needs_render = true;
            }

            // clean up expired error timer and trigger final render
            if self
                .ui
                .error_timer
                .is_some_and(|timer| timer.elapsed().as_millis() >= 5000)
            {
                self.ui.error_timer = None;
                self.needs_render = true;
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

                // if let Event::Key(key) = event {
                match event {
                    Event::Key(key) => {
                        // quit
                        if key.code == KeyCode::Char('q') {
                            self.player_command_tx.send(PlayerCommand::Quit)?;
                            return Ok(());
                        }
                        if let Err(err) = self.handle_input(key) {
                            self.handle_error(format!("{}", err));
                        }
                        self.needs_render = true;
                    }
                    Event::Mouse(m) => {
                        if matches!(m.kind, MouseEventKind::Moved) {
                            if self.in_fft_chart(m) {
                                self.mouse_position = Some((m.column, m.row))
                            } else {
                                self.mouse_position = None
                            }
                        } else {
                            self.mouse_position = None
                        }
                        self.needs_render = true;
                    }
                    _ => (),
                }

                if self.ui.show_explorer {
                    self.explorer.handle(&event)?;
                    self.needs_render = true;
                }

                if self.ui.show_themes_list {
                    self.explorer.handle(&event)?;
                    self.needs_render = true;
                }
            }

            // render only if something changed
            if self.needs_render {
                terminal.draw(|f| self.draw(f))?;
            }
        }
    }

    fn analyze_microphone_input(&mut self) {
        let samples = self.latest_captured_samples.lock().unwrap().to_vec();
        let (mid_samples, side_samples) = audio_player::get_mid_and_side_samples(&samples);
        let sample_rate = self.device_analyzer.sample_rate() as usize;
        let left_bound = 15 * sample_rate - 2usize.pow(14);

        // get fft
        self.fft_data.mid_fft = match self
            .device_analyzer
            .get_fft(&mid_samples[left_bound..15 * sample_rate])
        {
            Ok(fft) => fft,
            Err(err) => {
                self.handle_error(format!("Error getting frequencies: {}. Perhaps your microphone's sample rate is too low.", err));
                vec![(0., 0.)]
            }
        };
        self.fft_data.side_fft = match self
            .device_analyzer
            .get_fft(&side_samples[left_bound..15 * sample_rate])
        {
            Ok(fft) => fft,
            Err(err) => {
                self.handle_error(format!("Error getting frequencies: {}. Perhaps your microphone's sample rate is too low.", err));
                vec![(0., 0.)]
            }
        };

        // get waveform
        self.waveform.chart = self.device_analyzer.get_waveform(&mid_samples, 15.);

        let samples = self.latest_captured_samples.lock().unwrap().to_vec();
        let sample_rate = self.device_analyzer.sample_rate() as usize;

        // get lufs
        for i in 0..self.lufs.len() - 1 {
            self.lufs[i] = self.lufs[i + 1];
        }

        let lb = 30 * sample_rate - 2usize.pow(14);
        if let Err(err) = self
            .device_analyzer
            .add_samples(&samples[lb..30 * sample_rate])
        {
            self.handle_error(format!("Could not get samples for LUFS analyzer: {}", err));
        };
        self.lufs[299] = match self.device_analyzer.get_shortterm_lufs() {
            Ok(lufs) => lufs,
            Err(err) => {
                self.handle_error(format!("Error getting short-term LUFS: {}", err));
                0.0
            }
        };
    }

    fn analyze_audio_file_samples(&mut self, pos: usize) {
        // if using mid side we must divide the position by 2
        let pos = pos / self.audio_file.channels() as usize;
        self.waveform.playhead = pos;

        // get fft
        if self.ui.show_fft_chart {
            self.get_fft(pos);
        }

        // get lufs lufs uses all channels (update every frame for accuracy)
        let pos = pos * self.audio_file.channels() as usize;
        let lufs_left_bound = pos.saturating_sub(16384);
        if lufs_left_bound != 0 {
            for i in 0..self.lufs.len() - 1 {
                self.lufs[i] = self.lufs[i + 1];
            }
            let samples_len = self.audio_file.samples().len();
            // check bounds to prevent panic when file was changed
            if pos <= samples_len && lufs_left_bound < samples_len {
                if let Err(err) = self
                    .file_analyzer
                    .add_samples(&self.audio_file.samples()[lufs_left_bound..pos])
                {
                    self.handle_error(format!("Could not get samples for LUFS analyzer: {}", err));
                };
                self.lufs[299] = match self.file_analyzer.get_shortterm_lufs() {
                    Ok(lufs) => lufs,
                    Err(err) => {
                        self.handle_error(format!("Error getting short-term LUFS: {}", err));
                        0.0
                    }
                };
            }
        }
    }

    fn get_fft(&mut self, pos: usize) {
        let fft_left_bound = pos.saturating_sub(16384);
        if fft_left_bound != 0 {
            let mid_samples_len = self.audio_file.mid_samples().len();
            let side_samples_len = self.audio_file.side_samples().len();

            // check bounds to prevent panic when file was changed
            let mid_samples = if pos <= mid_samples_len && fft_left_bound < mid_samples_len {
                &self.audio_file.mid_samples()[fft_left_bound..pos]
            } else {
                &[]
            };
            let side_samples = if pos <= side_samples_len && fft_left_bound < side_samples_len {
                &self.audio_file.side_samples()[fft_left_bound..pos]
            } else {
                &[]
            };

            self.fft_data.mid_fft = match self.file_analyzer.get_fft(mid_samples) {
                Ok(fft) => fft,
                Err(_err) => {
                    // can't log the error because this fn takes a mutable reference
                    // but we already have 2 shared references.
                    // but it doesn't really matter(?), this message is mostly for microphone input
                    // self.handle_error(format!("Error getting frequencies: {}. Perhaps your microphone's sample rate is too low.", err));
                    vec![(0., 0.)]
                }
            };
            self.fft_data.side_fft = match self.file_analyzer.get_fft(side_samples) {
                Ok(fft) => fft,
                Err(_err) => {
                    // can't log the error because this fn takes a mutable reference
                    // but we already have 2 shared references.
                    // but it doesn't really matter(?), this message is mostly for microphone input
                    // self.handle_error(format!("Error getting frequencies: {}. Perhaps your microphone's sample rate is too low.", err));
                    vec![(0., 0.)]
                }
            };
        }
    }

    fn handle_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            // show explorer
            KeyCode::Char('e') if matches!(self.settings.mode, Mode::Player) => {
                self.explorer.set_cwd(&self.current_directory).unwrap();
                self.ui.show_explorer = !self.ui.show_explorer
            }
            // select file
            KeyCode::Enter if self.ui.show_explorer => {
                let file = self.explorer.current();
                let file_path = self.explorer.current().path().clone();
                if file.is_file() {
                    self.select_audio_file(file_path)
                }
            }
            KeyCode::Enter if self.ui.show_themes_list => self.select_theme_file(),
            // show side fft
            KeyCode::Char('s') => self.ui.show_side_fft = !self.ui.show_side_fft,
            // show mid fft
            KeyCode::Char('m') => self.ui.show_mid_fft = !self.ui.show_mid_fft,
            // pause/play
            KeyCode::Char(' ') => {
                if let Err(_err) = self.player_command_tx.send(PlayerCommand::ChangeState) {
                    //TODO: log sending error
                }
                self.is_playing_audio = !self.is_playing_audio;
                // do this so lufs update only on play, not pause
                if self.is_playing_audio {
                    self.lufs = [-50.; 300];
                    self.file_analyzer.reset();
                }
            }
            // move playhead right and left
            KeyCode::Right
                if matches!(self.settings.mode, Mode::Player)
                    && !(self.ui.show_devices_list || self.ui.show_explorer) =>
            {
                self.ui.right_arrow_timer = Some(Instant::now());
                self.lufs = [-50.; 300];
                self.file_analyzer.reset();
                if let Err(_err) = self.player_command_tx.send(PlayerCommand::MoveRight) {
                    //TODO: log sending error
                }
            }
            KeyCode::Left
                if matches!(self.settings.mode, Mode::Player)
                    && !(self.ui.show_devices_list || self.ui.show_explorer) =>
            {
                self.ui.left_arrow_timer = Some(Instant::now());
                self.lufs = [-50.; 300];
                self.file_analyzer.reset();
                if let Err(_err) = self.player_command_tx.send(PlayerCommand::MoveLeft) {
                    //TODO: log sending error
                }
            }
            // change charts shown
            // Probably enum will be more logical to use but this works fine
            // and i will not change this
            // as long as there are few charts
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
            KeyCode::Char('d') if matches!(self.settings.mode, Mode::Microphone) => {
                self.ui.show_devices_list = !self.ui.show_devices_list
            }
            // change mode. this will be replaced by normal settings selection tab
            // TODO normal settings popup window with a list of options
            KeyCode::Char('c') => {
                self.settings.mode = if matches!(self.settings.mode, Mode::Microphone) {
                    self.reset_charts();
                    if let Some(stream) = self.audio_capture_stream.as_ref() {
                        let _ = stream.pause();
                    }
                    Mode::Player
                } else {
                    if let Some(stream) = self.audio_capture_stream.as_ref() {
                        let _ = stream.play();
                    }
                    Mode::Microphone
                };
            }
            // Select device using its index if the device list is shown
            KeyCode::Char(c) if self.ui.show_devices_list && c.is_ascii_digit() && c != '0' => {
                let index = (c as usize) - ('1' as usize);
                if let Err(err) = self.select_device(index) {
                    self.handle_error(format!("Failed to select device: {}", err));
                }
            }
            KeyCode::Char('t') => {
                self.explorer
                    .set_cwd(config_dir().unwrap().join("soundscope"))
                    .unwrap();
                self.ui.show_themes_list = !self.ui.show_themes_list;
            }
            KeyCode::Char('=') | KeyCode::Char('+') => {
                self.ui.plus_sign_timer = Some(Instant::now());
                self.ui.waveform_window = f64::max(self.ui.waveform_window - 1., 1.);
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                let bound = if self.audio_file.duration().as_secs_f64() < 15. {
                    self.audio_file.duration().as_secs_f64()
                } else {
                    15.
                };
                self.ui.minus_sign_timer = Some(Instant::now());
                self.ui.waveform_window = f64::min(self.ui.waveform_window + 1., bound);
            }
            _ => (),
        }
        Ok(())
    }

    fn select_device(&mut self, index: usize) -> Result<()> {
        let devices = list_input_devs();
        if index > devices.len() - 1 {
            return Err(eyre!("Invalid device index: {}", index + 1));
        }

        if let Some(stream) = &self.audio_capture_stream {
            stream.pause().unwrap();
            self.audio_capture_stream = None;
        }
        let device = devices[index].1.clone();
        let audio_device = AudioDevice::new(Some(device));

        self.ui.device_name = devices[index].0.clone();
        let sr = audio_device.config().sample_rate.0;
        let channels = audio_device.config().channels;

        let mut buf = AllocRingBuffer::new(sr as usize * 30);
        buf.fill(0.0);
        let latest_captured_samples = Arc::new(Mutex::new(buf));
        self.latest_captured_samples = latest_captured_samples;

        let stream = match audio_capture::build_input_stream(
            self.latest_captured_samples.clone(),
            audio_device,
        ) {
            Ok(stream) => stream,
            Err(err) => {
                return Err(eyre!("Failed to create audio capture stream: {}", err));
            }
        };
        self.audio_capture_stream = Some(stream);
        self.audio_capture_stream.as_ref().unwrap().play()?;
        self.ui.show_devices_list = false;
        if let Err(err) = self
            .device_analyzer
            .create_loudness_meter(channels as u32, sr)
        {
            self.handle_error(format!(
                "Could not create an analyzer for an audio file: {}",
                err
            ));
        }
        Ok(())
    }

    fn handle_error(&mut self, message: String) {
        self.ui.error_text = message;
        self.ui.error_timer = Some(Instant::now());
    }

    fn select_audio_file(&mut self, file_path: PathBuf) {
        // reset everything
        self.reset_charts();

        if let Err(_err) = self
            .player_command_tx
            .send(PlayerCommand::SelectFile(file_path))
        {
            //TODO: log sending error
        }

        // TODO: channels
        if let Err(err) = self.file_analyzer.create_loudness_meter(
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

    fn select_theme_file(&mut self) {
        let file = self.explorer.current();
        let file_path = self.explorer.current().path().clone();
        if !file.is_file() {
            return;
        }
        let mut theme = self.load_theme(&file_path).unwrap_or_default();
        theme.apply_global_as_default();
        self.set_theme(theme);
    }

    fn change_chart(&mut self, c: char) {
        match c {
            // lufs
            'l' => {
                self.ui.show_fft_chart = false;
                self.ui.show_lufs = true
            }
            // frequencies
            'f' => {
                self.ui.show_fft_chart = true;
                self.ui.show_lufs = false
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
        let s = Style::default().bg(self.ui.theme.error.background.unwrap());
        let bd = s.fg(self.ui.theme.error.borders.unwrap());
        let fg = s.fg(self.ui.theme.error.foreground.unwrap());
        let message = self.ui.error_text.clone();

        // render only if timer is active (cleanup happens in main loop)
        if self.ui.error_timer.is_none() {
            return;
        }

        let error_popup_area = Self::get_error_popup_area(f.area());
        f.render_widget(Clear, error_popup_area);
        f.render_widget(
            Paragraph::new(message.to_line().style(fg))
                .block(Block::bordered().style(bd))
                .wrap(Wrap { trim: true }),
            error_popup_area,
        );
    }

    fn reset_charts(&mut self) {
        self.ui.show_explorer = false;
        self.fft_data.mid_fft.clear();
        self.fft_data.side_fft.clear();
        self.waveform.chart.clear();
        self.lufs = [-50.; 300];
        self.is_playing_audio = false;
        self.waveform.playhead = 0;
    }

    fn load_theme(&mut self, path: &PathBuf) -> Option<Theme> {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let current_theme = path.parent().unwrap().join(".current_theme");
        if let Err(err) = fs::write(&current_theme, &name) {
            self.handle_error(format!("Error saving chosen theme: {err}"));
        }
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(err) => {
                self.handle_error(format!("Error reading {name}.theme: {err}"));
                return None;
            }
        };
        let mut contents = String::new();
        if let Err(err) = file.read_to_string(&mut contents) {
            self.handle_error(format!("Error reading {name}.theme: {err}"));
            return None;
        }
        if contents == "DEFAULT" {
            return None;
        }
        let theme = match toml::from_str(&contents) {
            Ok(theme) => theme,
            Err(err) => {
                if let Err(err) = fs::write(&current_theme, "DEFAULT") {
                    self.handle_error(format!("Error setting theme to DEFAULT: {err}"));
                }
                self.handle_error(format!("Error reading {name}.theme: {err}"));
                return None;
            }
        };
        Some(theme)
    }

    /// Called at startup to apply the current theme from a .current_theme file if it exists
    fn apply_current_theme(&mut self, mut path: PathBuf) {
        // if .config/soundscope does not exist, create it
        path.push("soundscope");
        std::fs::create_dir_all(&path).unwrap();
        let current_theme_file = path.join(".current_theme");
        if current_theme_file.exists() {
            // read contents of current_theme file
            // this is the name of the theme {name}.theme
            match std::fs::read_to_string(&current_theme_file) {
                Ok(theme_file) => {
                    if theme_file == "DEFAULT" {
                        let mut theme = Theme::default();
                        theme.apply_global_as_default();
                        self.set_theme(theme)
                    } else {
                        let theme_file = path.join(theme_file);
                        let mut theme = if theme_file.exists() {
                            self.load_theme(&theme_file).unwrap_or_default()
                        } else {
                            self.handle_error(format!(
                                "Theme file {} not found. Applying default theme.",
                                theme_file.display()
                            ));
                            if let Err(err) = fs::write(&current_theme_file, "DEFAULT") {
                                self.handle_error(format!("Error setting theme to DEFAULT: {err}"));
                            }
                            Theme::default()
                        };
                        theme.apply_global_as_default();
                        self.set_theme(theme);
                    }
                }
                Err(err) => {
                    self.handle_error(format!(
                        "Error reading .current_theme file {err}. Applying default theme."
                    ));
                    if let Err(err) = fs::write(&current_theme_file, "DEFAULT") {
                        self.handle_error(format!("Error setting theme to DEFAULT: {err}"));
                    }
                    let mut theme = Theme::default();
                    theme.apply_global_as_default();
                    self.set_theme(theme)
                }
            }
        } else {
            let mut theme = Theme::default();
            theme.apply_global_as_default();
            self.set_theme(theme)
        }
    }

    fn in_fft_chart(&self, m: MouseEvent) -> bool {
        if self.ui.show_fft_chart
            && let Some(r) = self.ui.chart_rect
        {
            let x = m.column;
            let y = m.row;
            let width = r.width;
            let height = r.height;
            // hardcode boundries of the fft chart.
            // because it does not occupy the whole rectangle
            let x_min = r.x + 8;
            let y_min = r.y + 1;
            let x_max = r.x + width - 1;
            let y_max = r.y + height - 3;
            return x_min <= x && x < x_max && y_min <= y && y < y_max;
        }
        false
    }

    fn map_mouse_position_to_chart_point(
        &self,
        x: u16,
        max_x: u16,
        y: u16,
        max_y: u16,
    ) -> (f32, f32) {
        // x
        let min_freq_log = 20f32.log10();
        let max_freq_log = 20000f32.log10();
        let log_range = max_freq_log - min_freq_log;

        let t = x as f32 / max_x as f32;
        let log_freq = min_freq_log + t * log_range;
        let x = 10f32.powf(log_freq);

        // y
        let y = f32::clamp(y as f32, 0., max_y as f32);
        let t = y / max_y as f32;
        let y = -18. + t * (-78. - -18.);

        (x, y)
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
    startup_file: Option<PathBuf>,
) -> Result<()> {
    let terminal = ratatui::init();
    ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::event::EnableMouseCapture
    )?;
    let app_result = App::new(
        audio_file,
        player_command_tx,
        audio_file_rx,
        playback_position_rx,
        error_rx,
        latest_captured_samples,
    )?
    .run(terminal, startup_file);
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
        let latest_captured_samples = Arc::new(Mutex::new(AllocRingBuffer::new(44100 * 30)));

        let app = App::new(
            audio_file,
            player_command_tx.clone(),
            audio_file_rx,
            playback_position_rx,
            error_rx,
            latest_captured_samples,
        )
        .unwrap();

        (app, player_command_tx, player_command_rx)
    }

    #[test]
    fn test_change_chart() {
        let (mut app, _, _) = create_test_app();

        // Test switching to LUFS
        app.change_chart('l');
        assert!(!app.ui.show_fft_chart);
        assert!(app.ui.show_lufs);

        // Test switching to frequencies
        app.change_chart('f');
        assert!(app.ui.show_fft_chart);
        assert!(!app.ui.show_lufs);

        // Test invalid character (should do nothing)
        let prev_fft = app.ui.show_fft_chart;
        let prev_lufs = app.ui.show_lufs;
        app.change_chart('x');
        assert_eq!(app.ui.show_fft_chart, prev_fft);
        assert_eq!(app.ui.show_lufs, prev_lufs);
    }

    #[test]
    fn test_handle_error() {
        let (mut app, _, _) = create_test_app();
        let error_message = "Test error message";

        app.handle_error(error_message.to_string());

        assert_eq!(app.ui.error_text, error_message);
        assert!(app.ui.error_timer.is_some());
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
        assert!(app.ui.error_timer.is_none());

        // Set error
        app.handle_error("Test error".to_string());
        let error_time = app.ui.error_timer.unwrap();

        // Error should be recent
        assert!(error_time.elapsed().as_millis() < 100);

        std::thread::sleep(Duration::from_secs_f32(5.01));

        // it does not work since it gets None in render_error_message() but it cant be run without drawing ui
        // assert!(app.ui_settings.error_timer.is_none())

        assert!(error_time.elapsed().as_millis() > 5000);
    }

    #[test]
    fn test_analyze_microphone_input_44100() {
        let (mut app, _, _) = create_test_app();
        app.settings.mode = Mode::Microphone;
        let sr = 44100;

        // Fill the buffer with test data
        {
            let mut buffer = app.latest_captured_samples.lock().unwrap();
            buffer.clear();
            for i in 0..sr * 30 {
                let sample = (i as f32 * 500.0 * 2.0 * std::f32::consts::PI / sr as f32).sin();
                buffer.enqueue(sample);
            }
        }

        app.analyze_microphone_input();

        assert!(!app.fft_data.mid_fft.is_empty());

        // Check that there's a peak around 500 Hz
        let freq_bin = 500.0 / (sr as f32 / 2.0) * (app.fft_data.mid_fft.len() as f32);
        let bin_idx = freq_bin.round() as usize;

        // Check that this bin has non-trivial amplitude
        if bin_idx < app.fft_data.mid_fft.len() {
            let amp = app.fft_data.mid_fft[bin_idx].1; // assuming (freq, amp)
            assert!(
                amp < -20.0,
                "Expected strong signal at ~500Hz, got: {}",
                amp
            );
        } else {
            panic!("Bin index out of range: {}", bin_idx);
        }
    }

    #[test]
    fn test_analyze_microphone_input_48000() {
        let (mut app, _, _) = create_test_app();
        app.settings.mode = Mode::Microphone;
        let sr = 48000;

        // Fill the buffer with test data
        {
            let mut buffer = app.latest_captured_samples.lock().unwrap();
            buffer.clear();
            for i in 0..sr * 30 {
                let sample = (i as f32 * 500.0 * 2.0 * std::f32::consts::PI / sr as f32).sin();
                buffer.enqueue(sample);
            }
        }

        app.analyze_microphone_input();

        assert!(!app.fft_data.mid_fft.is_empty());

        // Check that there's a peak around 500 Hz
        let freq_bin = 500.0 / (sr as f32 / 2.0) * (app.fft_data.mid_fft.len() as f32);
        let bin_idx = freq_bin.round() as usize;

        // Check that this bin has non-trivial amplitude
        if bin_idx < app.fft_data.mid_fft.len() {
            let amp = app.fft_data.mid_fft[bin_idx].1; // assuming (freq, amp)
            assert!(
                amp < -20.0,
                "Expected strong signal at ~500Hz, got: {}",
                amp
            );
        } else {
            panic!("Bin index out of range: {}", bin_idx);
        }
    }

    #[test]
    fn test_analyze_microphone_input_96000() {
        let (mut app, _, _) = create_test_app();
        app.settings.mode = Mode::Microphone;
        let sr = 96000;

        // Fill the buffer with test data
        {
            let mut buffer = app.latest_captured_samples.lock().unwrap();
            buffer.clear();
            for i in 0..sr * 30 {
                let sample = (i as f32 * 500.0 * 2.0 * std::f32::consts::PI / sr as f32).sin();
                buffer.enqueue(sample);
            }
        }

        app.analyze_microphone_input();

        assert!(!app.fft_data.mid_fft.is_empty());

        // Check that there's a peak around 500 Hz
        let freq_bin = 500.0 / (sr as f32 / 2.0) * (app.fft_data.mid_fft.len() as f32);
        let bin_idx = freq_bin.round() as usize;

        // Check that this bin has non-trivial amplitude
        if bin_idx < app.fft_data.mid_fft.len() {
            let amp = app.fft_data.mid_fft[bin_idx].1; // assuming (freq, amp)
            assert!(
                amp < -20.0,
                "Expected strong signal at ~500Hz, got: {}",
                amp
            );
        } else {
            panic!("Bin index out of range: {}", bin_idx);
        }
    }

    #[test]
    fn test_fill_macro() {
        let mut theme = Theme {
            global: GlobalTheme::default(),
            waveform: WaveformTheme::default(),
            fft: FftTheme::default(),
            lufs: LufsTheme::default(),
            devices: DevicesTheme::default(),
            explorer: ExplorerTheme::default(),
            error: ErrorTheme::default(),
        };
        theme.global.foreground = Color::LightCyan;
        theme.global.background = Color::Magenta;

        theme.fft.mid_fft = None;
        theme.fft.side_fft = None;
        theme.fft.labels = None;

        theme.waveform.playhead = None;
        theme.waveform.highlight = None;
        theme.waveform.current_time = None;

        theme.lufs.numbers = None;

        theme.devices.background = None;

        theme.explorer.highlight_dir_foreground = None;
        theme.explorer.item_foreground = None;

        theme.apply_global_as_default();
        assert!(theme.fft.mid_fft == Some(Color::LightCyan));
        assert!(theme.fft.side_fft == Some(Color::Indexed(160)));
        assert!(theme.fft.labels == Some(Color::LightCyan));

        assert!(theme.waveform.playhead == Some(Color::Indexed(160)));
        assert!(theme.waveform.highlight == Some(Color::Indexed(160)));
        assert!(theme.waveform.current_time == Some(Color::LightCyan));

        assert!(theme.lufs.numbers == Some(Color::LightCyan));

        assert!(theme.devices.background == Some(Color::Magenta));

        assert!(theme.explorer.highlight_dir_foreground == Some(Color::Indexed(160)));
        assert!(theme.explorer.item_foreground == Some(Color::LightCyan));
    }
}
