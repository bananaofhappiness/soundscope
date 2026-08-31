use ratatui::{
    Frame,
    layout::Rect,
    style::{Style, Stylize},
    symbols,
    text::{Line, Span, ToSpan},
    widgets::{Axis, Block, BorderType, Chart, Dataset, GraphType},
};

pub const SPECTRUM_TARGET_DBFS: f32 = -13.0;
pub const SPECTRUM_LOWER_BOUND: f64 = -100.0;
pub const SPECTRUM_UPPER_BOUND: f64 = -18.0;

use crate::tui::theme::SpectrumTheme;

/// Spectrum
///  data for the UI.
pub struct Spectrum {
    pub mid_freq: Vec<(f64, f64)>,
    pub side_freq: Vec<(f64, f64)>,
    /// Gain compensation in dB to normalize track to target LUFS
    pub gain_compensation: f32,
    pub show_mid_freq: bool,
    pub show_side_freq: bool,
}

impl Default for Spectrum {
    fn default() -> Self {
        Self {
            mid_freq: Vec::with_capacity(20_000),
            side_freq: Vec::with_capacity(20_000),
            gain_compensation: 0.0,
            show_mid_freq: true,
            show_side_freq: false,
        }
    }
}

impl Spectrum {
    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &SpectrumTheme) {
        let s = Style::default().bg(theme.background.unwrap());
        let fg = s.fg(theme.axes_labels.unwrap());
        let ax = s.fg(theme.axes.unwrap());
        let lb = s.fg(theme.labels.unwrap());
        let bd = s.fg(theme.borders.unwrap());
        let mf = s.fg(theme.mid_freq.unwrap());
        let sf = s.fg(theme.side_freq.unwrap());
        let hl = s.fg(theme.highlight.unwrap());
        let x_labels = vec![
            Span::styled("20Hz", fg),
            Span::styled("632.46Hz", fg),
            Span::styled("20kHz", fg),
        ];

        let gain_comp = self.gain_compensation as f64;

        let mid_freq_normalized: Vec<(f64, f64)> = if self.show_mid_freq {
            self.mid_freq
                .iter()
                .map(|(x, y)| (*x, y + gain_comp))
                .collect()
        } else {
            vec![(-1000.0, -1000.0)]
        };

        let side_freq_normalized: Vec<(f64, f64)> = if self.show_side_freq {
            self.side_freq
                .iter()
                .map(|(x, y)| (*x, y + gain_comp))
                .collect()
        } else {
            vec![(-1000.0, -1000.0)]
        };

        let datasets = vec![
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Area)
                .style(mf)
                .fill_to_y(SPECTRUM_LOWER_BOUND)
                .data(&mid_freq_normalized),
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Area)
                .style(sf)
                .fill_to_y(SPECTRUM_LOWER_BOUND)
                .data(&side_freq_normalized),
        ];

        let chart = Chart::new(datasets)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .style(bd)
                    .title(vec![
                        "²".to_span().style(hl).bold(),
                        "spectrum".to_span().style(lb).bold(),
                    ])
                    .title({
                        let mut mid = if self.show_mid_freq {
                            vec![
                                "M".to_span().style(hl).bold(),
                                "id".to_span().bold(),
                                "/".to_span(),
                            ]
                        } else {
                            vec!["M".to_span().style(hl), "id".to_span(), "/".to_span()]
                        };
                        let mut side = if self.show_side_freq {
                            vec!["S".to_span().style(hl).bold(), "ide".to_span().bold()]
                        } else {
                            vec!["S".to_span().style(hl), "ide".to_span()]
                        };
                        mid.append(&mut side);
                        Line::from(mid).right_aligned()
                    }),
            )
            .x_axis(
                Axis::default()
                    .title("Hz")
                    .labels(x_labels)
                    .style(ax)
                    .bounds([0., 100.]),
            )
            .y_axis(
                Axis::default()
                    .title("dBFS")
                    .labels(vec![
                        Span::raw(SPECTRUM_LOWER_BOUND.to_string()).style(fg),
                        Span::raw(
                            ((SPECTRUM_UPPER_BOUND + SPECTRUM_LOWER_BOUND) / 2f64).to_string(),
                        )
                        .style(fg),
                        Span::raw(SPECTRUM_UPPER_BOUND.to_string()).style(fg),
                    ])
                    .style(ax)
                    .bounds([SPECTRUM_LOWER_BOUND, SPECTRUM_UPPER_BOUND]),
            )
            .style(s);

        frame.render_widget(chart, area);
    }
}
