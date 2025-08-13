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
    analyzer,
    audio_player::{AudioFile, PlayerCommand, Samples},
};

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    NotAFile(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotAFile(path) => write!(f, "Path is not a file: {}", path),
        }
    }
}

impl std::error::Error for Error {}

struct App {
    explorer: FileExplorer,
    samples: Samples,
    tui_tx: Sender<PlayerCommand>,
    analyzer_rx: Receiver<usize>,
    show_explorer: bool,
    mid_fft_vec: Vec<(f64, f64)>,
    side_fft_vec: Vec<(f64, f64)>,
}

impl App {
    fn new(
        explorer: FileExplorer,
        tui_tx: Sender<PlayerCommand>,
        samples: Samples,
        analyzer_rx: Receiver<usize>,
    ) -> Self {
        Self {
            explorer,
            samples,
            tui_tx,
            analyzer_rx,
            show_explorer: false,
            mid_fft_vec: vec![(0., 0.); 0],
            side_fft_vec: vec![(0., 0.); 0],
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        let area = f.area();

        self.render_animated_chart(f, area);
        // render explorer
        if self.show_explorer {
            let area = Self::popup_area(area, 50, 70);
            f.render_widget(Clear, area);
            f.render_widget(&self.explorer.widget(), area);
        }
    }

    fn render_animated_chart(&mut self, frame: &mut Frame, area: Rect) {
        let x_labels = vec![
            Span::styled("20Hz", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("632Hz"),
            Span::styled("20kHz", Style::default().add_modifier(Modifier::BOLD)),
        ];
        let datasets = vec![
            Dataset::default()
                .name("Mid Frequency")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Green))
                .data(&self.mid_fft_vec),
            Dataset::default()
                .name("Side Frequency")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Yellow))
                .data(&self.side_fft_vec),
        ];

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

            // receive playback position
            if let Ok(pos) = self.analyzer_rx.try_recv() {
                let left_bound = pos.saturating_sub(16384);
                if left_bound != 0 {
                    let samples = &self.samples.read().unwrap()[left_bound..pos];

                    // get fft
                    (self.mid_fft_vec, self.side_fft_vec) = analyzer::get_fft(samples);
                }
            }

            // event reader
            if poll(Duration::from_micros(1))? {
                let event = read()?;
                if let Event::Key(key) = event {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('e') => self.show_explorer = !self.show_explorer,
                        KeyCode::Enter => self.select_file(),
                        KeyCode::Char(' ') => {
                            if let Err(err) = self.tui_tx.send(PlayerCommand::ChangeState) {
                                //do smth idk
                            }
                        }
                        _ => (),
                    }
                }
                if self.show_explorer {
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
        self.show_explorer = false;
        if let Err(err) = self.tui_tx.send(PlayerCommand::SelectFile(file_path)) {
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
    tui_tx: Sender<PlayerCommand>,
    analyzer_rx: Receiver<usize>,
    samples: Samples,
) -> Result<()> {
    let terminal = ratatui::init();
    let theme = Theme::default()
        .add_default_title()
        .with_item_style(Style::default().fg(Color::Black));
    let file_explorer = FileExplorer::with_theme(theme)?;
    let app_result = App::new(file_explorer, tui_tx, samples, analyzer_rx).run(terminal);
    ratatui::restore();
    app_result
}
