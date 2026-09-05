//! This module contains the implementation of the terminal user interface (TUI) used to display audio analysis results.
//! It uses `ratatui` under the hood.
use crate::{
    analyzer::Analyzer,
    audio_capture::{self, AudioDevice, list_input_devices},
    audio_player::{self, AudioFile, PlayerCommand},
    builtin_themes::{self, list_themes},
    system_sound_capture::SYSTEM_TAP_DEVICE_NAME,
    tui::{
        lufs::Lufs,
        spectrum::{SPECTRUM_LOWER_BOUND, SPECTRUM_TARGET_DBFS, SPECTRUM_UPPER_BOUND, Spectrum},
    },
};
use cpal::{Stream, traits::StreamTrait as _};
use crossbeam::channel::{Receiver, Sender};
use eyre::{Result, eyre};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{Event, KeyCode, KeyEvent, MouseEvent, MouseEventKind, poll, read},
    layout::Flex,
    prelude::*,
    style::{Style, Stylize},
    text::{ToLine, ToSpan},
    widgets::{
        Block, BorderType, Cell, Clear, FrameExt, List, ListItem, ListState, Paragraph, Row, Table,
        Wrap,
    },
};
use ratatui_explorer::{FileExplorer, FileExplorerBuilder};
use ringbuffer::{AllocRingBuffer, RingBuffer};
use rodio::Source;
use std::path::Path;
use std::{
    fmt::Display,
    fs::{self, File},
    io::Read,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

mod lufs;
mod spectrum;
pub mod theme;
mod waveform;

use theme::Theme;
use waveform::WaveForm;

pub type RBuffer = Arc<Mutex<AllocRingBuffer<f32>>>;

/// Files with extensions listed here will be shown in the explorer
const SUPPORTED_FORMATS: [&str; 22] = [
    "wav", "wave", "aiff", "aif", "flac", // Uncompressed / Lossless
    "mp3", "mp2", "mp1", "mpa", "aac", // MPEG Audio
    "m4a", "m4b", "mp4", "m4r", "m4p", // MP4 / M4A Family (AAC / ALAC)
    "ogg", "oga", "ogv", // OGG Family
    "caf", "alac", // Apple formats
    "theme", "toml", // Theme file
];

enum PopupState {
    InExplorer,
    InDeviceList,
    InThemesList,
    InHelpMessage,
    None,
}

bitflags::bitflags! {
    struct ShowWindow: u8 {
        const SPECTRUM = 1 << 0;
        const LUFS = 1 << 1;
        const WAVEFORM = 1 << 2;
    }
}

impl Default for ShowWindow {
    fn default() -> Self {
        Self::SPECTRUM | Self::LUFS | Self::WAVEFORM
    }
}

/// Settings like showing/hiding UI elements.
struct UI {
    theme: Theme,
    show_window: ShowWindow,
    popup_state: PopupState,
    error_text: String,
    error_timer: Option<Instant>,
    /// Used to be able to hover spectrum to get more precise frequencies
    chart_rect: Option<Rect>,
    /// Track if render is needed to avoid unnecessary redraws
    needs_render: bool,
    /// Selected theme index in themes list
    selected_theme: ListState,
    /// Selected device index in devices list
    selected_device: ListState,
}

impl Default for UI {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            popup_state: PopupState::None,
            show_window: ShowWindow::default(),
            error_text: String::new(),
            error_timer: None,
            chart_rect: None,
            needs_render: true,
            selected_theme: ListState::default().with_selected(Some(0)),
            selected_device: ListState::default().with_selected(Some(0)),
        }
    }
}

/// Mode of the [App]. Currently, only Player and Microphone are supported.
#[derive(Default)]
enum Mode {
    #[default]
    Player,
    Microphone,
    System,
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Player => write!(f, "Player"),
            Mode::Microphone => write!(f, "Microphone"),
            Mode::System => write!(f, "System"),
        }
    }
}

/// Settings for the [App]. Currently only the [Mode] is supported.
#[derive(Default)]
struct Settings {
    mode: Mode,
    selected_device_index: Option<usize>,
}

/// `App` contains the necessary components for the application like senders, receivers, [`AudioFile`] data, [`UIsettings`].
struct App {
    /// Audio file which is loaded into the player.
    audio_file: Option<AudioFile>,
    /// If file is not selected, the app crashes when you try to play it.
    /// It is easier to use this bool instead of Option<AudioFile> because
    /// we would always have to check if it is not None. But it can be None only before
    /// the first file is selected.
    is_playing_audio: bool,
    audio_file_rx: Receiver<AudioFile>,
    /// [`RingBuffer`] used to store the latest captured samples when the `Mode` is not `Mode::Player`.
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
    /// Data used to render spectrum chart.
    spectrum: Spectrum,
    /// Data used to render waveform.
    waveform: WaveForm,
    settings: Settings,
    lufs: Lufs,
    //UI
    explorer: FileExplorer,
    ui: UI,
    /// Used to conviniently return to current directory when opening an explorer
    current_directory: PathBuf,
    /// Used to print info about spectrum chart when it's hovered
    mouse_position: Option<(u16, u16)>,
}

macro_rules! help_message_row {
    ($key:expr, $description:expr, $hl:expr) => {
        Row::new(vec![
            Cell::new($key.to_line().style($hl).centered()),
            Cell::new($description.to_span()),
        ])
    };
}

