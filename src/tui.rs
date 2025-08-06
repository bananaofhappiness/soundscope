use std::{
    fmt,
    sync::{Arc, Mutex},
};

use color_eyre::{Result, eyre::Ok};
use crossbeam::channel::Sender;
use ratatui::{
    DefaultTerminal,
    crossterm::event::{Event, KeyCode, read},
    layout::Flex,
    prelude::*,
    widgets::{Axis, Block, Borders, Chart, Clear, Dataset, Paragraph},
};
use ratatui_explorer::{FileExplorer, Theme};

use crate::{
    audio_player::{self, AudioFile, AudioReader, PlayerCommand},
    file_reader,
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

#[derive(Default)]
struct App {
    show_explorer: bool,
    selected_file: Option<String>,
    data1: Vec<(f64, f64)>,
    window: [f64; 2],
}

impl App {
    fn draw(&mut self, f: &mut Frame, explorer: &FileExplorer) {
        let area = f.area();

        //get filename and render it
        // let file_name = self.selected_file.get_or_insert("".to_string());
        // f.render_widget(
        //     Paragraph::new(file_name.to_owned()).block(Block::default().borders(Borders::all())),
        //     // Chart::new().block(Block::default().borders(Borders::all())),
        //     area,
        // );

        self.render_animated_chart(f, area);
        // render explorer
        if self.show_explorer {
            let area = Self::popup_area(area, 50, 70);
            f.render_widget(Clear, area);
            f.render_widget(&explorer.widget(), area);
        }
    }

    fn render_animated_chart(&mut self, frame: &mut Frame, area: Rect) {
        if self.selected_file.is_none() {
            return;
        }
        self.data1 = file_reader::read_file("VIRUS.mp3");
        // println!("{:?}", self.data1);

        // let x_labels = vec![
        //     Span::styled(
        //         format!("{}", self.window[0]),
        //         Style::default().add_modifier(Modifier::BOLD),
        //     ),
        //     Span::raw(format!("{}", (self.window[0] + self.window[1]) / 2.0)),
        //     Span::styled(
        //         format!("{}", self.window[1]),
        //         Style::default().add_modifier(Modifier::BOLD),
        //     ),
        // ];
        let datasets = vec![
            Dataset::default()
                .name("data")
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(Color::Black))
                .data(&self.data1),
            // .data(&[(0., 0.)]),
        ];

        let chart = Chart::new(datasets)
            .block(Block::bordered())
            .x_axis(
                Axis::default()
                    .title("X Axis")
                    .style(Style::default().fg(Color::Black))
                    // .labels(x_labels)
                    .labels(["0".bold(), "0".into(), "20000".bold()])
                    .bounds([0., 22050.]),
            )
            .y_axis(
                Axis::default()
                    .title("Y Axis")
                    .style(Style::default().fg(Color::Black))
                    .labels(["0".bold(), "0".into(), "10".bold()])
                    .bounds([0., 500.]),
            );

        frame.render_widget(chart, area);
    }

    fn run(
        mut self,
        mut terminal: DefaultTerminal,
        mut explorer: FileExplorer,
        audio_file: Arc<Mutex<AudioFile>>,
    ) -> Result<()> {
        loop {
            terminal.draw(|f| self.draw(f, &explorer))?;

            //event reader
            let event = read()?;
            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('e') => {
                        if !self.show_explorer {
                            self.selected_file = None;
                        }
                        self.show_explorer = !self.show_explorer
                    }
                    KeyCode::Enter => {
                        if let Err(err) = self.select_file(&explorer, &audio_file) {
                            //todo error handling
                        }
                    }
                    _ => (),
                }
            }
            if self.show_explorer {
                explorer.handle(&event)?;
            }
        }
    }

    fn select_file(
        &mut self,
        explorer: &FileExplorer,
        audio_file: &Arc<Mutex<AudioFile>>,
    ) -> Result<()> {
        let file = explorer.current();
        let file_name = explorer.current().name();
        let file_path = explorer.current().path().to_str().unwrap().to_owned();
        if !file.is_file() {
            return Err(Error::NotAFile(file_path).into());
        }
        audio_file.lock().unwrap().load_file(&file_path)?;
        self.show_explorer = false;
        Ok(())
    }

    fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
        let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);
        area
    }
}

pub fn run(tui_reader: AudioReader, tui_tx: Sender<PlayerCommand>) -> Result<()> {
    let audio_file = tui_reader.get_file();
    let terminal = ratatui::init();
    let theme = Theme::default()
        .add_default_title()
        .with_item_style(Style::default().fg(Color::Black));
    let file_explorer = FileExplorer::with_theme(theme)?;
    let app_result = App::default().run(terminal, file_explorer, audio_file);
    ratatui::restore();
    app_result
}
