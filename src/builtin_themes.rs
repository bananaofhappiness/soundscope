//! Built-in themes for SoundScope
//!
//! This module contains pre-defined themes for popular color schemes.

use crate::tui::{GlobalTheme, Theme};
use ratatui::style::Color;

#[allow(dead_code)]
// Helper function to convert hex RGB to Color::Rgb
const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

/// Catppuccin Mocha theme
///
/// A soothing pastel theme with warm, cozy colors.
/// Based on [Catppuccin](https://github.com/catppuccin/catppuccin)
pub fn catppuccin_mocha() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(30, 30, 46),         // #1e1e2e
            foreground: rgb(205, 214, 244),      // #cdd6f4
            highlight: Some(rgb(203, 166, 247)), // #cba6f7 (mauve)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// Catppuccin Macchiato theme
///
/// A soothing pastel theme, slightly lighter than Mocha.
/// Based on [Catppuccin](https://github.com/catppuccin/catppuccin)
pub fn catppuccin_macchiato() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(36, 39, 58),         // #24273a
            foreground: rgb(202, 211, 245),      // #cad3f5
            highlight: Some(rgb(183, 189, 248)), // #b7bdf8 (mauve)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// Catppuccin Frappé theme
///
/// A soothing pastel theme, darker and more muted.
/// Based on [Catppuccin](https://github.com/catppuccin/catppuccin)
pub fn catppuccin_frappe() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(48, 52, 70),         // #303446 (base)
            foreground: rgb(198, 208, 245),      // #c6d0f5 (text)
            highlight: Some(rgb(202, 158, 230)), // #ca9ee6 (mauve)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// Catppuccin Latte theme
///
/// A soothing pastel theme with light, warm colors.
/// Based on [Catppuccin](https://github.com/catppuccin/catppuccin)
pub fn catppuccin_latte() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(239, 241, 245),     // #eff1f5 (base)
            foreground: rgb(76, 79, 105),       // #4c4f69 (text)
            highlight: Some(rgb(136, 57, 239)), // #8839ef (mauve)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// Dracula theme
///
/// A dark theme with high contrast and vibrant accent colors.
/// Based on [Dracula Theme](https://draculatheme.com/)
pub fn dracula() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(40, 42, 54),         // #282a36
            foreground: rgb(248, 248, 242),      // #f8f8f2
            highlight: Some(rgb(189, 147, 249)), // #bd93f9 (purple)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// Gruvbox Dark theme
///
/// A warm, retro theme designed to be easy on the eyes.
/// Based on [Gruvbox](https://github.com/morhetz/gruvbox)
pub fn gruvbox_dark() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(40, 40, 40),        // #282828
            foreground: rgb(235, 219, 178),     // #ebdbb2
            highlight: Some(rgb(254, 128, 25)), // #fe8019 (orange)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// Monokai theme
///
/// A classic dark theme with vibrant colors and high contrast.
/// Originally from TextMate editor.
pub fn monokai() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(39, 40, 34),        // #272822
            foreground: rgb(248, 248, 242),     // #f8f8f2
            highlight: Some(rgb(249, 38, 114)), // #f92672 (pink/red)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// Nord theme
///
/// An arctic, north-bluish color palette with a cold and clean look.
/// Based on [Nord](https://github.com/arcticicestudio/nord)
pub fn nord() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(46, 52, 64),         // #2E3440 (nord0)
            foreground: rgb(216, 222, 233),      // #D8DEE9 (nord4)
            highlight: Some(rgb(136, 192, 208)), // #88C0D0 (nord8 - frost)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// Material Design Dark theme
///
/// Based on Google's Material Design dark theme specifications.
pub fn material_dark() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(38, 50, 56),       // #263238 (blue grey 900)
            foreground: rgb(236, 239, 241),    // #ECEFF1
            highlight: Some(rgb(3, 169, 244)), // #03A9F4 (light blue)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// Ayu Dark theme
///
/// A simple theme with bright colors, comfortable for all-day coding.
/// Based on [ayu](https://github.com/dempfi/ayu)
pub fn ayu_dark() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(15, 20, 25),        // #0F1419
            foreground: rgb(230, 225, 207),     // #E6E1CF
            highlight: Some(rgb(255, 180, 84)), // #FFB454 (orange)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// Tokyo Night theme