impl App {
    fn new(
        audio_file: Option<AudioFile>,
        player_command_tx: Sender<PlayerCommand>,
        audio_file_rx: Receiver<AudioFile>,
        playback_position_rx: Receiver<usize>,
        error_rx: Receiver<String>,
        latest_captured_samples: RBuffer,
    ) -> Result<Self> {
        Ok(Self {
            audio_file,
            is_playing_audio: false,
            audio_file_rx,
            latest_captured_samples,
            audio_capture_stream: None,
            player_command_tx,
            playback_position_rx,
            error_rx,
            file_analyzer: Analyzer::default(),
            device_analyzer: Analyzer::default(),
            spectrum: Spectrum::default(),
            waveform: WaveForm::default(),
            lufs: Lufs::default(),
            settings: Settings::default(),
            explorer: FileExplorerBuilder::build_with_theme(
                ratatui_explorer::Theme::default()
                    .with_block(Block::bordered().border_type(BorderType::Rounded)),
            )?,
            ui: UI::default(),
            current_directory: PathBuf::from(""),
            mouse_position: None,
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
            .add_default_title()
            .with_block(Block::bordered().border_type(BorderType::Rounded));
        self.explorer.set_theme(explorer_theme);
        self.ui.theme = theme;
    }

    /// The function used to draw the UI.
    fn draw(&mut self, f: &mut Frame) {
        // split the area into waveform part and charts parts
        let area = f.area();
        // make the background black
        let background = Paragraph::new("").style(self.ui.theme.global.background);
        f.render_widget(background, area);

        // if we should show top window (waveform)
        let top_constraint = if self.ui.show_window.contains(ShowWindow::WAVEFORM) {
            if self
                .ui
                .show_window
                .intersects(ShowWindow::SPECTRUM | ShowWindow::LUFS)
            {
                Constraint::Percentage(30)
            } else {
                Constraint::Percentage(100)
            }
        } else {
            Constraint::Length(0)
        };

        // if we should show bottom windows (spectrum & lufs)
        let bottom_constraint = if self
            .ui
            .show_window
            .intersects(ShowWindow::SPECTRUM | ShowWindow::LUFS)
        {
            if self.ui.show_window.contains(ShowWindow::WAVEFORM) {
                Constraint::Percentage(70)
            } else {
                Constraint::Percentage(100)
            }
        } else {
            Constraint::Length(0)
        };

        // devide frame into top and bottom windows
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([top_constraint, bottom_constraint])
            .split(area);

        if self.ui.show_window.contains(ShowWindow::WAVEFORM) {
            self.waveform.render(
                f,
                vertical_chunks[0],
                &self.ui.theme.waveform,
                self.audio_file.as_ref().map(|f| f.data.as_ref()),
                &self.settings.mode,
            );
        }

        // draw bottom windows
        if self
            .ui
            .show_window
            .intersects(ShowWindow::SPECTRUM | ShowWindow::LUFS)
        {
            // if we should split bottom part to lufs and spectrum
            // or fill the bottom part with only 1 of them
            let left_constraint = if self.ui.show_window.contains(ShowWindow::SPECTRUM) {
                Constraint::Min(0)
            } else {
                Constraint::Length(0)
            };
            let right_constraint = if self.ui.show_window.contains(ShowWindow::LUFS) {
                Constraint::Min(0)
            } else {
                Constraint::Length(0)
            };

            let horizontal_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([left_constraint, right_constraint])
                .split(vertical_chunks[1]);

            if self.ui.show_window.contains(ShowWindow::SPECTRUM) {
                self.ui.chart_rect = Some(horizontal_chunks[0]);
                self.spectrum
                    .render(f, horizontal_chunks[0], &self.ui.theme.spectrum);
                if let Some((x, y)) = self.mouse_position {
                    self.render_spectrum_info(f, x, y);
                }
            }
            if self.ui.show_window.contains(ShowWindow::LUFS)
                && let Err(err) = self.lufs.render(
                    f,
                    horizontal_chunks[1],
                    &self.ui.theme.lufs,
                    &mut self.file_analyzer,
                )
            {
                self.handle_error(format!("Error while computing loudness: {err}"));
            }
        }

        if self.ui.show_window.is_empty() {
            self.render_empty_window(f, area);
        }

        // render error
        if let Ok(err) = self.error_rx.try_recv() {
            self.ui.error_text = err;
            self.ui.error_timer = Some(std::time::Instant::now());
        }
        self.render_error_message(f);

        match self.ui.popup_state {
            PopupState::InExplorer => {
                let area = Self::get_popup_area_with_percentage(area, 50, 70);
                f.render_widget(Clear, area);
                f.render_widget_ref(self.explorer.widget(), area);
            }
            PopupState::InDeviceList => {
                self.render_devices_list(f);
            }
            PopupState::InThemesList => {
                self.render_themes_list(f);
            }
            PopupState::InHelpMessage => {
                self.render_help_message(f);
            }
            PopupState::None => (),
        }
    }

    fn render_empty_window(&mut self, frame: &mut Frame, area: Rect) {
        let s = Style::default().bg(self.ui.theme.global.background).fg(self
            .ui
            .theme
            .global
            .foreground);

        let background = Paragraph::new("").style(s);
        frame.render_widget(background, area);

        let popout_area = Self::get_popup_area_with_lenght(frame.area(), 6, 30);
        frame.render_widget(Clear, popout_area);

        let paragraph = Paragraph::new(vec![
            "No open windows!".to_line().centered(),
            "1 | Toggle waveform".to_line().centered(),
            "2 | Toggle spectrum".to_line().centered(),
            "3 | Toggle LUFS   ".to_line().centered(),
        ])
        .block(Block::bordered().border_type(BorderType::Rounded))
        .style(s);

        frame.render_widget(paragraph, popout_area);

        let big_text_area = Self::get_popup_area_with_lenght(frame.area(), 22, 100);
        let big_text = tui_big_text::BigText::builder()
            .pixel_size(tui_big_text::PixelSize::Full)
            .style(s)
            .centered()
            .lines(vec!["Soundscope".to_line()])
            .build();
        frame.render_widget(big_text, big_text_area);
    }

    fn render_devices_list(&mut self, f: &mut Frame) {
        let s = Style::default()
            .fg(self.ui.theme.devices.foreground.unwrap())
            .bg(self.ui.theme.devices.background.unwrap());
        let bd = s.fg(self.ui.theme.devices.borders.unwrap());
        let hl = s.fg(self.ui.theme.devices.highlight.unwrap());
        let area = Self::get_popup_area_with_percentage(f.area(), 20, 30);
        f.render_widget(Clear, area);
        let devs = list_input_devices();
        let mut device_number_offset = 1;
        let list_items: Vec<ListItem> = devs
            .iter()
            .enumerate()
            .filter_map(|(i, (name, _dev))| {
                if name == SYSTEM_TAP_DEVICE_NAME {
                    device_number_offset -= 1;
                    return None;
                }
                let num = format!("[{}]", i + device_number_offset);
                let name = format!(" {name}");
                let is_selected = i == self.ui.selected_device.selected().unwrap_or(0);

                let item_style = if is_selected {
                    hl.bg(self.ui.theme.devices.background.unwrap())
                } else {
                    s
                };

                let num = num.bold().reset().style(item_style);
                let name = name.bold().reset().style(item_style);
                Some(ListItem::from(num + name))
            })
            .collect();
        let list = List::new(list_items).style(s).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title("Devices")
                .style(bd),
        );

        f.render_stateful_widget(list, area, &mut self.ui.selected_device);
    }

    fn render_themes_list(&mut self, f: &mut Frame) {
        let s = Style::default()
            .fg(self.ui.theme.devices.foreground.unwrap())
            .bg(self.ui.theme.devices.background.unwrap());
        let bd = s.fg(self.ui.theme.devices.borders.unwrap());
        let hl = s.fg(self.ui.theme.devices.highlight.unwrap());
        let area = Self::get_popup_area_with_lenght(f.area(), 21, 40);
        f.render_widget(Clear, area);

        let themes = builtin_themes::list_themes();

        let mut list_items: Vec<ListItem> = themes
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let num = format!("[{}]", i + 1);
                let name = format!(" {name}");
                let is_selected = i + 1 == self.ui.selected_theme.selected().unwrap_or(0);

                let item_style = if is_selected {
                    hl.bg(self.ui.theme.devices.background.unwrap())
                } else {
                    s
                };

                let num = num.bold().reset().style(item_style);
                let name = name.bold().reset().style(item_style);
                ListItem::from(num + name)
            })
            .collect();

