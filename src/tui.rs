use std::{
    fmt,
    os::unix::fs::FileTypeExt,
    sync::{Arc, Mutex},
};

use color_eyre::{Result, eyre::Ok};
use crossbeam::channel::Sender;
use ratatui::{
    DefaultTerminal,
    crossterm::event::{Event, KeyCode, read},
    layout::Flex,
    prelude::*,
    widgets::{Axis, Block, Chart, Clear, Dataset},
};
use ratatui_explorer::{FileExplorer, Theme};

use crate::{
    audio_player::{AudioFile, AudioPlayer, PlayerCommand},
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

struct App {
    explorer: FileExplorer,
    // audio_file: Arc<Mutex<AudioFile>>,
    tui_tx: Sender<PlayerCommand>,
    show_explorer: bool,
    selected_file: Option<String>,
    data1: Vec<(f64, f64)>,
}

impl App {
    fn new(
        explorer: FileExplorer,
        // audio_file: Arc<Mutex<AudioFile>>,
        tui_tx: Sender<PlayerCommand>,
    ) -> Self {
        Self {
            explorer,
            // audio_file,
            tui_tx,
            show_explorer: false,
            selected_file: None,
            data1: vec![(0., 0.); 0],
        }
    }

    fn draw(&mut self, f: &mut Frame) {
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
            f.render_widget(&self.explorer.widget(), area);
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

    fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|f| self.draw(f));

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

    fn select_file(&mut self) {
        let file = self.explorer.current();
        let file_name = self.explorer.current().name();
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

pub fn run(tui_player: AudioPlayer, tui_tx: Sender<PlayerCommand>) -> Result<()> {
    // let audio_file = tui_player.audio_file;
    let terminal = ratatui::init();
    let theme = Theme::default()
        .add_default_title()
        .with_item_style(Style::default().fg(Color::Black));
    let file_explorer = FileExplorer::with_theme(theme)?;
    let app_result = App::new(file_explorer, tui_tx).run(terminal);
    ratatui::restore();
    app_result
}
