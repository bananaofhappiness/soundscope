//! Built-in themes for `Soundscope`
//!
//! This module contains pre-defined themes for popular color schemes.

use crate::tui::theme::{GlobalTheme, Theme};
use ratatui::style::Color;

#[inline]
const fn hex(value: u32) -> Color {
    Color::Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

macro_rules! builtin_themes {
    ($($name:ident, $display_name:expr, $bg:expr, $fg:expr, $hl:expr);+) => {
        $(fn $name() -> Theme {
            let mut theme = Theme {
                global: GlobalTheme {
                    background: hex($bg),
                    foreground: hex($fg),
                    highlight: Some(hex($hl)),
                },
                ..Default::default()
            };
            theme.apply_global_as_default();
            theme
        })+

        pub fn get_by_name(name: &str) -> Option<Theme> {
            match name {
                $($display_name => Some($name()),)+
                _ => None,
            }
        }

        pub fn list_themes() -> &'static [&'static str] {
            &[
                $($display_name,)+
            ]
        }
    };
}

builtin_themes! {
    ayu_dark,"Ayu Dark", 0x0F1419, 0xE6E1CF, 0xFFB454; // Based on [ayu](https://github.com/dempfi/ayu)
    black_white,"Black & White", 0x000000, 0xFFFFFF, 0x808080; // A minimal monochrome theme with pure black background and white foreground.
    catppuccin_frappe,"Catppuccin Frappe", 0x303446, 0xc6d0f5, 0xca9ee6; // Based on [Catppuccin](https://github.com/catppuccin/catppuccin)
    catppuccin_latte,"Catppuccin Latte", 0xeff1f5, 0x4c4f69, 0x8839ef; // Based on [Catppuccin](https://github.com/catppuccin/catppuccin)
    catppuccin_macchiato,"Catppuccin Macchiato", 0x24273a, 0xcad3f5, 0xb7bdf8; // Based on [Catppuccin](https://github.com/catppuccin/catppuccin)
    catppuccin_mocha,"Catppuccin Mocha", 0x1e1e2e, 0xcdd6f4, 0xcba6f7; // Based on [Catppuccin](https://github.com/catppuccin/catppuccin)
    dracula,"Dracula", 0x282a36, 0xf8f8f2, 0xbd93f9; // Based on [Dracula Theme](https://draculatheme.com/)
    gruvbox_dark,"Gruvbox Dark", 0x282828, 0xebdbb2, 0xfe8019; // Based on [Gruvbox](https://github.com/morhetz/gruvbox)
    material_dark,"Material Dark", 0x263238, 0xECEFF1, 0x03A9F4; // Based on Google's Material Design dark theme specifications.
    monokai,"Monokai", 0x272822, 0xf8f8f2, 0xf92672; // Originally from [TextMate](https://macromates.com/) editor.
    nord,"Nord", 0x2E3440, 0xD8DEE9, 0x88C0D0; // Based on [Nord](https://github.com/arcticicestudio/nord)
    one_dark,"One Dark", 0x282C34, 0xABB2BF, 0xC678DD; // Based on [One Dark Pro](https://github.com/binaryify/OneDark-Pro)
    one_light,"One Light", 0xEFF1F5, 0x505765, 0x9828b7; // The light variant of One Dark theme.
    solarized_dark,"Solarized Dark", 0x002B36, 0x839496, 0x2aa198; // Designed by Ethan Schoonover.
    solarized_light,"Solarized Light", 0xFDF6E3, 0x657B83, 0x268bd2; // The light variant of Solarized with the same carefully designed color palette.
    tokyo_night,"Tokyo Night", 0x1a1b26, 0xc0caf5, 0xbb9af7; // Based on [tokyonight.nvim](https://github.com/folke/tokyonight.nvim)
    white_black,"White & Black", 0xFFFFFF, 0x000000, 0x808080 // A minimal monochrome theme with pure white background and black foreground.
}
