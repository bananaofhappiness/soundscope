use ratatui::style::Color;
use serde::Deserialize;

/// Defines theme using .toml file
/// Otherwise, uses default values.
#[derive(Debug, Deserialize, Default)]
pub struct Theme {
    pub global: GlobalTheme,
    pub waveform: WaveformTheme,
    pub spectrum: SpectrumTheme,
    pub lufs: LufsTheme,
    pub devices: DeviceListTheme,
    pub explorer: ExplorerTheme,
    pub error: ErrorTheme,
    pub help: HelpMessageTheme,
}

/// Uses [fill] to conviniently fill all fields of a struct.
macro_rules! fill_fields {
    ($self:ident.$section:ident.$($field:ident <- $value:expr),* $(,)?) => {
        $( fill(&mut $self.$section.$field, $value); )*
    };
}

/// Used to set `default: T` to a `field` if it is not set (it is None).
/// Used in [`fill_fields`] macro
fn fill<T>(field: &mut Option<T>, default: T) {
    if field.is_none() {
        *field = Some(default);
    }
}

impl Theme {
    /// Sets `self.global.foreground` and `self.global.background` for every field that was not defined in a theme file.
    pub fn apply_global_as_default(&mut self) {
        let fg = self.global.foreground;
        let bg = self.global.background;
        self.global.highlight = self.global.highlight.or(Some(fg));
        let hl = self.global.highlight.unwrap();

        fill_fields!(self.waveform.
            borders <- fg,
            controls <- fg,
            controls_highlight <- hl,
            labels <- fg,
            playhead <- hl,
            current_time <- fg,
            total_duration <- fg,
            waveform <- fg,
            background <- bg,
            highlight <- hl,
        );

        fill_fields!(self.lufs.
            axis <- fg,
            chart <- fg,
            foreground <- fg,
            labels <- fg,
            numbers <- fg,
            borders <- fg,
            background <- bg,
            highlight <- hl,
        );

        fill_fields!(self.spectrum.
            axes <- fg,
            axes_labels <- fg,
            borders <- fg,
            labels <- fg,
            mid_freq <- fg,
            side_freq <- hl,
            background <- bg,
            highlight <- hl,
        );

        fill_fields!(self.explorer.
            background <- bg,
            borders <- fg,
            dir_foreground <- fg,
            item_foreground <- fg,
            highlight_dir_foreground <- hl,
            highlight_item_foreground <- hl,
        );

        fill_fields!(self.devices.
            background <- bg,
            foreground <- fg,
            borders <- fg,
            highlight <- hl,
        );

        fill_fields!(self.error.
            background <- bg,
            foreground <- fg,
            borders <- fg,
        );

        fill_fields!(self.help.
            background <- bg,
            foreground <- fg,
            borders <- fg,
            highlight <- hl,
        );
    }
}

/// Used to set default values of every UI element if they are not specified in the config file.
#[derive(Debug, Deserialize)]
pub struct GlobalTheme {
    pub background: Color,
    /// It is default value for everything that is not a background
    pub foreground: Color,
    /// Color used to highlight corresponding characters
    /// Like highlighting L in LUFS to let the user know
    /// that pressing L will open the LUFS meter
    pub highlight: Option<Color>,
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
#[derive(Debug, Deserialize, Default)]
pub struct WaveformTheme {
    pub borders: Option<Color>,
    pub waveform: Option<Color>,
    pub playhead: Option<Color>,
    /// Current playing time and total duration
    pub current_time: Option<Color>,
    pub total_duration: Option<Color>,
    /// Buttons like <-, +, -, ->
    pub controls: Option<Color>,
    pub controls_highlight: Option<Color>,
    pub labels: Option<Color>,
    /// Background of the chart
    pub background: Option<Color>,
    pub highlight: Option<Color>,
}

/// Used to define the theme for the Spectrum display.
#[derive(Debug, Deserialize, Default)]
pub struct SpectrumTheme {
    pub borders: Option<Color>,
    /// Frequencies and LUFS tabs text
    pub labels: Option<Color>,
    pub axes: Option<Color>,
    pub axes_labels: Option<Color>,
    pub mid_freq: Option<Color>,
    pub side_freq: Option<Color>,
    /// Background of the chart
    pub background: Option<Color>,
    pub highlight: Option<Color>,
}

/// Used to define the theme for the LUFS display.
#[derive(Debug, Deserialize, Default)]
pub struct LufsTheme {
    pub axis: Option<Color>,
    pub chart: Option<Color>,
    /// Frequencies and LUFS tabs text
    pub labels: Option<Color>,
    /// Text color on the left
    pub foreground: Option<Color>,
    /// Color of the numbers on the left
    pub numbers: Option<Color>,
    pub borders: Option<Color>,
    /// Background of the chart
    pub background: Option<Color>,
    pub highlight: Option<Color>,
}

/// Used to define the theme for the devices list.
#[derive(Debug, Deserialize, Default)]
pub struct DeviceListTheme {
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub borders: Option<Color>,
    pub highlight: Option<Color>,
}

/// Used to define the theme for the explorer.
#[derive(Debug, Deserialize, Default)]
pub struct ExplorerTheme {
    pub background: Option<Color>,
    pub borders: Option<Color>,
    pub item_foreground: Option<Color>,
    pub highlight_item_foreground: Option<Color>,
    pub dir_foreground: Option<Color>,
    pub highlight_dir_foreground: Option<Color>,
}

/// Used to define the theme for the error popup.
#[derive(Debug, Deserialize)]
pub struct ErrorTheme {
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub borders: Option<Color>,
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

/// Used to define the theme for the devices list.
#[derive(Debug, Deserialize, Default)]
pub struct HelpMessageTheme {
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub borders: Option<Color>,
    pub highlight: Option<Color>,
}

#[test]
fn test_fill_macro() {
    let mut theme = Theme {
        global: GlobalTheme::default(),
        waveform: WaveformTheme::default(),
        spectrum: SpectrumTheme::default(),
        lufs: LufsTheme::default(),
        devices: DeviceListTheme::default(),
        explorer: ExplorerTheme::default(),
        error: ErrorTheme::default(),
        help: HelpMessageTheme::default(),
    };
    theme.global.foreground = Color::LightCyan;
    theme.global.background = Color::Magenta;

    theme.spectrum.mid_freq = None;
    theme.spectrum.side_freq = None;
    theme.spectrum.labels = None;

    theme.waveform.playhead = None;
    theme.waveform.highlight = None;
    theme.waveform.current_time = None;

    theme.lufs.numbers = None;

    theme.devices.background = None;

    theme.explorer.highlight_dir_foreground = None;
    theme.explorer.item_foreground = None;

    theme.apply_global_as_default();
    assert!(theme.spectrum.mid_freq == Some(Color::LightCyan));
    assert!(theme.spectrum.side_freq == Some(Color::Indexed(160)));
    assert!(theme.spectrum.labels == Some(Color::LightCyan));

    assert!(theme.waveform.playhead == Some(Color::Indexed(160)));
    assert!(theme.waveform.highlight == Some(Color::Indexed(160)));
    assert!(theme.waveform.current_time == Some(Color::LightCyan));

    assert!(theme.lufs.numbers == Some(Color::LightCyan));

    assert!(theme.devices.background == Some(Color::Magenta));

    assert!(theme.explorer.highlight_dir_foreground == Some(Color::Indexed(160)));
    assert!(theme.explorer.item_foreground == Some(Color::LightCyan));
}