///
/// Inspired by Tokyo's nightlife with colors representing neon lights.
/// Based on [tokyonight.nvim](https://github.com/folke/tokyonight.nvim)
pub fn tokyo_night() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(26, 27, 38),         // #1a1b26
            foreground: rgb(192, 202, 245),      // #c0caf5
            highlight: Some(rgb(187, 154, 247)), // #bb9af7 (magenta)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// Solarized Dark theme
///
/// A precision color scheme with careful attention to color theory.
/// Designed by Ethan Schoonover.
pub fn solarized_dark() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(0, 43, 54),         // #002B36 (base03)
            foreground: rgb(131, 148, 150),     // #839496 (base0)
            highlight: Some(rgb(42, 161, 152)), // #2aa198 (cyan)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// Solarized Light theme
///
/// The light variant of Solarized with the same carefully designed color palette.
pub fn solarized_light() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(253, 246, 227),     // #FDF6E3 (base3)
            foreground: rgb(101, 123, 131),     // #657B83 (base00)
            highlight: Some(rgb(38, 139, 210)), // #268bd2 (blue)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// One Dark theme
///
/// The popular dark theme from Atom editor, now widely used in VSCode.
/// Based on [One Dark Pro](https://github.com/binaryify/OneDark-Pro)
pub fn one_dark() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(40, 44, 52),         // #282C34
            foreground: rgb(171, 178, 191),      // #ABB2BF
            highlight: Some(rgb(198, 120, 221)), // #C678DD (purple)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// One Light theme
///
/// The light variant of One Dark theme.
pub fn one_light() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(239, 241, 245),     // #EFF1F5
            foreground: rgb(80, 87, 101),       // #505765
            highlight: Some(rgb(152, 40, 183)), // #9828b7 (purple)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// Black & White (Dark) theme
///
/// A minimal monochrome theme with pure black background and white foreground.
pub fn black_white_dark() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(0, 0, 0),            // #000000
            foreground: rgb(255, 255, 255),      // #FFFFFF
            highlight: Some(rgb(128, 128, 128)), // #808080 (gray)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// White & Black (Light) theme
///
/// A minimal monochrome theme with pure white background and black foreground.
pub fn white_black_light() -> Theme {
    let mut theme = Theme {
        global: GlobalTheme {
            background: rgb(255, 255, 255),      // #FFFFFF
            foreground: rgb(0, 0, 0),            // #000000
            highlight: Some(rgb(128, 128, 128)), // #808080 (gray)
        },
        ..Default::default()
    };
    theme.apply_global_as_default();
    theme
}

/// Get a theme by name
///
/// Returns `None` if the theme name is not recognized.
///
/// # Arguments
///
/// * `name` - The name of the theme (case-sensitive, must match list_themes() output)
///
/// # Examples
///
/// ```
/// let theme = builtin_themes::get_by_name("Catppuccin Mocha").unwrap();
/// let theme = builtin_themes::get_by_name("One Dark").unwrap();
/// ```
pub fn get_by_name(name: &str) -> Option<Theme> {
    match name {
        "Ayu Dark" => Some(ayu_dark()),
        "Black & White" => Some(black_white_dark()),
        "Catppuccin Frappé" => Some(catppuccin_frappe()),
        "Catppuccin Latte" => Some(catppuccin_latte()),
        "Catppuccin Macchiato" => Some(catppuccin_macchiato()),
        "Catppuccin Mocha" => Some(catppuccin_mocha()),
        "Dracula" => Some(dracula()),
        "Gruvbox Dark" => Some(gruvbox_dark()),
        "Material Dark" => Some(material_dark()),
        "Monokai" => Some(monokai()),
        "Nord" => Some(nord()),
        "One Dark" => Some(one_dark()),
        "One Light" => Some(one_light()),
        "Solarized Dark" => Some(solarized_dark()),
        "Solarized Light" => Some(solarized_light()),
        "Tokyo Night" => Some(tokyo_night()),
        "White & Black" => Some(white_black_light()),
        _ => None,
    }
}

/// Get a list of all available theme names
pub fn list_themes() -> &'static [&'static str] {
    &[
        "Ayu Dark",
        "Black & White",
        "Catppuccin Frappé",
        "Catppuccin Latte",
        "Catppuccin Macchiato",
        "Catppuccin Mocha",
        "Dracula",
        "Gruvbox Dark",
        "Material Dark",
        "Monokai",
        "Nord",
        "One Dark",
        "One Light",
        "Solarized Dark",
        "Solarized Light",
        "Tokyo Night",
        "White & Black",
    ]
}
