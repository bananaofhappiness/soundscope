use std::time::Instant;

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
    tui::{Mode, theme::WaveformTheme},
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
    pub window: f64,
    pub device_name: String,
    pub timer: Timer,
    pub audio_file_chart: Vec<(f64, f64)>,
    pub microphone_input_chart: Vec<(f64, f64)>,
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
        let samples_in_one_ms = sample_rate / 1000;

        let playhead_chart = if !matches!(mode, Mode::Player) {
            [(-1., -1.), (-1., -1.)]
        } else {
            let playhead_x = self.playhead as f64 / samples_in_one_ms as f64;
            [(playhead_x, 1.), (playhead_x, -1.)]
        };

        // get current playback time in seconds
        let playhead_ms = (self.playhead as f64 / sample_rate as f64 * 1000.) as u64;
        let fmt_time = |secs: u64| format!("{:0>2}:{:0>2}", secs / 60, secs % 60);
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

        let (x_min, x_max) = match mode {
            Mode::Microphone | Mode::System => {
                let window_millis = self.window as usize * 1000;
                (15000. - window_millis as f64, 15000.)
            }
            Mode::Player => {
                let half_window = self.window * 500.;
                let playhead_millis = playhead_ms as f64;
                let max_x = self.audio_file_chart.len() as f64 / 2.;
                let min_bound = (playhead_millis - half_window)
                    .min(max_x - self.window * 1000.)
                    .max(0.);
                let max_bound = (playhead_millis + half_window)
                    .min(max_x)
                    .max(self.window * 1000.);
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
                .data({
                    if matches!(mode, Mode::Player) {
                        &self.audio_file_chart
                    } else {
                        &self.microphone_input_chart
                    }
                }),
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
            .x_axis(Axis::default().bounds([x_min, x_max]))
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
            audio_file_chart: vec![(0., 0.)],
            microphone_input_chart: vec![(0., 0.)],
            playhead: 0,
            window: 15.,
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