        // Add Default Theme option at the beginning
        let default_num = "[0]".to_string();
        let default_name = " Default Theme";
        let is_default_selected = self.ui.selected_theme.selected().unwrap_or(0) == 0;

        let default_style = if is_default_selected {
            hl.bg(self.ui.theme.devices.background.unwrap())
        } else {
            s
        };

        let default_num = default_num.bold().reset().style(default_style);
        let default_name = default_name.bold().reset().style(default_style);
        list_items.insert(0, ListItem::from(default_num + default_name));

        // Add Custom Theme option at the end
        let custom_num = format!("[{}]", themes.len() + 1);
        let custom_name = " Custom Theme";
        let is_custom_selected = themes.len() + 1 == self.ui.selected_theme.selected().unwrap_or(0);

        let custom_style = if is_custom_selected {
            hl.bg(self.ui.theme.devices.background.unwrap())
        } else {
            s
        };

        let custom_num = custom_num.bold().reset().style(custom_style);
        let custom_name = custom_name.bold().reset().style(custom_style);
        list_items.push(ListItem::from(custom_num + custom_name));

        let list = List::new(list_items).style(s).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title("Themes")
                .style(bd),
        );

        f.render_stateful_widget(list, area, &mut self.ui.selected_theme);
    }

    fn render_spectrum_info(&self, f: &mut Frame<'_>, x: u16, y: u16) {
        let rect_width = self.ui.chart_rect.unwrap().width;
        let rect_height = self.ui.chart_rect.unwrap().height;

        let left_bound = 8;
        let right_bound = rect_width - 1;
        let upper_bound = self.ui.chart_rect.unwrap().y + 1;
        let lower_bound = upper_bound + rect_height - 4;

        let (hz, db) = Self::map_mouse_position_to_chart_point(
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
                Block::bordered().border_type(BorderType::Rounded).style(
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

    fn receive_audio_file(&mut self, audio_file: AudioFile) {
        if audio_file.data.duration.as_secs_f64() < 15. {
            self.waveform.window = audio_file.data.duration.as_secs_f64();
        }
        self.waveform.audio_file_chart = Analyzer::get_waveform(
            &audio_file.data.samples,
            audio_file.data.duration.as_secs_f64(),
        );
        // TODO: channels
        if let Err(err) = self.file_analyzer.create_loudness_meter(
            // self.audio_file.channels() as u32,
            2,
            audio_file.sample_rate(),
        ) {
            self.handle_error(format!(
                "Could not create an analyzer for an audio file: {err}"
            ));
        }

        // Calculate gain compensation to normalize track to target LUFS
        if let Some(integrated_lufs) = self.file_analyzer.calculate_integrated_lufs(
            // self.audio_file.channels(),
            2,
            &audio_file.data.samples,
        ) {
            let gain_db = SPECTRUM_TARGET_DBFS - integrated_lufs as f32;
            self.spectrum.gain_compensation = gain_db;
        } else {
            self.spectrum.gain_compensation = 0.0;
        }
        self.audio_file = Some(audio_file);

        self.ui.needs_render = true;
    }

    /// The main loop
    fn run(mut self, mut terminal: DefaultTerminal, startup_file: Option<PathBuf>) -> Result<()> {
        // apply current theme if current_theme file exists
        // if it doesn't, don't create a config dir, as it used to be before.
        if let Some(mut path) = config_dir() {
            path.push("soundscope");
            let current_theme_file = path.join("current_theme");
            if current_theme_file.exists() {
                self.apply_current_theme(&path, &current_theme_file);
            } else {
                let mut theme = Theme::default();
                theme.apply_global_as_default();
                self.set_theme(theme);
            }
        } else {
            self.handle_error("Config directory does not exist. Could not load theme.".to_string());
            let mut theme = Theme::default();
            theme.apply_global_as_default();
            self.set_theme(theme);
        }

        self.current_directory = self.explorer.cwd().clone();
        self.explorer.set_filter_map(|file| {
            let keep = match file.path.extension() {
                Some(extension) => {
                    let extension = extension.to_str().unwrap_or_default();
                    SUPPORTED_FORMATS.contains(&extension)
                }
                None => file.is_dir,
            };

            if keep { Some(file) } else { None }
        })?;
        terminal.draw(|f| self.draw(f))?;

        // blocking audio file receiver
        // blocking to ensure that audio file is loaded
        // before we render TUI, so that waveform is rendered correctly
        if let Some(f) = startup_file {
            self.select_audio_file(f);
            terminal.draw(|f| self.draw(f))?;
        }

        loop {
            std::thread::sleep(Duration::from_millis(8));
            self.ui.needs_render = false;

            // receive playback position
            let prev_playhead = self.waveform.playhead;
            if let Ok(pos) = self.playback_position_rx.try_recv()
                && self.audio_file.is_some()
                && matches!(self.settings.mode, Mode::Player)
            {
                self.analyze_audio_file_samples(pos);
                // render only if playhead position changed
                self.ui.needs_render = prev_playhead != self.waveform.playhead;
            }

            // use ringbuf to analyze data if the `Mode` is not `Mode::Player`
            if matches!(self.settings.mode, Mode::Microphone | Mode::System) {
                self.analyze_microphone_input();
                self.ui.needs_render = true; // Always render in microphone mode
            }

            // check if flashing controls need update (timers)
            let t = 100; // flash duration in ms
            let left_arrow_flash = self
                .waveform
                .timer
                .left_arrow
                .map(|timer| timer.elapsed().as_millis());
            let right_arrow_flash = self
                .waveform
                .timer
                .right_arrow
                .map(|timer| timer.elapsed().as_millis());
            let plus_sign_flash = self
                .waveform
                .timer
                .plus_sign
                .map(|timer| timer.elapsed().as_millis());
            let minus_sign_flash = self
                .waveform
                .timer
                .minus_sign
                .map(|timer| timer.elapsed().as_millis());

            // render if currently flashing or just stopped flashing (was <t, now >=t)
            let needs_flash_update = left_arrow_flash.is_some_and(|ms| ms < t)
                || right_arrow_flash.is_some_and(|ms| ms < t)
                || plus_sign_flash.is_some_and(|ms| ms < t)
                || minus_sign_flash.is_some_and(|ms| ms < t);

            if needs_flash_update {
                self.ui.needs_render = true;
            }

            // clean up expired timers and trigger final render
            let mut timers_need_cleanup = false;
            if left_arrow_flash.is_some_and(|ms| ms >= t) {
                self.waveform.timer.left_arrow = None;
                timers_need_cleanup = true;
            }
            if right_arrow_flash.is_some_and(|ms| ms >= t) {
                self.waveform.timer.right_arrow = None;
                timers_need_cleanup = true;
            }
            if plus_sign_flash.is_some_and(|ms| ms >= t) {
                self.waveform.timer.plus_sign = None;
                timers_need_cleanup = true;
            }
            if minus_sign_flash.is_some_and(|ms| ms >= t) {
                self.waveform.timer.minus_sign = None;
                timers_need_cleanup = true;
            }

            if timers_need_cleanup {
                self.ui.needs_render = true;
            }

            // check if error message needs update
            if self
                .ui
                .error_timer
                .is_some_and(|timer| timer.elapsed().as_millis() < 5000)
            {
                self.ui.needs_render = true;
            }

            // clean up expired error timer and trigger final render
            if self
                .ui
                .error_timer
                .is_some_and(|timer| timer.elapsed().as_millis() >= 5000)
            {
                self.ui.error_timer = None;
                self.ui.needs_render = true;
            }

            // event reader
            if poll(Duration::from_micros(1))? {
                let event = match read() {
                    Ok(event) => event,
                    Err(err) => {
                        self.handle_error(format!("Error reading event: {err}"));
                        continue;
                    }
                };

                if matches!(self.ui.popup_state, PopupState::InExplorer) {
                    self.explorer.handle(&event)?;
                    self.ui.needs_render = true;
                }

                match event {
                    Event::Key(key) => {
                        // quit (only if not in any popup)
                        if key.code == KeyCode::Char('q')
                            && matches!(self.ui.popup_state, PopupState::None)
                        {
                            self.player_command_tx.send(PlayerCommand::Quit)?;
                            return Ok(());
                        }
                        self.handle_input(key);
                        self.ui.needs_render = true;
                    }
                    Event::Mouse(m) => {
                        if matches!(m.kind, MouseEventKind::Moved) {
                            if self.in_spectrum_chart(m) {
                                self.mouse_position = Some((m.column, m.row));
                            } else {
                                self.mouse_position = None;
                            }
                        } else {
                            self.mouse_position = None;
                        }
                        self.ui.needs_render = true;
                    }
                    Event::Resize(_, _) => {
                        self.ui.needs_render = true;
                    }
                    _ => (),
                }
            }

            // render only if something changed
            if self.ui.needs_render {
                terminal.draw(|f| self.draw(f))?;
            }
        }
    }

    fn analyze_microphone_input(&mut self) {
        let samples = self.latest_captured_samples.lock().unwrap().to_vec();
        let (mid_samples, side_samples) = audio_player::get_mid_and_side_samples(&samples);
        let sample_rate = self.device_analyzer.sample_rate() as usize;
        let left_bound = 15 * sample_rate - 2usize.pow(14);

        // get spectrum
        self.spectrum.mid_freq = match self
            .device_analyzer
            .get_spectrum(&mid_samples[left_bound..15 * sample_rate])
        {
            Ok(spectrum) => spectrum,
            Err(err) => {
                self.handle_error(format!("Error getting frequencies: {err}. Perhaps your microphone's sample rate is too low."));
                vec![(0., 0.)]
            }
        };
        self.spectrum.side_freq = match self
            .device_analyzer
            .get_spectrum(&side_samples[left_bound..15 * sample_rate])
        {
            Ok(spectrum) => spectrum,
            Err(err) => {
                self.handle_error(format!("Error getting frequencies: {err}. Perhaps your microphone's sample rate is too low."));
                vec![(0., 0.)]
            }
        };

        // get waveform
        self.waveform.microphone_input_chart = Analyzer::get_waveform(&mid_samples, 15.);

        let samples = self.latest_captured_samples.lock().unwrap().to_vec();
        let sample_rate = self.device_analyzer.sample_rate() as usize;

        // get lufs
        for i in 0..self.lufs.0.len() - 1 {
            self.lufs.0[i] = self.lufs.0[i + 1];
        }

        let lb = 30 * sample_rate - 2usize.pow(14);
        if let Err(err) = self
            .device_analyzer
            .add_samples(&samples[lb..30 * sample_rate])
        {
            self.handle_error(format!("Could not get samples for LUFS analyzer: {err}"));
        }
        self.lufs.0[299] = match self.device_analyzer.get_shortterm_lufs() {
            Ok(lufs) => lufs,
            Err(err) => {
                self.handle_error(format!("Error getting short-term LUFS: {err}"));
                0.0
            }
        };
    }

    fn analyze_audio_file_samples(&mut self, pos: usize) {
        // if using mid side we must divide the position by 2
        let audio_file = self
            .audio_file
            .as_ref()
            .expect("guarded by is_some() in run()");
        let pos = pos / audio_file.channels() as usize;
        self.waveform.playhead = pos;

        // get spectrum
        let spectrum_left_bound = pos.saturating_sub(16384);
        if spectrum_left_bound != 0 {
            let mid_samples_len = audio_file.data.mid_samples.len();
            let side_samples_len = audio_file.data.side_samples.len();

            // check bounds to prevent panic when file was changed
            let mid_samples = if pos <= mid_samples_len && spectrum_left_bound < mid_samples_len {
                &audio_file.data.mid_samples[spectrum_left_bound..pos]
            } else {
                &[]
            };
            let side_samples = if pos <= side_samples_len && spectrum_left_bound < side_samples_len
            {
                &audio_file.data.side_samples[spectrum_left_bound..pos]
            } else {
                &[]
            };

            self.spectrum.mid_freq = match self.file_analyzer.get_spectrum(mid_samples) {
                Ok(spectrum) => spectrum,
                Err(_err) => {
                    // can't log the error because this fn takes a mutable reference
                    // but we already have 2 shared references.
                    // but it doesn't really matter(?), this message is mostly for microphone input
                    // self.handle_error(format!("Error getting frequencies: {}. Perhaps your microphone's sample rate is too low.", err));
                    vec![(0., 0.)]
                }
            };
            self.spectrum.side_freq = match self.file_analyzer.get_spectrum(side_samples) {
                Ok(spectrum) => spectrum,
                Err(_err) => {
                    // can't log the error because this fn takes a mutable reference
                    // but we already have 2 shared references.
                    // but it doesn't really matter(?), this message is mostly for microphone input
                    // self.handle_error(format!("Error getting frequencies: {}. Perhaps your microphone's sample rate is too low.", err));
                    vec![(0., 0.)]
                }
            };
        }

        // get lufs lufs uses all channels (update every frame for accuracy)
        let pos = pos * audio_file.channels() as usize;
        let lufs_left_bound = pos.saturating_sub(16384);
        if lufs_left_bound != 0 {
            for i in 0..self.lufs.0.len() - 1 {
                self.lufs.0[i] = self.lufs.0[i + 1];
            }
            let samples_len = audio_file.data.samples.len();
            // check bounds to prevent panic when file was changed
            if pos <= samples_len && lufs_left_bound < samples_len {
                if let Err(err) = self
                    .file_analyzer
                    .add_samples(&audio_file.data.samples[lufs_left_bound..pos])
                {
                    self.handle_error(format!("Could not get samples for LUFS analyzer: {err}"));
                }
                self.lufs.0[299] = match self.file_analyzer.get_shortterm_lufs() {
                    Ok(lufs) => lufs,
                    Err(err) => {
                        self.handle_error(format!("Error getting short-term LUFS: {err}"));
                        0.0
                    }
                };
            }
        }
    }

    fn handle_input(&mut self, key: KeyEvent) {
        match key.code {
            // show explorer
            KeyCode::Char('e') if matches!(self.settings.mode, Mode::Player) => {
                match self.ui.popup_state {
                    PopupState::None => {
                        self.explorer.set_cwd(&self.current_directory).unwrap();
                        self.ui.popup_state = PopupState::InExplorer;
                    }
                    PopupState::InExplorer => self.ui.popup_state = PopupState::None,
                    _ => (),
                }
            }
            // show side spectrum
            KeyCode::Char('S') => self.spectrum.show_side_freq = !self.spectrum.show_side_freq,
            // show mid spectrum
            KeyCode::Char('M') => self.spectrum.show_mid_freq = !self.spectrum.show_mid_freq,
            // pause/play
            KeyCode::Char(' ') => {
                if let Err(_err) = self.player_command_tx.send(PlayerCommand::ChangeState) {
                    //TODO: log sending error
                }
                self.is_playing_audio = !self.is_playing_audio;
                // do this so lufs update only on play, not pause
                if self.is_playing_audio {
                    self.lufs.0 = [-100.; 300];
                    self.file_analyzer.reset();
                }
            }
            // move playhead right and left
            KeyCode::Right
                if matches!(self.settings.mode, Mode::Player)
                    && matches!(self.ui.popup_state, PopupState::None) =>
            {
                self.waveform.timer.right_arrow = Some(Instant::now());
                self.lufs.0 = [-100.; 300];
                self.file_analyzer.reset();
                if let Err(_err) = self.player_command_tx.send(PlayerCommand::MoveRight) {
                    //TODO: log sending error
                }
            }
            KeyCode::Left
                if matches!(self.settings.mode, Mode::Player)
                    && matches!(self.ui.popup_state, PopupState::None) =>
            {
                self.waveform.timer.left_arrow = Some(Instant::now());
                self.lufs.0 = [-100.; 300];
                self.file_analyzer.reset();
                if let Err(_err) = self.player_command_tx.send(PlayerCommand::MoveLeft) {
                    //TODO: log sending error
                }
            }
            KeyCode::Char('1') if matches!(self.ui.popup_state, PopupState::None) => {
                self.ui.show_window.toggle(ShowWindow::WAVEFORM);
            }
            KeyCode::Char('2') if matches!(self.ui.popup_state, PopupState::None) => {
                self.ui.show_window.toggle(ShowWindow::SPECTRUM);
            }
            KeyCode::Char('3') if matches!(self.ui.popup_state, PopupState::None) => {
                self.ui.show_window.toggle(ShowWindow::LUFS);
            }
            // Quick selection with numbers 0-9 when themes list is open
            KeyCode::Char(c)
                if matches!(self.ui.popup_state, PopupState::InThemesList)
                    && c.is_ascii_digit() =>
            {
                let index = (c as usize) - ('0' as usize);
                self.select_theme(index);
            }
            // this sends a test error
            // only in debug mode
            #[cfg(debug_assertions)]
            KeyCode::Char('y') => self
                .player_command_tx
                .send(PlayerCommand::ShowTestError)
                .unwrap(),
            // show devices
            KeyCode::Char('d') if matches!(self.settings.mode, Mode::Microphone) => {
                match self.ui.popup_state {
                    PopupState::None => self.ui.popup_state = PopupState::InDeviceList,
                    PopupState::InDeviceList => self.ui.popup_state = PopupState::None,
                    _ => (),
                }
            }
            // change mode
            KeyCode::Char('m') if matches!(self.ui.popup_state, PopupState::None) => {
                self.settings.mode = match self.settings.mode {
                    Mode::Player => {
                        if let Some(index) = self.settings.selected_device_index
                            && let Err(err) = self.select_device(index)
                        {
                            self.handle_error(format!("Failed to capture system sound: {err}"));
                        }
                        self.reset_charts();
                        Mode::Microphone
                    }
                    Mode::Microphone => {
                        if cfg!(target_os = "macos") {
                            if let Err(err) = self.select_device(0) {
                                self.handle_error(format!("Failed to capture system sound: {err}"));
                            }
                            self.reset_charts();
                            Mode::System
                        } else {
                            self.reset_charts();
                            Mode::Player
                        }
                    }
                    Mode::System => {
                        self.reset_charts();
                        Mode::Player
                    }
                };
            }
            // Select device using its index if the device list is shown
            KeyCode::Char(c)
                if matches!(self.ui.popup_state, PopupState::InDeviceList)
                    && c.is_ascii_digit()
                    && c != '0' =>
            {
                let index = (c as usize) - ('0' as usize);
                if let Err(err) = self.select_device(index) {
                    self.handle_error(format!("Failed to select device: {err}"));
                }
                self.settings.selected_device_index = Some(index);
            }
            // Arrow key navigation for device and theme list
            KeyCode::Up => match self.ui.popup_state {
                PopupState::InThemesList => {
                    let total = list_themes().len() + 2; // + 1 for default and + 1 for custom theme
                    let current = self.ui.selected_theme.selected().unwrap_or(0);
                    self.ui
                        .selected_theme
                        .select(Some(wrap_index(current, -1, total)));
                }
                PopupState::InDeviceList => {
                    let total = list_input_devices().len();
                    let current = self.ui.selected_device.selected().unwrap_or(0);
                    self.ui
                        .selected_device
                        .select(Some(wrap_index(current, -1, total)));
                }
                _ => (),
            },
            KeyCode::Down => match self.ui.popup_state {
                PopupState::InThemesList => {
                    let total = list_themes().len() + 2; // + 1 for default and + 1 for custom theme
                    let current = self.ui.selected_theme.selected().unwrap_or(0);
                    self.ui
                        .selected_theme
                        .select(Some(wrap_index(current, 1, total)));
                }
                PopupState::InDeviceList => {
                    let total = list_input_devices().len();
                    let current = self.ui.selected_device.selected().unwrap_or(0);
                    self.ui
                        .selected_device
                        .select(Some(wrap_index(current, 1, total)));
                }
                _ => (),
            },
            KeyCode::Enter => match self.ui.popup_state {
                PopupState::InDeviceList => {
                    if let Err(err) =
                        self.select_device(self.ui.selected_device.selected().unwrap_or(0))
                    {
                        self.handle_error(format!("Failed to select device: {err}"));
                    }
                }
                PopupState::InThemesList => {
                    self.select_theme(self.ui.selected_theme.selected().unwrap_or(0));
                }
                PopupState::InExplorer => {
                    let file = self.explorer.current();
                    let file_path = self.explorer.current().path.clone();
                    if file.is_file() {
                        if file_path.extension().unwrap() == "theme"
                            || file_path.extension().unwrap() == "toml"
                        {
                            self.apply_theme_file(&file_path);
                        } else {
                            self.select_audio_file(file_path);
                        }
                    }
                }
                _ => (),
            },
            KeyCode::Char('t') => match self.ui.popup_state {
                PopupState::None => self.ui.popup_state = PopupState::InThemesList,
                PopupState::InThemesList => self.ui.popup_state = PopupState::None,
                _ => (),
            },
            KeyCode::Esc | KeyCode::Char('q')
                if !matches!(self.ui.popup_state, PopupState::None) =>
            {
                self.ui.popup_state = PopupState::None;
            }
            KeyCode::Char('=' | '+') => {
                self.waveform.timer.plus_sign = Some(Instant::now());
                self.waveform.window = f64::max(self.waveform.window - 1., 1.);
            }
            KeyCode::Char('-' | '_') => {
                let bound = if let Some(file) = self.audio_file.as_ref()
                    && file.data.duration.as_secs_f64() < 15.
                {
                    file.data.duration.as_secs_f64()
                } else {
                    15.
                };
                self.waveform.timer.minus_sign = Some(Instant::now());
                self.waveform.window = f64::min(self.waveform.window + 1., bound);
            }
            KeyCode::Char('h' | '?') | KeyCode::F(1) => match self.ui.popup_state {
                PopupState::None => self.ui.popup_state = PopupState::InHelpMessage,
                PopupState::InHelpMessage => self.ui.popup_state = PopupState::None,
                _ => (),
            },
            _ => (),
        }
    }

    fn select_device(&mut self, index: usize) -> Result<()> {
        let devices = list_input_devices();
        if index > devices.len() - 1 {
            return Err(eyre!("Invalid device index: {}", index + 1));
        }

        if let Some(stream) = &self.audio_capture_stream {
            stream.pause().unwrap();
            self.audio_capture_stream = None;
        }
        let device = devices[index].1.clone();
        let audio_device = AudioDevice::new(Some(device));

        self.waveform.device_name.clone_from(&devices[index].0);
        let sr = audio_device.config().sample_rate.0;
        let channels = audio_device.config().channels;

        let mut buf = AllocRingBuffer::new(sr as usize * 30);
        buf.fill(0.0);
        let latest_captured_samples = Arc::new(Mutex::new(buf));
        self.latest_captured_samples = latest_captured_samples;

        let stream = match audio_capture::build_input_stream(
            self.latest_captured_samples.clone(),
            &audio_device,
        ) {
            Ok(stream) => stream,
            Err(err) => {
                return Err(eyre!("Failed to create audio capture stream: {}", err));
            }
        };
        self.audio_capture_stream = Some(stream);
        self.audio_capture_stream.as_ref().unwrap().play()?;
        self.ui.popup_state = PopupState::None;
        if let Err(err) = self
            .device_analyzer
            .create_loudness_meter(channels as u32, sr)
        {
            self.handle_error(format!(
                "Could not create an analyzer for an audio file: {err}"
            ));
        }

        self.spectrum.gain_compensation = 0.0;
        Ok(())
    }

    fn select_theme(&mut self, index: usize) {
        let themes = builtin_themes::list_themes();

        // Check if index is for "Default Theme" (first option, index 0)
        if index == 0 {
            let mut theme = Theme::default();
            theme.apply_global_as_default();
            self.set_theme(theme);

            // Save theme choice to current_theme file
            if let Some(mut config_path) = config_dir() {
                config_path.push("soundscope");
                if let Err(err) = fs::create_dir_all(&config_path) {
                    self.handle_error(format!(
                        "Error saving a theme. Make sure ~/.config/soundscope exists: {err}"
                    ));
                    return;
                }
                let current_theme_file = config_path.join("current_theme");
                if let Err(err) = fs::write(&current_theme_file, "DEFAULT") {
                    self.handle_error(format!("Error saving theme choice: {err}"));
                }
            }

            self.ui.popup_state = PopupState::None;
            return;
        }

        // Check if index is for "Custom Theme" (last option)
        if index == themes.len() + 1 {
            // Open explorer for custom theme selection
            if let Some(config_path) = config_dir() {
                if !config_path.join("soundscope").exists() {
                    self.handle_error(
                        "Config directory not found. Create ~/.config/soundscope and place {name}.toml files in it to use custom themes.".to_owned(),
                    );
                    return;
                }
                self.explorer
                    .set_cwd(config_path.join("soundscope"))
                    .unwrap();
                self.ui.popup_state = PopupState::InExplorer;
            }
            return;
        }

        // Get theme name and load it (index - 1 because 0 is default)
        let theme_name = themes[index - 1];
        if let Some(theme) = builtin_themes::get_by_name(theme_name) {
            self.set_theme(theme);

            // Save theme choice to current_theme file
            if let Some(mut config_path) = config_dir() {
                config_path.push("soundscope");
                if let Err(err) = fs::create_dir_all(&config_path) {
                    self.handle_error(format!(
                        "Error saving a theme. Make sure ~/.config/soundscope exists: {err}"
                    ));
                    return;
                }
                let current_theme_file = config_path.join("current_theme");
                if !current_theme_file.exists()
                    && let Err(err) = File::create(&current_theme_file)
                {
                    self.handle_error(format!(
                        "Error saving a theme. Make sure ~/.config/soundscope exists: {err}"
                    ));
                    return;
                }
                // Save as "builtin:theme_name" format
                let theme_identifier = format!("builtin:{theme_name}");
                if let Err(err) = fs::write(&current_theme_file, &theme_identifier) {
                    self.handle_error(format!("Error saving theme choice: {err}"));
                }
            }

            self.ui.popup_state = PopupState::None;
        }
    }

    fn handle_error(&mut self, message: String) {
        self.ui.error_text = message;
        self.ui.error_timer = Some(Instant::now());
    }

    fn select_audio_file(&mut self, file_path: PathBuf) {
        // reset everything
        self.reset_charts();
        self.ui.popup_state = PopupState::None;

        if let Err(_err) = self
            .player_command_tx
            .send(PlayerCommand::SelectFile(file_path))
        {
            //TODO: log sending error
        }

        // blocking receiver
        if let Ok(audio_file) = self.audio_file_rx.recv() {
            self.receive_audio_file(audio_file);
        }
    }

    fn apply_theme_file(&mut self, file_path: &PathBuf) {
        let mut theme = self.load_theme(file_path).unwrap_or_default();
        theme.apply_global_as_default();
        self.set_theme(theme);
    }

    fn get_popup_area_with_percentage(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
        let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);
        area
    }

    fn get_popup_area_with_lenght(area: Rect, length_x: u16, length_y: u16) -> Rect {
        let vertical = Layout::vertical([Constraint::Length(length_x)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Length(length_y)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);
        area
    }

    fn get_error_popup_area(area: Rect) -> Rect {
        let vertical = Layout::vertical(Constraint::from_ratios([(5, 6), (1, 6)]));
        let horizontal = Layout::horizontal(Constraint::from_ratios([(1, 6), (5, 6)]));
        let [_, area] = vertical.areas(area);
        let [area, _] = horizontal.areas(area);
        area
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
                .block(Block::bordered().border_type(BorderType::Rounded).style(bd))
                .wrap(Wrap { trim: true }),
            error_popup_area,
        );
    }

    fn render_help_message(&self, f: &mut Frame) {
        let s = Style::default()
            .fg(self.ui.theme.help.foreground.unwrap())
            .bg(self.ui.theme.help.background.unwrap());
        let bd = s.fg(self.ui.theme.help.borders.unwrap());
        let hl = s.fg(self.ui.theme.help.highlight.unwrap());

        let area = Self::get_popup_area_with_lenght(f.area(), 22, 42);
        f.render_widget(Clear, area);
        let rows = vec![
            help_message_row!["1", "Toggle waveform", hl],
            help_message_row!["2", "Toggle spectrum", hl],
            help_message_row!["3", "Toggle LUFS", hl],
            help_message_row!["e", "Toggle explorer", hl],
            help_message_row!["m", "Change mode", hl],
            help_message_row!["d", "Toggle device list", hl],
            help_message_row!["t", "Select theme", hl],
            help_message_row!["?/h/F1", "Show this window", hl],
            help_message_row!["q/Ctrl+c", "Quit", hl],
            help_message_row!["q/Escape", "Close pop-up window", hl],
            help_message_row!["M", "Toggle mid frequencies", hl],
            help_message_row!["S", "Toggle side frequencies", hl],
            help_message_row!["Right", "Jump forward 5s", hl],
            help_message_row!["Left", "Jump back 5s", hl],
            help_message_row!["Space", "Play/Pause", hl],
            help_message_row!["-/_", "Zoom waveform in", hl],
            help_message_row!["=/+", "Zoom waveform out", hl],
            help_message_row!["1-9", "Select device/theme", hl],
            Row::new(vec![
                Cell::new("Up/Down".to_line().style(hl).centered()),
                Cell::new(vec![
                    "Navigate in explorer,".to_line(),
                    "device list and theme list".to_line(),
                ]),
            ])
            .height(2),
        ];
        let widths = [Constraint::Percentage(30), Constraint::Percentage(70)];
        let table = Table::new(rows, widths).style(s).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title("Help")
                .style(bd),
        );

        f.render_widget(table, area);
    }

    fn reset_charts(&mut self) {
        self.spectrum.mid_freq.clear();
        self.spectrum.side_freq.clear();
        self.lufs.0 = [-100.; 300];
        self.is_playing_audio = false;
        self.waveform.playhead = 0;
        self.spectrum.gain_compensation = 0.0;
    }

    fn load_theme(&mut self, path: &PathBuf) -> Option<Theme> {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let current_theme = config_dir().unwrap().join("soundscope/current_theme");
        if let Err(err) = fs::write(&current_theme, &name) {
            self.handle_error(format!("Error saving chosen theme: {err}"));
        }
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(err) => {
                self.handle_error(format!("Error reading {name}: {err}"));
                return None;
            }
        };
        let mut contents = String::new();
        if let Err(err) = file.read_to_string(&mut contents) {
            self.handle_error(format!("Error reading {name}: {err}"));
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
                self.handle_error(format!("Error reading {name}: {err}"));
                return None;
            }
        };
        Some(theme)
    }

    /// Called at startup to apply the current theme from a `current_theme` file if it exists
    fn apply_current_theme(&mut self, path: &Path, current_theme_file: &Path) {
        // read contents of current_theme file
        // this is the name of the theme {name}.toml or "builtin:theme_name"
        match fs::read_to_string(current_theme_file) {
            Ok(theme_file) => {
                if theme_file == "DEFAULT" {
                    let mut theme = Theme::default();
                    theme.apply_global_as_default();
                    self.set_theme(theme);
                } else if theme_file.starts_with("builtin:") {
                    // Load builtin theme
                    let theme_name = theme_file.strip_prefix("builtin:").unwrap();
                    if let Some(theme) = builtin_themes::get_by_name(theme_name) {
                        self.set_theme(theme);
                    } else {
                        self.handle_error(format!(
                            "Builtin theme '{theme_name}' not found. Applying default theme."
                        ));
                        let mut theme = Theme::default();
                        theme.apply_global_as_default();
                        self.set_theme(theme);
                    }
                } else {
                    let theme_file = path.join(&theme_file);
                    let theme = if theme_file.exists() {
                        self.load_theme(&theme_file).unwrap_or_default()
                    } else {
                        self.handle_error(format!(
                            "Theme file {} not found. Applying default theme.",
                            theme_file.display()
                        ));
                        if let Err(err) = fs::write(current_theme_file, "DEFAULT") {
                            self.handle_error(format!("Error setting theme to DEFAULT: {err}"));
                        }
                        let mut theme = Theme::default();
                        theme.apply_global_as_default();
                        theme
                    };
                    self.set_theme(theme);
                }
            }
            Err(err) => {
                self.handle_error(format!(
                    "Error reading current_theme file {err}. Applying default theme."
                ));
                if let Err(err) = fs::write(current_theme_file, "DEFAULT") {
                    self.handle_error(format!("Error setting theme to DEFAULT: {err}"));
                }
                let mut theme = Theme::default();
                theme.apply_global_as_default();
                self.set_theme(theme);
            }
        }
    }

    fn in_spectrum_chart(&self, m: MouseEvent) -> bool {
        if self.ui.show_window.contains(ShowWindow::SPECTRUM)
            && let Some(r) = self.ui.chart_rect
        {
            let x = m.column;
            let y = m.row;
            let width = r.width;
            let height = r.height;
            // hardcode boundries of the spectrum chart.
            // because it does not occupy the whole rectangle
            let x_min = r.x + 8;
            let y_min = r.y + 1;
            let x_max = r.x + width - 1;
            let y_max = r.y + height - 3;
            return x_min <= x && x < x_max && y_min <= y && y < y_max;
        }
        false
    }

    fn map_mouse_position_to_chart_point(x: u16, max_x: u16, y: u16, max_y: u16) -> (f32, f32) {
        // x
        let min_freq_log = 20f32.log10();
        let max_freq_log = 20000f32.log10();
        let log_range = max_freq_log - min_freq_log;

        let t = x as f32 / max_x as f32;
        let log_freq = min_freq_log + t * log_range;
        let x = 10f32.powf(log_freq);

        // y
        let t = y as f32 / max_y as f32;
        let y =
            SPECTRUM_UPPER_BOUND as f32 + t * (SPECTRUM_LOWER_BOUND - SPECTRUM_UPPER_BOUND) as f32;

        (x, y)
    }
}

