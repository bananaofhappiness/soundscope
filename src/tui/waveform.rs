use std::time::{Duration, Instant};

use ratatui::{
    Frame,
    layout::Rect,
    style::{Style, Stylize},
    symbols,
    text::{Line, ToSpan},
    widgets::{Axis, Block, BorderType, Chart, Dataset, GraphType},
};

use crate::{
    audio_player::AudioData,
    tui::{Mode, fmt_time, theme::WaveformTheme},
};

pub struct Timer {
    // Used to flash control elements when the button is pressed
    pub left_arrow: Option<Instant>,
    pub right_arrow: Option<Instant>,
    pub plus_sign: Option<Instant>,
    pub minus_sign: Option<Instant>,
}

/// Waveform data for the UI.
pub struct WaveForm {
    pub window: usize, // in seconds
    pub device_name: String,
    pub timer: Timer,
    pub waveform: Vec<(f64, f64)>,
    pub playhead: usize,
}

impl WaveForm {
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &WaveformTheme,
        audio_data: Option<&AudioData>,
        mode: &Mode,
    ) {
        // Match the number of waveform bins to the number of dot columns of
        // the chart. Minus 2 becasue the bordered block takes 2 cells
        // times 2 since Braille gives 2 dot columns per cell.
        let dots = usize::from(area.width.saturating_sub(2)) * 2;
        if dots >= 2 && self.waveform.len() != dots * 2 {
            self.waveform = vec![(0., 0.); dots * 2];
        }
        let bins = self.waveform.len() / 2;
        let x_max = bins.saturating_sub(1).max(1) as f64;

        let s = Style::default().bg(theme.background.unwrap());
        let lb = s.fg(theme.labels.unwrap());
        let bd = s.fg(theme.borders.unwrap());
        let hl = s.fg(theme.highlight.unwrap());
        let pl = s.fg(theme.playhead.unwrap());
        let ct = s.fg(theme.current_time.unwrap());
        let td = s.fg(theme.total_duration.unwrap());
        let wv = s.fg(theme.waveform.unwrap());

        // playhead is just a function that looks like a vertical line
        let sample_rate = audio_data.map_or(44100, |d| d.sample_rate);
        let duration = audio_data.map_or(Duration::ZERO, |d| d.duration);

        let playhead_chart = if !matches!(mode, Mode::Player) {
            [(-1., -1.), (-1., -1.)]
        } else {
            // just copy-pasted from tui/mod.rs:959
            let pos_in_sec = self.playhead as f32 / sample_rate as f32;
            enum Pos {
                AtBeginning,
                AtEnd,
                InMiddle,
            }
            let relative_position = if pos_in_sec <= (self.window as f32 / 2.) {
                Pos::AtBeginning
            } else if pos_in_sec * 1000.
                >= (duration.as_millis() - (self.window * 1000 / 2) as u128) as f32
            {
                Pos::AtEnd
            } else {
                Pos::InMiddle
            };

            // smooth playhead scrolling
            let half_window = self.window as f32 / 2.;
            let center = self.waveform.len() as f64 / 4.;
            let playhead_x = match relative_position {
                Pos::AtBeginning => (pos_in_sec / half_window) as f64 * center,
                Pos::InMiddle => center,
                Pos::AtEnd => {
                    let secs_past_middle = pos_in_sec - (duration.as_secs_f32() - half_window);
                    center + (secs_past_middle / half_window) as f64 * (x_max - center)
                }
            };
            [(playhead_x, 1.), (playhead_x, -1.)]
        };

        // get current playback time in seconds
        let playhead_ms = (self.playhead as f64 / sample_rate as f64 * 1000.) as u64;
        let (current_time, total_duration) = match mode {
            Mode::Player => match audio_data {
                Some(data) => (
                    fmt_time(playhead_ms / 1000),
                    fmt_time(data.duration.as_secs()),
                ),
                None => (String::new(), String::new()),
            },
            _ => (String::new(), String::new()),
        };

        // make datasets
        // first one to render a waveform
        // the other one to render the playhead
        let datasets = vec![
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(wv)
                .data(&self.waveform),
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(pl)
                .data(&playhead_chart),
        ];

        // render chart
        let title = audio_data.map_or("", |data| data.title.as_str());
        let mode_text = mode.to_span().style(lb);
        let upper_right_title = match mode {
            Mode::Microphone => Line::from(vec![
                "d".bold().style(hl),
                "evice: ".to_span().style(lb),
                self.device_name.to_span().style(lb),
                " ".to_span(),
                "m".bold().style(hl),
                "ode: ".to_span().style(lb),
                mode_text,
            ])
            .right_aligned(),
            _ => Line::from(vec![
                "m".bold().style(hl),
                "ode: ".to_span().style(lb),
                mode_text,
            ])
            .right_aligned(),
        };

        // let x_min = 15000. - (self.window as usize * 1000) as f64 ;
        // build the chart widget
        let chart = Chart::new(datasets)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title("¹".to_span().style(hl).bold() + title.to_span().style(lb))
                    .title_bottom(self.get_flashing_controls_text(theme).left_aligned())
                    .title_bottom(Line::styled(current_time, ct).centered())
                    .title_bottom(Line::styled(total_duration, td).right_aligned())
                    .title(upper_right_title)
                    .style(bd),
            )
            .style(wv)
            .x_axis(Axis::default().bounds([0., x_max]))
            .y_axis(Axis::default().bounds([-1., 1.]));

        frame.render_widget(chart, area);
    }

    fn get_flashing_controls_text(&self, theme: &WaveformTheme) -> Line<'_> {
        let t = 100;
        let s = Style::default()
            .bg(theme.background.unwrap())
            .fg(theme.controls.unwrap());
        let hl = s.fg(theme.controls_highlight.unwrap());
        let left_arrow = match self.timer.left_arrow {
            Some(timer) if timer.elapsed().as_millis() < t => "<-".to_span().style(hl),
            _ => "<-".to_span().style(s),
        };
        let right_arrow = match self.timer.right_arrow {
            Some(timer) if timer.elapsed().as_millis() < t => "->".to_span().style(hl),
            _ => "->".to_span().style(s),
        };
        let minus = match self.timer.minus_sign {
            Some(timer) if timer.elapsed().as_millis() < t => "-".to_span().style(hl),
            _ => "-".to_span().style(s),
        };
        let plus = match self.timer.plus_sign {
            Some(timer) if timer.elapsed().as_millis() < t => "+".to_span().style(hl),
            _ => "+".to_span().style(s),
        };
        Line::from(vec![
            left_arrow,
            " ".to_span(),
            minus,
            " ".to_span(),
            format!("{:0>2}s", self.window.to_span().style(s)).into(),
            " ".to_span(),
            plus,
            " ".to_span(),
            right_arrow,
        ])
    }
}

impl Default for WaveForm {
    fn default() -> Self {
        Self {
            // resized to the chart area on the first render
            waveform: vec![(0., 0.); 1000],
            playhead: 0,
            window: 15,
            device_name: String::new(),
            timer: Timer {
                left_arrow: None,
                right_arrow: None,
                plus_sign: None,
                minus_sign: None,
            },
        }
    }
}
