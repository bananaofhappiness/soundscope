use eyre::Result;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    symbols,
    text::ToSpan,
    widgets::{Axis, Block, BorderType, Chart, Dataset, GraphType, Paragraph},
};

use crate::{analyzer::Analyzer, tui::theme::LufsTheme};

pub struct Lufs(pub [f64; 300]);

impl Default for Lufs {
    fn default() -> Self {
        Self([-100.; 300])
    }
}

impl Lufs {
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &LufsTheme,
        file_analyzer: &mut Analyzer,
    ) -> Result<()> {
        let s = Style::default().bg(theme.background.unwrap());
        let fg = s.fg(theme.foreground.unwrap());
        let ax = s.fg(theme.axis.unwrap());
        let hl = s.fg(theme.highlight.unwrap());
        let bd = s.fg(theme.borders.unwrap());
        let ch = s.fg(theme.chart.unwrap());
        let lb = s.fg(theme.labels.unwrap());
        let nb = s.fg(theme.numbers.unwrap());
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
            .split(area);
        let data = self
            .0
            .iter()
            .enumerate()
            .map(|(x, &y)| (x as f64, y))
            .collect::<Vec<(f64, f64)>>();

        let integrated_lufs = file_analyzer.get_integrated_lufs()?;

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
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
            ])
            .split(layout[1]);

        // get lufs text
        let integrated = format!("{integrated_lufs:05.1}");
        let short_term = format!("{:05.1}", self.0[299]);
        let integrated_lufs_text = integrated.to_span().style(nb) + " LUFS".to_span();
        let short_term_lufs_text = short_term.to_span().style(nb) + " LUFS".to_span();

        // get true peak
        let (tp_left, tp_right) = file_analyzer.get_true_peak()?;

        // get true peak text
        let left = format!("{tp_left:.1}");
        let right = format!("{tp_right:.1}");
        let left = left.to_span().style(nb);
        let right = right.to_span().style(nb);
        let true_peak_text = vec![
            "L: ".bold().style(fg) + left + " Db".bold().style(fg),
            "R: ".bold().style(fg) + right + " Db".bold().style(fg),
        ];

        //get range text
        let range = file_analyzer.get_loudness_range()?;
        let range_text = format!("{range:.1} LU");

        // paragraphs
        let lufs_paragraph = Paragraph::new(short_term_lufs_text)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .style(bd)
                    .title_alignment(Alignment::Center)
                    .title("Short term".bold()),
            )
            .alignment(Alignment::Center);
        let integrated_paragraph = Paragraph::new(integrated_lufs_text)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .style(bd)
                    .title_alignment(Alignment::Center)
                    .title("Integrated".bold()),
            )
            .alignment(Alignment::Center);
        let true_peak_paragraph = Paragraph::new(true_peak_text)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .style(bd)
                    .title_alignment(Alignment::Center)
                    .title("True Peak".bold()),
            )
            .alignment(Alignment::Center)
            .style(bd);
        let range_paragraph = Paragraph::new(range_text)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .style(bd)
                    .title_alignment(Alignment::Center)
                    .title("Range".bold()),
            )
            .alignment(Alignment::Center)
            .style(bd);

        // chart section
        let dataset = vec![
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Area)
                .style(ch)
                .fill_to_y(-50.0)
                .data(&data),
        ];
        let chart = Chart::new(dataset)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .style(bd)
                    .title(vec![
                        "³".to_span().style(hl).bold(),
                        "lufs".to_span().style(lb).bold(),
                    ]),
            )
            .x_axis(Axis::default().bounds([0., 300.]).style(ax))
            .y_axis(
                Axis::default()
                    .bounds([-50., 0.])
                    .labels(["-50".bold(), "0".bold()])
                    .style(ax),
            )
            .style(s);
        frame.render_widget(lufs_paragraph, paragraph_layout[0]);
        frame.render_widget(integrated_paragraph, paragraph_layout[1]);
        frame.render_widget(range_paragraph, paragraph_layout[2]);
        frame.render_widget(true_peak_paragraph, paragraph_layout[3]);

        frame.render_widget(chart, layout[0]);
        Ok(())
    }
}