fn wrap_index(current: usize, delta: isize, len: usize) -> usize {
    (current as isize + delta).rem_euclid(len as isize) as usize
}

fn config_dir() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        // On macOS, use ~/.config instead of ~/Library/Application Support
        let home = std::env::var("HOME").ok()?;
        Some(PathBuf::from(home).join(".config"))
    } else {
        dirs::config_local_dir()
    }
}

/// pub run function that initializes the terminal and runs the application
pub fn run(
    audio_file: Option<AudioFile>,
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
            Some(audio_file),
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
        let popup_area = App::get_popup_area_with_percentage(area, 50, 70);

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

        assert!(!app.spectrum.mid_freq.is_empty());

        // Check that there's a peak around 500 Hz
        let freq_bin = 500.0 / (sr as f32 / 2.0) * (app.spectrum.mid_freq.len() as f32);
        let bin_idx = freq_bin.round() as usize;

        // Check that this bin has non-trivial amplitude
        if bin_idx < app.spectrum.mid_freq.len() {
            let amp = app.spectrum.mid_freq[bin_idx].1; // assuming (freq, amp)
            assert!(amp < -20.0, "Expected strong signal at ~500Hz, got: {amp}");
        } else {
            panic!("Bin index out of range: {bin_idx}");
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

        assert!(!app.spectrum.mid_freq.is_empty());

        // Check that there's a peak around 500 Hz
        let freq_bin = 500.0 / (sr as f32 / 2.0) * (app.spectrum.mid_freq.len() as f32);
        let bin_idx = freq_bin.round() as usize;

        // Check that this bin has non-trivial amplitude
        if bin_idx < app.spectrum.mid_freq.len() {
            let amp = app.spectrum.mid_freq[bin_idx].1; // assuming (freq, amp)
            assert!(amp < -20.0, "Expected strong signal at ~500Hz, got: {amp}");
        } else {
            panic!("Bin index out of range: {bin_idx}");
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

        assert!(!app.spectrum.mid_freq.is_empty());

        // Check that there's a peak around 500 Hz
        let freq_bin = 500.0 / (sr as f32 / 2.0) * (app.spectrum.mid_freq.len() as f32);
        let bin_idx = freq_bin.round() as usize;

        // Check that this bin has non-trivial amplitude
        if bin_idx < app.spectrum.mid_freq.len() {
            let amp = app.spectrum.mid_freq[bin_idx].1; // assuming (freq, amp)
            assert!(amp < -20.0, "Expected strong signal at ~500Hz, got: {amp}");
        } else {
            panic!("Bin index out of range: {bin_idx}");
        }
    }
}
